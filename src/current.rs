//! 指示当前状态的全局变量
//!

use core::{
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering},
};

use lazyinit::LazyInit;
use spin::mutex::SpinMutex;
use vdso_helper::{get_vvar_data, vvar_data};

use crate::{
    interface::{CPU_NUM, SMP, SMPVirtImpl, TaskVirtImpl},
    scheduler::{ProcessInfoTable, Scheduler}, stack::StackHandler,
};

/// 当前栈变量以栈底指针形式存储，实现为perCPU的私有数据。
///
/// 通过每个地址空间提供的`Stack::alloc`和`Stack::dealloc`管理。
///
/// 通过比较栈顶寄存器与当前变量中存储的栈底指针，可以得知调度器当前在空栈还是非空栈上运行。进而决定是否回收/切换栈。
pub(crate) static CURRENT_STACK: [AtomicPtr<()>; CPU_NUM] = [AtomicPtr::new(null_mut()); CPU_NUM];

vvar_data! {
    /// 当前任务以指针形式存储，且使用已初始化的虚函数表调用当前任务的方法。
    ///
    /// 实现为perCPU的共享数据。
    ///
    /// 虽然该变量跨地址空间共享，但当前任务相关的方法只会在任务所在的地址空间（可能为用户态或内核态）中调用。
    CURRENT_TASK: [AtomicPtr<()>; CPU_NUM],
    /// 内核调度器的全局实例，实现为非perCPU的共享数据。
    ///
    /// 指针指向存放在内核空间的调度器实例，用于防止在用户态访问内核态调度器。
    KERNEL_SCHEDULER: LazyInit<AtomicPtr<Scheduler>>,
    /// 当前位于用户态或内核态，实现为perCPU的共享数据。
    IN_KERNEL: [AtomicBool; CPU_NUM],
    /// 以进程号表示的当前地址空间，实现为perCPU的共享数据。
    ///
    /// 进程号即为全局进程表中的索引。
    ///
    /// 若位于内核，则进程号为当前地址空间所属进程的进程号。
    /// （这一点与调度器中不同，因为内核态任务可以属于多个地址空间，但使用同一个调度器。）
    CURRENT_VSPACE: [AtomicUsize; CPU_NUM],
    /// 全局进程表，实现为非perCPU的共享数据。
    ///
    /// 存储了进程的最高优先级（全局共享）和地址空间（仅内核态访问），且承担了分配进程号的功能。
    PROCESS_INFO_TABLE: ProcessInfoTable,
}

pub(crate) fn get_current_task() -> &'static TaskVirtImpl {
    let cpu_id = SMPVirtImpl::cpu_id();
    let mut_ptr = get_vvar_data!(CURRENT_TASK)[cpu_id].load(Ordering::Acquire);
    unsafe { TaskVirtImpl::from_mut(mut_ptr) }
}

pub(crate) fn set_current_task(task: &'static TaskVirtImpl) {
    let cpu_id = SMPVirtImpl::cpu_id();
    let current_task = &get_vvar_data!(CURRENT_TASK)[cpu_id];
    current_task.store(task.to_ptr() as *mut (), Ordering::Release);
}

/// 当前进程的调度器，实现为非perCPU的私有数据
pub(crate) static USER_SCHEDULER: LazyInit<Scheduler> = LazyInit::new();

/// 当前进程的栈池，实现为非perCPU的私有数据。
pub(crate) static STACK_HANDLER: LazyInit<SpinMutex<StackHandler>> = LazyInit::new();
