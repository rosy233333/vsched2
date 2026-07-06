use core::{pin::Pin, sync::atomic::Ordering};

use spin::mutex::SpinMutex;
use vdso_helper::{get_vvar_data, log::info};

use crate::{
    current::{get_current_task, get_user_data, STACK_HANDLER, USER_SCHEDULER},
    schedule::scheduler::Scheduler,
    set_pre_stack,
    stack::StackHandler,
    SMPVirtImpl, StackVirtImpl, Task, TaskVirtImpl, SMP,
};

/// 在内核的主核心调用的调度器初始化接口。
///
/// 需要调用该函数之后，才能打开中断，因为中断的处理设计本模块的任务调度功能。
///
/// 该函数不会切换任务。初始化完成后若需切换任务，则需再调用`reschedule`函数。
///
/// 参数：
///
/// - `init_stack`：内核初始化使用的栈（也就是当前栈）的`Stack`数据结构指针。
/// - `init_task_ptr`：内核初始化执行流（当前执行流）所属的任务指针。需要内核先创建该任务，再将其指针传入该函数中。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_init_main(init_stack: *mut (), init_task_ptr: *const ()) {
    let cpu_id = SMPVirtImpl::cpu_id();

    // 初始化CURRENT_TASK
    get_vvar_data!(CURRENT_TASK)[cpu_id].store(init_task_ptr as *mut (), Ordering::Release);
    // info!(
    //     "current task inited: {:#x}!",
    //     get_vvar_data!(CURRENT_TASK)[cpu_id].load(Ordering::Acquire) as usize
    // );

    // 调度器初始化，虽然名称是USER_SCHEDULER，但它是内核调度器的实例，且在内核空间中存储和使用。
    Scheduler::init(unsafe { Pin::new_unchecked(&USER_SCHEDULER) }, 0);
    get_vvar_data!(KERNEL_SCHEDULER).store(
        USER_SCHEDULER.get().unwrap() as *const Scheduler as *mut Scheduler,
        Ordering::Release,
    );

    // 初始化IN_KERNEL
    get_vvar_data!(IN_KERNEL)[cpu_id].store(true, Ordering::Release);

    // 初始化CURRENT_VSPACE
    get_vvar_data!(CURRENT_VSPACE)[cpu_id].store(0, Ordering::Release);

    // PROCESS_INFO_TABLE无需初始化，因为其默认值已经包含了一个有效的内核进程。

    // 内核态不需要初始化STACK_HANDLER，但需初始化KERNEL_STACKS中的current_stack和trap_stacks
    let mut stacks = get_vvar_data!(KERNEL_STACKS).lock();
    stacks.current_stack[cpu_id] = Some(unsafe { StackVirtImpl::from_mut(init_stack) });
    // info!(
    //     "set current_stack in kernel_init_main: {:#x}",
    //     init_stack as usize
    // );
    let base = stacks.alloc_trap_stack(cpu_id);
    set_pre_stack!(base);
    drop(stacks);

    info!("kernel_init_main complete!");
}

/// 在内核的副核心调用的调度器初始化接口。
///
/// 需要调用该函数之后，才能打开中断，因为中断的处理设计本模块的任务调度功能。
///
/// 该函数不会切换任务。初始化完成后若需切换任务，则需再调用`reschedule`函数。
///
/// 参数：
///
/// - `init_stack`：内核初始化使用的栈（也就是当前栈）的`Stack`数据结构指针。
/// - `init_task_ptr`：内核初始化执行流（当前执行流）所属的任务指针。需要内核先创建该任务，再将其指针传入该函数中。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn kernel_init_secondary(init_stack: *mut (), init_task_ptr: *const ()) {
    // 不需初始化调度器，因为其已由`kernel_init_main`在主核心中初始化，并通过vDSO在所有核心中共享。
    let cpu_id = SMPVirtImpl::cpu_id();

    // 初始化CURRENT_TASK
    get_vvar_data!(CURRENT_TASK)[cpu_id].store(init_task_ptr as *mut (), Ordering::Release);

    // 初始化IN_KERNEL
    get_vvar_data!(IN_KERNEL)[cpu_id].store(true, Ordering::Release);

    // 初始化CURRENT_VSPACE
    get_vvar_data!(CURRENT_VSPACE)[cpu_id].store(0, Ordering::Release);

    // PROCESS_INFO_TABLE无需初始化，因为其默认值已经包含了一个有效的内核进程。

    // 内核态不需要初始化STACK_HANDLER，但需初始化KERNEL_STACKS中的current_stack
    let mut stacks = get_vvar_data!(KERNEL_STACKS).lock();
    stacks.current_stack[cpu_id] = Some(unsafe { StackVirtImpl::from_mut(init_stack) });
    // info!(
    //     "set current_stack in kernel_init_secondary: {:#x}",
    //     init_stack as usize
    // );
    let base = stacks.alloc_trap_stack(cpu_id);
    set_pre_stack!(base);

    info!("kernel_init_secondary complete!");
}

/// 在内核态调用的进程初始化接口，每个用户进程初始化一次。
///
/// 在调用此函数前，用户进程的地址空间需要已创建完成，且已映射和加载vDSO。
///
/// 参数：
///
/// - `vspace_ptr`：用户进程的地址空间（页表根节点）的指针，代表该进程所属的地址空间。需要内核先创建该地址空间，并将其指针传入该函数中。
/// 在实现时，一级指针可以放在TCB等位置，从而和进程一同释放。
///
/// 返回值：为该进程分配的pid
#[unsafe(no_mangle)]
pub extern "C" fn process_init(vspace: *mut ()) -> usize {
    // 初始化PROCESS_INFO_TABLE，分配进程号，填写地址空间。
    let pid = get_vvar_data!(PROCESS_INFO_TABLE)
        .register_process()
        .expect("Failed to register process");
    get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
        .vspace
        .store(vspace, Ordering::Release);
    // get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
    //     .highest_prio
    //     .store(isize::MAX, Ordering::Release);

    // 初始化用户态vDSO私有数据。
    // 需要在此处初始化的原因是需要初始化进程调度器，之后才能将该进程的任务放入调度器中。
    let user_scheduler = unsafe { get_user_data(&USER_SCHEDULER, Some(vspace)) };
    Scheduler::init_except_sources(unsafe { Pin::new_unchecked(user_scheduler) }, pid);
    get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
        .scheduler
        .store(
            user_scheduler.get().unwrap() as *const Scheduler as *mut Scheduler,
            Ordering::Release,
        );
    let stack_handler = unsafe { get_user_data(&STACK_HANDLER, Some(vspace)) };
    stack_handler.init_once(SpinMutex::new(StackHandler::default()));
    pid
}

/// 在内核态调用的进程销毁接口。
///
/// 目前只会释放进程表中的对应项。不会回收地址空间等资源，需要os负责回收。
#[unsafe(no_mangle)]
pub extern "C" fn process_drop(pid: usize) {
    get_vvar_data!(PROCESS_INFO_TABLE).unregister_process(pid);
}

// 在build_vdso中增加了暴露extern "C"函数的功能，通过以下的写法可以暴露汇编函数接口。
// 在os中使用时，不使用同名函数，而是直接从vtable中获取函数指针，从而避免多余的跳转和函数调用接口的适配。
// 以下函数可能有参数。接口见相应的汇编实现。
extern "C" {
    /// `raw_trap_entry`为os发生trap、保存上下文并进行一定的解析后进入的入口。
    ///
    /// os传递给调度器的参数：
    ///
    /// - #1: trap类型
    ///     - 0: 不是外部中断
    ///     - 1: 外部中断
    ///     - 2: 特殊参数的系统调用，仅用于“从用户态调度器进入内核”的情况。
    /// - #2: 代表（trap后的）当前特权级，1为用户态，0为内核态。
    ///
    /// 不同架构下传参使用的寄存器：
    ///
    /// |架构|#1|#2|
    /// |-|-|-|
    /// |riscv|a0|a1|
    /// |x86|ax|bx|
    ///
    /// 需要在关中断环境下进入
    pub fn raw_trap_entry() -> !;
    /// `raw_thread_entry`为os进行线程主动让权，保存上下文后进入的入口。
    ///
    /// 不同架构下传参使用的寄存器：
    ///
    /// |架构|#1|#2|
    /// |-|-|-|
    /// |riscv|a0|a1|
    /// |x86|ax|bx|
    ///
    /// 需要在关中断环境下进入
    pub fn raw_thread_entry() -> !;
    /// `raw_run_task`为从内核态调度器返回用户态调度器时返回的pc。
    ///
    ///  从内核返回用户态时，需要设置正确的reg1和reg2寄存器。
    ///
    /// 从`run_task`中返回后，需要重新设置reg1和reg2寄存器，因为`run_task`使用跳板切换了栈，再从另一个函数返回。
    /// 此时，被调用者不再能可靠地保存reg1和reg2。
    /// `uschedule`和`krun_utask`也涉及跳板换栈，但它们在换栈后一定不会返回，因此不需重新设置s1和s2。
    ///
    /// |架构|reg1|reg2|
    /// |-|-|-|
    /// |riscv|s1|s2|
    /// |x86_64|r12|r13|
    /// |x86_32|di|si|
    ///
    /// 需要在关中断环境下进入。
    pub fn raw_run_task() -> !;
    /// `raw_kschdule`为内核初始化时进入调度器的入口。
    ///
    /// 进入时，需设置s1=0, s2=0（riscv）/r12=0, r13=0（x86_64）/di=0, si=0（x86_32）
    ///
    /// 且在x86下进入该函数需使用call，以平衡堆栈。
    ///
    /// 需要在关中断环境下进入
    pub fn raw_kschedule() -> !;
}

/// 在用户态或内核态调用的调度器初始化接口，每个用户进程初始化一次。
///
/// 通过 vspace 显式定位目标地址空间中的 vDSO，完成 scheduler sources 初始化。
/// 兼容单页表和双页表：`get_user_data` 通过 vspace 翻译到目标进程的 vDSO，
/// 且 scheduler sources 使用字段偏移量存储，无论从内核 KVA 还是用户 UVA 访问均正确。
/// 
/// 该函数不会切换任务。初始化完成后若需切换任务，则需再调用`reschedule`函数。
#[unsafe(no_mangle)]
pub extern "C" fn user_init(vspace: *mut ()) {
    let scheduler = unsafe { get_user_data(&USER_SCHEDULER, Some(vspace)) };
    Scheduler::init_sources(unsafe { Pin::new_unchecked(scheduler) });
    // 用户态不需要初始化CURRENT_TASK、IN_KERNEL、STACK_HANDLER和CURRENT_VSPACE，因为它们在内核态切换到用户态任务时会被正确设置。
    // （TODO: 真的吗？）
}

/// 将任务放入当前进程、当前特权级的就绪队列。
///
/// task指针指向实现了`Task` trait的类型。
///
/// 返回值表示是否成功放入。
#[unsafe(no_mangle)]
pub extern "C" fn push_task_into_current(task: *const ()) -> bool {
    let scheduler = USER_SCHEDULER.get().unwrap();
    unsafe { scheduler.push_task(TaskVirtImpl::from_ptr(task)).is_ok() }
}

/// 将任务放入它所属的就绪队列。
///
/// 因为涉及到对其它地址空间的就绪队列的操作，因此只能在内核调用。
///
/// task指针指向实现了`Task` trait的类型。
///
/// 返回值表示是否成功放入。
#[unsafe(no_mangle)]
pub extern "C" fn push_task(task: *const ()) -> bool {
    let task = unsafe { TaskVirtImpl::from_ptr(task) };
    let scheduler = if task.is_kernel() {
        USER_SCHEDULER.get().unwrap() // 在内核，USER_SCHEDULER代表内核调度器
    } else {
        // 获取任务pid对应的用户态调度器
        let pid = task.pid();
        if get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
            .valid
            .load(Ordering::Acquire)
            == false
        {
            return false;
        }
        let ptr = get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
            .scheduler
            .load(Ordering::Acquire);
        if ptr.is_null() {
            return false;
        }
        unsafe { &*ptr }
    };
    scheduler.push_task(task).is_ok()
}

/// 将任务放入pid指定的进程的就绪队列。只能在内核调用。
///
/// task指针指向实现了`Task` trait的类型。
///
/// 返回值表示是否成功放入。
#[unsafe(no_mangle)]
pub extern "C" fn push_task_into_process(task: *const (), pid: usize) -> bool {
    if get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
        .valid
        .load(Ordering::Acquire)
        == false
    {
        return false;
    }
    let scheduler = get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
        .scheduler
        .load(Ordering::Acquire);
    if scheduler.is_null() {
        false
    } else {
        unsafe {
            (&*scheduler)
                .push_task(TaskVirtImpl::from_ptr(task))
                .is_ok()
        }
    }
}

/// 当前地址空间
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn current_vspace() -> usize {
    get_vvar_data!(CURRENT_VSPACE)[SMPVirtImpl::cpu_id()].load(Ordering::Acquire)
}

/// 在trap处理任务中运行的函数。
///
/// OS需在`TrapInfo::new_handler`的实现中，用这个函数创建trap处理任务。
/// 该函数的参数即为`new_handler`接口中传入的参数，即指向trap等待队列中某个核心的队列的指针。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn trap_handler(queue: *const ()) {
    crate::schedule::trap_wait_queue::trap_handler(queue);
}

/// 获取当前任务指针
///
/// 可能未初始化，此时会返回空指针。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn current_task_ptr() -> *const () {
    get_vvar_data!(CURRENT_TASK)[SMPVirtImpl::cpu_id()].load(Ordering::Acquire)
}

/// 设置当前任务指针，返回之前的值。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn set_current_task_ptr(task: *const ()) -> *const () {
    get_vvar_data!(CURRENT_TASK)[SMPVirtImpl::cpu_id()].swap(task as *mut (), Ordering::AcqRel)
}

/// 在保存线程上下文时，通过此函数获取当前栈。该核心的current_stack变为None。
///
/// （临时设计）获取当前栈之后，在调度器中实际使用的栈仍是被获取的栈。因为这个栈不会在调度器中再次操作，因此可以设置为None。
///
/// TODO: 修改如上设计和栈的相关设计，避免调度器和线程同时使用一个栈的同步问题。
///
/// 需要在关中断环境下调用。
#[unsafe(no_mangle)]
pub extern "C" fn take_current_stack() -> *mut () {
    let cpu_id = SMPVirtImpl::cpu_id();
    // 此处使用任务特权级而非当前特权级，这样才能获取与任务对应的栈。
    if get_current_task().is_kernel() {
        let res = get_vvar_data!(KERNEL_STACKS)
            .lock()
            .take_current_stack(cpu_id)
            .to_mut();
        res
    } else {
        STACK_HANDLER.lock().take_current_stack(cpu_id).to_mut()
    }
}
