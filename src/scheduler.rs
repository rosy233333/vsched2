//! 任务调度相关的数据结构与操作

use heapless::Vec;
use spin::{mutex::SpinMutex, rwlock::RwLock};

use crate::interface::{EventSorceVtable, EVENT_SORCE_NUM, PROCESS_NUM};
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize};

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
    /// 当前事件源数量
    source_num: AtomicUsize,
    /// 全局进程表中的索引，同时作为进程号使用
    ///
    /// 内核调度器固定为0
    global_index: usize,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

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
    highest_prio: AtomicUsize,
    /// 进程的地址空间
    ///
    /// AtomicPtr指向的内容（`*mut ()`）为存储在内核空间的页表根节点，
    /// 通过这种方式限制地址空间信息只能在内核态访问。
    vspace: AtomicPtr<*mut ()>,
}

impl Default for ProcessInfoTable {
    fn default() -> Self {
        Self {
            table: [const {
                ProcessInfo {
                    valid: AtomicBool::new(false),
                    highest_prio: AtomicUsize::new(0),
                    vspace: AtomicPtr::new(core::ptr::null_mut()),
                }
            }; PROCESS_NUM],
            next_index: AtomicUsize::new(0),
        }
    }
}
