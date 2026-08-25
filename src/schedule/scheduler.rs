//! 调度器

use core::{marker::PhantomPinned, pin::Pin, sync::atomic::Ordering};

use heapless::Vec;
use lazyinit::LazyInit;
// use pinned_init::{pin_data, pin_init, PinInit};
use spin::{rwlock::RwLock, RwLockReadGuard};
use vdso_helper::{get_vvar_data, log::warn};

use super::event_source::{EventSource, EventSourceVtable};
use crate::{
    interface::{SMPVirtImpl, TaskVirtImpl, EVENT_SORCE_NUM, SMP},
    schedule::{
        ready_queue::ReadyQueue,
        trap_wait_queue::{self, TrapWaitQueue},
    },
    TrapInfoVirtImpl, CPU_NUM,
};

/// 一个事件源在用户态和内核态中的数据地址及其vtable。
///
/// 两个地址可以指向同一组物理页的不同虚拟地址，也可以在只供内核 Scheduler中保持相同。
#[derive(Debug)]
struct EventSourceEntry {
    kernel_data: *const (),
    user_data: *const (),
    vtable: EventSourceVtable,
}

impl EventSourceEntry {
    const fn new(kernel_data: *const (), user_data: *const (), vtable: EventSourceVtable) -> Self {
        Self {
            kernel_data,
            user_data,
            vtable,
        }
    }

    fn data(&self, in_kernel: bool) -> *const () {
        if in_kernel {
            self.kernel_data
        } else {
            self.user_data
        }
    }
}

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
    /// 每项分别保存事件源数据和实现代码在用户态、内核态中的地址。
    ///
    /// 不能只保存相对Scheduler的偏移：外部事件源可能位于独立的共享映射，
    /// 它在用户态和内核态中相对Scheduler的位置不一定相同。
    sources: RwLock<Vec<EventSourceEntry, EVENT_SORCE_NUM>>,
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
    pub(crate) trap_wait_queue: TrapWaitQueue,
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

    /// 当前是否应使用事件源的内核态地址。
    fn in_kernel(&self, cpu_id: usize) -> bool {
        self.global_index == 0 || get_vvar_data!(IN_KERNEL)[cpu_id].load(Ordering::Acquire)
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
        let mut sources = self_ref.sources.write();
        // pin 投影，Pin<&LazyInit<Self>> -> Pin<&TrapWaitQueue>
        let twq_ref = unsafe { self_ref.map_unchecked(|s| &s.trap_wait_queue) };
        twq_ref.init(&**self_ref);
        let s = unsafe { self_ref.get_ref() };
        let twq = &s.trap_wait_queue as *const TrapWaitQueue as *const ();
        sources
            .push(EventSourceEntry::new(twq, twq, TrapWaitQueue::vtable()))
            .unwrap();
        let rq = &s.ready_queue as *const ReadyQueue as *const ();
        sources
            .push(EventSourceEntry::new(rq, rq, ReadyQueue::vtable()))
            .unwrap();
        self_ref.get_and_update_all_prio_with_guard(sources.downgrade());
    }

    /// 初始化调度器实例的`sources`以外的字段。
    ///
    /// 该函数用于新建进程时，从内核态初始化进程调度器实例。
    /// 因为在内核态访问用户态地址空间时无法正确处理调度器实例中的自引用指针，因此需要调用该函数初始化`sources`以外的字段。
    ///
    /// 内核可能访问进程调度器的`ready_queue`字段，因此需要在内核态即初始化调度器。
    /// 而内核不会访问`sources`字段，因此其可以在用户态初始化。
    /// TODO：真的吗？目前内核也会访问sources字段，比如ktask_schedule的else里
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
    /// 新建进程时，在内核态调用了`init_except_sources`之后，再调用`init_sources`
    /// 填写事件源数据和vtable的用户态、内核态地址。
    pub(crate) fn init_sources(
        self_ref: Pin<&LazyInit<Self>>,
        user_ref: *const LazyInit<Self>,
        vspace: *mut (),
    ) {
        let mut sources = self_ref.sources.write();
        // pin 投影，Pin<&LazyInit<Self>> -> Pin<&TrapWaitQueue>
        let twq_ref = unsafe { self_ref.map_unchecked(|s| &s.trap_wait_queue) };
        twq_ref.init(&**self_ref);
        let s = unsafe { self_ref.get_ref() };
        let kernel_ref = self_ref.as_ref().get_ref() as *const LazyInit<Self> as usize;
        let kernel_self = &**self_ref as *const Self as usize;
        let user_self = user_ref as usize + (kernel_self - kernel_ref);

        let twq_offset = s.field_offset(&s.trap_wait_queue);
        sources
            .push(EventSourceEntry::new(
                &s.trap_wait_queue as *const TrapWaitQueue as *const (),
                (user_self + twq_offset) as *const (),
                TrapWaitQueue::vtable().with_user(vspace),
            ))
            .unwrap();
        let rq_offset = s.field_offset(&s.ready_queue);
        sources
            .push(EventSourceEntry::new(
                &s.ready_queue as *const ReadyQueue as *const (),
                (user_self + rq_offset) as *const (),
                ReadyQueue::vtable().with_user(vspace),
            ))
            .unwrap();
        self_ref.get_and_update_all_prio_with_guard(sources.downgrade());
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
        kernel_data: *const (),
        user_data: *const (),
        vtable: EventSourceVtable,
        index: isize,
    ) -> bool {
        if kernel_data.is_null() || user_data.is_null() {
            return false;
        }
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
        if sources
            .insert(
                insert_index,
                EventSourceEntry::new(kernel_data, user_data, vtable),
            )
            .is_ok()
        {
            self.get_and_update_all_prio_with_guard(sources.downgrade());
            true
        } else {
            false
        }
    }

    /// 取消注册事件源，返回是否成功取消
    fn unregister_event_source(&self, event_source: *const ()) -> bool {
        let mut sources = self.sources.write();
        let cpu_id = SMPVirtImpl::cpu_id();
        let in_kernel = self.in_kernel(cpu_id);
        if let Some(index) = sources
            .iter()
            .position(|source| source.data(in_kernel) == event_source)
        {
            sources.remove(index);
            self.get_and_update_all_prio_with_guard(sources.downgrade());
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
        let in_kernel = self.in_kernel(cpu_id);
        let sources = self.sources.read();
        sources
            .iter()
            .map(|source| {
                source
                    .vtable
                    .hightest_priority(source.data(in_kernel), cpu_id, in_kernel)
            })
            .fold(isize::MAX, |a, b| if a < b { a } else { b })
    }

    /// 在已持有self.source的Guard的情况下，执行hightest_priority。
    ///
    /// 返回该调度器中所有事件源中所有就绪任务的最高优先级。优先级数值越低，优先级越高。
    ///
    /// 若没有事件源，返回`isize::MAX`；若有事件源但没有就绪任务，返回比最低优先级更低一级的优先级。
    fn hightest_priority_with_guard<'a>(
        &self,
        guard: RwLockReadGuard<'a, Vec<EventSourceEntry, EVENT_SORCE_NUM>>,
    ) -> isize {
        let cpu_id = SMPVirtImpl::cpu_id();
        let in_kernel = self.in_kernel(cpu_id);
        guard
            .iter()
            .map(|source| {
                source
                    .vtable
                    .hightest_priority(source.data(in_kernel), cpu_id, in_kernel)
            })
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
        let in_kernel = self.in_kernel(cpu_id);
        let sources = self.sources.read();
        // info!("after get sources");
        let ((first_index, first_prio), (_second_index, second_prio)) = sources
            .iter()
            .map(|source| {
                source
                    .vtable
                    .hightest_priority(source.data(in_kernel), cpu_id, in_kernel)
            })
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
            // info!("before return, no source");
            // self.update_prio(isize::MAX);
            self.update_prio(isize::MAX, cpu_id as isize);
            return (None, isize::MAX);
        }

        let source = &sources[first_index];
        let ptr = source.data(in_kernel);
        // info!(
        //     "offset: {:#x}, ptr: {:#x}, fn: {:#x}",
        //     sources[first_index].0, ptr as usize, take_task_fn as usize
        // );
        // info!("before take_task");
        let (task, new_prio) = source.vtable.take_task(ptr, cpu_id, in_kernel);
        // info!("return from take_task");
        let prio = if new_prio < second_prio {
            new_prio
        } else {
            second_prio
        };

        if source.vtable.is_prio_per_cpu {
            self.update_prio(prio, cpu_id as isize);
        } else {
            self.update_prio(prio, -1);
        }
        if task.is_null() {
            // info!("before return, task = None");
            (None, prio)
        } else {
            // info!("before return, task = Some({:#x})", task as usize);
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
            if ReadyQueue::IS_PRIO_PER_CPU {
                self.get_and_update_current_prio();
            } else {
                self.get_and_update_all_prio();
                // 目前不主动发送ipi唤醒休眠核心，
                // 因为在测例里出现了在任务中接收到ipi --> 将任务放回就绪队列 --> 发送ipi的循环，
                // 导致出现了大量的中断请求。
                // 并且，休眠核心也会接收时钟中断，并在被唤醒后检查能否从调度器中取出任务，
                // 因此这里的主动ipi唤醒也是不必要的。
                // let cpu_id = SMPVirtImpl::cpu_id();
                // // 检查是否有CPU正在睡眠，若有则唤醒一个。
                // for i in 1..CPU_NUM {
                //     let target_cpu = (i + cpu_id) % CPU_NUM;
                //     if get_vvar_data!(IS_SLEEPING)[target_cpu].load(Ordering::Acquire) {
                //         SMPVirtImpl::send_ipi(target_cpu);
                //         break;
                //     }
                // }
            }
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
            if TrapWaitQueue::IS_PRIO_PER_CPU {
                self.get_and_update_current_prio();
            } else {
                self.get_and_update_all_prio();
                // 目前不主动发送ipi唤醒休眠核心，
                // 因为在测例里出现了在任务中接收到ipi --> 将任务放回就绪队列 --> 发送ipi的循环，
                // 导致出现了大量的中断请求。
                // 并且，休眠核心也会接收时钟中断，并在被唤醒后检查能否从调度器中取出任务，
                // 因此这里的主动ipi唤醒也是不必要的。
                // let cpu_id = SMPVirtImpl::cpu_id();
                // // 检查是否有CPU正在睡眠，若有则唤醒一个。
                // for i in 1..CPU_NUM {
                //     let target_cpu = (i + cpu_id) % CPU_NUM;
                //     if get_vvar_data!(IS_SLEEPING)[target_cpu].load(Ordering::Acquire) {
                //         SMPVirtImpl::send_ipi(target_cpu);
                //         break;
                //     }
                // }
            }
        }
        res
    }

    /// 更新全局进程表中，本进程的优先级
    ///
    /// cpu_id参数为需要更新的CPU的id，若为-1则更新所有CPU的优先级。
    #[inline]
    pub(crate) fn update_prio(&self, prio: isize, cpu_id: isize) {
        if cpu_id < 0 {
            for cpu_id in 0..CPU_NUM {
                get_vvar_data!(PROCESS_INFO_TABLE).table[self.global_index].highest_prio[cpu_id]
                    .store(prio, Ordering::Release);
            }
        } else {
            get_vvar_data!(PROCESS_INFO_TABLE).table[self.global_index].highest_prio
                [cpu_id as usize]
                .store(prio, Ordering::Release);
        }
    }

    /// 获取并更新当前cpu的本进程优先级
    #[inline]
    pub(crate) fn get_and_update_current_prio(&self) -> isize {
        let prio = self.hightest_priority();
        let cpu_id = SMPVirtImpl::cpu_id();
        self.update_prio(prio, cpu_id as isize);
        prio
    }

    /// 在已持有self.source的Guard的情况下，执行get_and_update_current_prio。
    ///
    /// 不会重复获取guard。
    #[inline]
    fn get_and_update_current_prio_with_guard<'a>(
        &self,
        guard: RwLockReadGuard<'a, Vec<EventSourceEntry, EVENT_SORCE_NUM>>,
    ) -> isize {
        let prio = self.hightest_priority_with_guard(guard);
        let cpu_id = SMPVirtImpl::cpu_id();
        self.update_prio(prio, cpu_id as isize);
        prio
    }

    /// 获取并更新所有cpu的本进程优先级
    #[inline]
    pub(crate) fn get_and_update_all_prio(&self) -> isize {
        let prio = self.hightest_priority();
        self.update_prio(prio, -1);
        prio
    }

    /// 在已持有self.source的Guard的情况下，执行get_and_update_all_prio。
    ///
    /// 不会重复获取guard。
    #[inline]
    fn get_and_update_all_prio_with_guard<'a>(
        &self,
        guard: RwLockReadGuard<'a, Vec<EventSourceEntry, EVENT_SORCE_NUM>>,
    ) -> isize {
        let prio = self.hightest_priority_with_guard(guard);
        self.update_prio(prio, -1);
        prio
    }
}
