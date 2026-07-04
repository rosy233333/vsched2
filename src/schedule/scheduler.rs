//! 调度器

use core::{marker::PhantomPinned, pin::Pin, sync::atomic::Ordering};

use heapless::Vec;
use lazyinit::LazyInit;
// use pinned_init::{pin_data, pin_init, PinInit};
use spin::rwlock::RwLock;
use vdso_helper::get_vvar_data;

use super::event_source::{EventSource, EventSourceVtable};
use crate::{
    interface::{SMPVirtImpl, TaskVirtImpl, EVENT_SORCE_NUM, SMP},
    schedule::{
        ready_queue::ReadyQueue,
        trap_wait_queue::{self, TrapWaitQueue},
    },
    TrapInfoVirtImpl,
};

/// 调度器数据结构
///
/// 每个进程的用户部分持有一个调度器实例；所有内核任务共享一个调度器实例。
pub(crate) struct Scheduler {
    /// 事件源数组
    ///
    /// 当前的RwLock仅保护事件源插入（申请写锁）与事件源查询（申请读锁）的冲突，并未保护多个事件源查询操作间的同步问题。
    ///
    /// 也就是要求事件源自身实现内部可变性和与之适配的同步机制。
    ///
    /// `source`的`index=0`处一定为就绪队列。
    ///
    /// 存储 (偏移量, vtable) 对，偏移量为事件源字段相对 Scheduler 结构体基址的字节偏移。
    ///
    /// 使用偏移量而非绝对指针，兼容双页表设计：
    /// - 内核态通过 KVA 访问调度器时，self + offset 得到 KVA 事件源指针
    /// - 用户态通过 UVA 访问调度器时，self + offset 得到 UVA 事件源指针
    sources: RwLock<Vec<(usize, EventSourceVtable), EVENT_SORCE_NUM>>,
    /// 全局进程表中的索引，同时作为进程号使用
    ///
    /// 内核调度器固定为0
    global_index: usize,
    /// 就绪队列
    ///
    /// 由于其同时放在了事件源数组中，因此在Scheduler结构中产生了自引用，需要声明为`!Unpin`。
    ///
    /// 放入任务时使用自身接口，取出任务时使用事件源接口。
    ready_queue: ReadyQueue,
    /// trap等待队列。
    ///
    /// 也会作为事件源放入事件源数组中并产生自引用。
    ///
    /// 放入任务时使用自身接口，取出任务时使用事件源接口。
    trap_wait_queue: TrapWaitQueue,
    // #[pin]
    _pin: PhantomPinned,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

impl Scheduler {
    /// 计算字段相对 self 基址的偏移量
    fn field_offset<T>(&self, field: *const T) -> usize {
        field as usize - self as *const Self as usize
    }

    /// 从偏移量还原绝对指针（在当前地址空间中有效）
    fn ptr_from_offset(&self, offset: usize) -> *const () {
        (self as *const Self as usize + offset) as *const ()
    }

    /// 初始化调度器实例
    pub(crate) fn init(self_ref: Pin<&LazyInit<Self>>, global_index: usize) {
        let ready_queue = ReadyQueue::new();
        let trap_wait_queue = TrapWaitQueue::new();
        self_ref.init_once(Self {
            sources: RwLock::new(Vec::new()),
            global_index,
            ready_queue,
            trap_wait_queue,
            _pin: PhantomPinned,
        });
        let twq_ref = unsafe { self_ref.map_unchecked(|s| &s.trap_wait_queue) };
        twq_ref.init();
        let s = unsafe { self_ref.get_ref() };
        self_ref
            .sources
            .write()
            .push((
                s.field_offset(&s.trap_wait_queue),
                TrapWaitQueue::vtable(),
            ))
            .unwrap();
        self_ref
            .sources
            .write()
            .push((
                s.field_offset(&s.ready_queue),
                ReadyQueue::vtable(),
            ))
            .unwrap();
        self_ref.get_and_update_prio();
    }

    /// 初始化调度器实例的`sources`以外的字段。
    ///
    /// 该函数用于新建进程时，从内核态初始化进程调度器实例。
    /// 因为在内核态访问用户态地址空间时无法正确处理调度器实例中的自引用指针，因此需要调用该函数初始化`sources`以外的字段。
    ///
    /// 内核可能访问进程调度器的`ready_queue`字段，因此需要在内核态即初始化调度器。
    /// 而内核不会访问`sources`字段，因此其可以在用户态初始化。
    pub(crate) fn init_except_sources(self_ref: Pin<&LazyInit<Self>>, global_index: usize) {
        let ready_queue = ReadyQueue::new();
        let trap_wait_queue = TrapWaitQueue::new();
        self_ref.init_once(Self {
            sources: RwLock::new(Vec::new()),
            global_index,
            ready_queue,
            trap_wait_queue,
            _pin: PhantomPinned,
        });
    }

    /// 初始化调度器实例的`sources`字段。
    ///
    /// 新建进程时，在内核态调用了`init_except_sources`之后，再在用户态调用`init_sources`以完成调度器实例的初始化。
    pub(crate) fn init_sources(self_ref: Pin<&LazyInit<Self>>) {
        let twq_ref = unsafe { self_ref.map_unchecked(|s| &s.trap_wait_queue) };
        twq_ref.init();
        let s = unsafe { self_ref.get_ref() };
        self_ref
            .sources
            .write()
            .push((
                s.field_offset(&s.trap_wait_queue),
                TrapWaitQueue::vtable(),
            ))
            .unwrap();
        self_ref
            .sources
            .write()
            .push((
                s.field_offset(&s.ready_queue),
                ReadyQueue::vtable(),
            ))
            .unwrap();
        self_ref.get_and_update_prio();
    }

    /// 注册事件源
    ///
    /// index参数为事件源的插入位置，在获取到的最高优先级相同时，优先选择位置靠前的事件源。
    ///
    /// index为0或正数时在index位置插入事件源，index为负数时在倒数第index位置插入事件源。插入成功则返回true。
    ///
    /// 若index>len或index<-len-1（len为当前事件源数量），则插入失败，返回false。
    fn register_event_source(
        &self,
        event_source: *const (),
        vtable: EventSourceVtable,
        index: isize,
    ) -> bool {
        let mut sources = self.sources.write();
        let len = sources.len() as isize;
        if index > len || index < -len - 1 {
            return false;
        }
        let insert_index = if index >= 0 {
            index as usize
        } else {
            (len + index) as usize
        };
        if sources.insert(insert_index, (self.field_offset(event_source), vtable)).is_ok() {
            self.get_and_update_prio();
            true
        } else {
            false
        }
    }

    /// 取消注册事件源，返回是否成功取消
    fn unregister_event_source(&self, event_source: *const ()) -> bool {
        let mut sources = self.sources.write();
        let target_offset = self.field_offset(event_source);
        if let Some(index) = sources.iter().position(|(off, _)| *off == target_offset) {
            sources.remove(index);
            self.get_and_update_prio();
            true
        } else {
            false
        }
    }

    /// 返回该调度器中所有事件源中所有就绪任务的最高优先级。优先级数值越低，优先级越高。
    ///
    /// 若没有事件源，返回`isize::MAX`；若有事件源但没有就绪任务，返回比最低优先级更低一级的优先级。
    pub(crate) fn hightest_priority(&self) -> isize {
        let cpu_id = SMPVirtImpl::cpu_id();
        let sources = self.sources.read();
        sources
            .iter()
            .map(|(off, vtable)| (vtable.hightest_priority)(self.ptr_from_offset(*off), cpu_id))
            .fold(isize::MAX, |a, b| if a < b { a } else { b })
    }

    /// 从调度器中取出最高优先级的下一任务
    ///
    /// 返回值：
    ///
    /// - 就绪任务的指针，指向外部定义，实现`Task` trait的类型，若没有就绪任务则返回空指针；
    /// - 取出就绪任务后事件源中就绪任务的最高优先级。
    ///     - 若没有事件源，则返回`isize::MAX`；
    ///     - 若有事件源但没有就绪任务，返回比最低优先级更低一级的优先级。
    pub(crate) fn pop_task(&self) -> (Option<&TaskVirtImpl>, isize) {
        let cpu_id = SMPVirtImpl::cpu_id();
        let sources = self.sources.read();
        let ((first_index, first_prio), (_second_index, second_prio)) = sources
            .iter()
            .map(|(off, vtable)| (vtable.hightest_priority)(self.ptr_from_offset(*off), cpu_id))
            .enumerate()
            .fold(
                ((usize::MAX, isize::MAX), (usize::MAX, isize::MAX)),
                |(first, second), current| {
                    if current.1 < first.1 {
                        (current, first)
                    } else if current.1 < second.1 {
                        (first, current)
                    } else {
                        (first, second)
                    }
                },
            );

        if first_index == usize::MAX {
            self.update_prio(isize::MAX);
            return (None, isize::MAX);
        }

        let ptr = self.ptr_from_offset(sources[first_index].0);
        let (task, new_prio) = (sources[first_index].1.take_task)(ptr, cpu_id);
        if task.is_null() {
            assert!(new_prio == first_prio);
            (None, new_prio)
        } else {
            let prio = if new_prio < second_prio {
                new_prio
            } else {
                second_prio
            };
            (Some(unsafe { TaskVirtImpl::from_ptr(task) }), prio)
        }
    }

    /// 获取全局进程表中的索引/进程号，只读
    pub(crate) fn global_index(&self) -> usize {
        self.global_index
    }

    /// 向就绪队列中放入任务
    pub(crate) fn push_task(
        &self,
        task: &'static TaskVirtImpl,
    ) -> Result<(), &'static TaskVirtImpl> {
        let res = self.ready_queue.push_task(task);
        if res.is_ok() {
            self.get_and_update_prio();
        }
        res
    }

    /// 将一个trap信息和一个可选的被trap的任务放入队列
    pub(crate) fn push_trap(
        &self,
        trap_info: &'static TrapInfoVirtImpl,
        task: Option<&'static TaskVirtImpl>,
        cpuid: usize,
    ) -> Result<(), (&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>)> {
        let res = self.trap_wait_queue.push_trap(trap_info, task, cpuid);
        if res.is_ok() {
            self.get_and_update_prio();
        }
        res
    }

    /// 更新全局进程表中，本进程的优先级
    #[inline]
    pub(crate) fn update_prio(&self, prio: isize) {
        get_vvar_data!(PROCESS_INFO_TABLE).table[self.global_index]
            .highest_prio
            .store(prio, Ordering::Release);
    }

    #[inline]
    pub(crate) fn get_and_update_prio(&self) {
        let prio = self.hightest_priority();
        self.update_prio(prio);
    }
}
