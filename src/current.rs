//! 指示当前状态的全局变量
//!

use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use lazyinit::LazyInit;
use spin::mutex::SpinMutex;
use vdso_helper::{get_vvar_data, vvar_data};

use crate::{
    interface::{SMPVirtImpl, TaskVirtImpl, UserData, UserDataVirtImpl, CPU_NUM, SMP},
    schedule::{process_info::ProcessInfoTable, scheduler::Scheduler},
    stack::StackHandler,
};

// / 当前栈变量以栈底指针形式存储，实现为perCPU的私有数据。
// /
// / 通过每个地址空间提供的`Stack::alloc`和`Stack::dealloc`管理。
// /
// / 通过比较栈顶寄存器与当前变量中存储的栈底指针，可以得知调度器当前在空栈还是非空栈上运行。进而决定是否回收/切换栈。
// pub(crate) static CURRENT_STACK: [AtomicPtr<()>; CPU_NUM] = [AtomicPtr::new(null_mut()); CPU_NUM];

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
    /// （这一点与调度器中不同，因为同一个调度器可以包含属于多个地址空间的内核态任务。）
    ///
    /// 该变量同时指示了当前地址空间和当前调度器，因此当两者产生分歧时，该变量的行为如下：
    ///
    /// 1. 当前在内核，下一任务为用户任务时，当前地址空间和当前调度器统一。在修改CURRENT_VSPACE后立刻切换地址空间，
    /// 之后从地址空间中获取用户调度器，并取出用户任务。
    /// 2. 当前在内核，下一任务为内核任务时，CURRENT_VSPACE设置为内核任务所在的地址空间所属进程的进程号后立刻切换地址空间。
    /// 仅在该内核任务所在的地址空间也为内核空间时，CURRENT_VSPACE为0。
    /// 3. 当前在用户态，由于其它调度器优先级更高而即将陷入内核时：先将CURRENT_VSPACE设置为下一个任务所在调度器的pid，
    /// 进入内核后，再参照第1、2条的情况，进行CURRENT_VSPACE的修正和地址空间的实际切换，
    /// 之后CURRENT_VSPACE即代表当前的地址空间。
    CURRENT_VSPACE: [AtomicUsize; CPU_NUM],
    /// 全局进程表，实现为非perCPU的共享数据。
    ///
    /// 存储了进程的最高优先级（全局共享）和地址空间（仅内核态访问），且承担了分配进程号的功能。
    PROCESS_INFO_TABLE: ProcessInfoTable,
    /// 内核栈池，实现为非perCPU的共享数据。
    ///
    /// 每个CPU分配一个内核栈池，管理内核态使用的栈
    KERNEL_STACKS: SpinMutex<StackHandler>,
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

/// 从内核中访问当前地址空间的用户态vDSO私有数据
///
/// 因为即使在一个地址空间中，用户态和内核态的vDSO私有数据也是分开的，因此需要借助这个函数进行地址运算，获得用户态对应数据的引用。
///
/// # Safety
///
/// - 因为访问的是用户态子空间的数据，因此不能在切换地址空间前后访问该函数返回的同一份引用。
pub(crate) unsafe fn get_user_data<T>(data: &T) -> &T {
    let kernel_addr = data as *const T as usize;
    let len = core::mem::size_of::<T>();

    let user_ptr = UserDataVirtImpl::get_user_data(kernel_addr, len);
    assert!(
        !user_ptr.is_null(),
        "UserData::get_user_data returned a null pointer"
    );

    unsafe { &*(user_ptr as *const T) }
}
