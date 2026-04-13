//! 任务调度相关的数据结构与操作

use heapless::Vec;
use spin::{mutex::SpinMutex, rwlock::RwLock};

use crate::{
    current,
    interface::{EventSorceVtable, SMPVirtImpl, TaskVirtImpl, EVENT_SORCE_NUM, PROCESS_NUM, SMP},
};
use core::{
    isize, ptr,
    sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicUsize, Ordering},
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
    sources: RwLock<Vec<(*const (), EventSorceVtable), EVENT_SORCE_NUM>>,
    /// 全局进程表中的索引，同时作为进程号使用
    ///
    /// 内核调度器固定为0
    global_index: usize,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

impl Scheduler {
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
        vtable: EventSorceVtable,
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
        sources.insert(insert_index, (event_source, vtable)).is_ok()
    }

    /// 取消注册事件源，返回是否成功取消
    fn unregister_event_source(&self, event_source: *const ()) -> bool {
        let mut sources = self.sources.write();
        if let Some(index) = sources.iter().position(|(ptr, _)| *ptr == event_source) {
            sources.remove(index);
            true
        } else {
            false
        }
    }

    /// 返回该调度器中所有事件源中所有就绪任务的最高优先级。优先级数值越低，优先级越高。
    ///
    /// 若没有事件源，返回`isize::MAX`；若有事件源但没有就绪任务，返回比最低优先级更低一级的优先级。
    pub(crate) fn hightest_priority(&self) -> isize {
        let sources = self.sources.read();
        sources
            .iter()
            .map(|(ptr, vtable)| (vtable.hightest_priority)(*ptr))
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
    pub(crate) fn take_task(&self) -> (Option<&TaskVirtImpl>, isize) {
        let sources = self.sources.read();
        let ((first_index, first_prio), (_second_index, second_prio)) = sources
            .iter()
            .map(|(ptr, vtable)| (vtable.hightest_priority)(*ptr))
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
            return (None, isize::MAX);
        }

        let cpu_id = SMPVirtImpl::cpu_id();
        let (task, new_prio) = (sources[first_index].1.take_task)(sources[first_index].0, cpu_id);
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
}

/// 以进程号为索引的数组，存储每个进程的信息
pub(crate) struct ProcessInfoTable {
    /// 数组
    table: [ProcessInfo; PROCESS_NUM],
    /// 下一个分配的进程索引
    ///
    /// 分配索引时，先使用该索引分配进程号，然后将其加1。若该值超过数组长度，则对PROCESS_NUM取模。
    ///
    /// 分配进程号后，需要将对应的`ProcessInfo.valid`置为`true`以表示该索引被占用。
    /// 若该索引已被占用，则继续分配下一个索引，直到找到一个未被占用的索引或遍历完整个数组。
    next_index: AtomicUsize,
}

/// 进程信息数据结构，实现了内部可变性。
///
/// 不包含进程号，因为进程号即为全局进程表中的索引。
///
/// 不包含调度器，因为进程调度器存储于不同地址空间的同一地址（通过vDSO实现），
/// 因此切换地址空间时自然切换了调度器。
pub(crate) struct ProcessInfo {
    /// 有效位，用于表示全局进程表中的该索引是否被占用
    valid: AtomicBool,
    /// 进程最高优先级，跨地址空间和特权级共享
    highest_prio: AtomicIsize,
    /// 进程的地址空间
    ///
    /// AtomicPtr指向的内容（`*mut ()`）为存储在内核空间的页表根节点，
    /// 通过这种方式限制地址空间信息只能在内核态访问。
    vspace: AtomicPtr<*mut ()>,
}

impl Default for ProcessInfoTable {
    fn default() -> Self {
        let mut default = Self {
            table: [const {
                ProcessInfo {
                    valid: AtomicBool::new(false),
                    highest_prio: AtomicIsize::new(isize::MAX),
                    vspace: AtomicPtr::new(core::ptr::null_mut()),
                }
            }; PROCESS_NUM],
            next_index: AtomicUsize::new(1),
        };
        default.table[0]
            .valid
            .store(true, core::sync::atomic::Ordering::Release);
        default
    }
}

impl ProcessInfoTable {
    /// 分配一个新的进程号，并返回对应的索引
    ///
    /// 若分配成功，则返回Some(索引)；若分配失败（即表中没有空位），则返回None。
    pub fn register_process(&self) -> Option<usize> {
        let start_index = self.next_index.fetch_add(1, Ordering::AcqRel) % PROCESS_NUM;
        let mut index = start_index;
        loop {
            if !self.table[index].valid.swap(true, Ordering::AcqRel) {
                return Some(index);
            }
            index = (index + 1) % PROCESS_NUM;
            if index == start_index {
                return None;
            }
        }
    }

    /// 注销一个进程号，返回是否成功注销
    pub fn unregister_process(&self, index: usize) -> bool {
        self.table[index].valid.swap(false, Ordering::AcqRel)
    }
}
