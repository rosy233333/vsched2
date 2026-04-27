// 全局进程数组，及每个进程在数组中存储的信息

use crate::interface::PROCESS_NUM;
use core::sync::atomic::{AtomicBool, AtomicIsize, AtomicPtr, AtomicUsize, Ordering};
use heapless::Vec;

/// 以进程号为索引的数组，存储每个进程的信息
///
/// 数组中至少有一个元素（内核）。
pub(crate) struct ProcessInfoTable {
    /// 数组
    pub(crate) table: [ProcessInfo; PROCESS_NUM],
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
    pub(crate) highest_prio: AtomicIsize,
    /// 进程的地址空间
    ///
    /// AtomicPtr指向的内容（`*mut ()`）为存储在内核空间的页表根节点，
    /// 通过这种方式限制地址空间信息只能在内核态访问。
    pub(crate) vspace: AtomicPtr<*mut ()>,
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
        default.table[0].valid.store(true, Ordering::Release);
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
            index = self.next_index.fetch_add(1, Ordering::AcqRel) % PROCESS_NUM;
            if index == start_index {
                return None;
            }
        }
    }

    /// 注销一个进程号，返回是否成功注销
    pub fn unregister_process(&self, index: usize) -> bool {
        self.table[index].valid.swap(false, Ordering::AcqRel)
    }

    /// 获取最高优先级的进程。
    ///
    /// 如果当前进程是最高优先级的进程之一，则返回当前进程。
    ///
    /// 若不是，则暂未规定以什么方式从所有最高优先级的进程中选择一个。
    pub fn highest_prio_process(&self, current_process: usize) -> usize {
        let next_index = self.next_index.load(Ordering::Acquire);
        let start = if next_index >= PROCESS_NUM {
            next_index - PROCESS_NUM
        } else {
            0
        };
        let end = next_index;

        let mut highest_prio: isize = isize::MAX;
        let mut processes: Vec<usize, PROCESS_NUM> = Vec::new();
        for i in start..end {
            if !self.table[i].valid.load(Ordering::Acquire) {
                continue;
            }
            let prio = self.table[i].highest_prio.load(Ordering::Acquire);
            if !self.table[i].valid.load(Ordering::Acquire) {
                continue;
            }
            if prio < highest_prio {
                highest_prio = prio;
                processes = Vec::from([i]);
            } else if prio == highest_prio {
                processes.push(i);
            }
        }
        if processes.contains(&current_process) {
            current_process
        } else {
            processes[0]
        }
    }
}
