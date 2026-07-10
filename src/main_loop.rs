//! 调度循环
//!
//! 结构详见：[调度循环结构图](https://github.com/rosy233333/weekly-progress/blob/dev/26.3.30~26.4.5/%E4%BB%BB%E5%8A%A1%E5%88%87%E6%8D%A2%E6%A8%A1%E5%9E%8B%EF%BC%88%E6%96%B0%E7%89%88%E8%B0%83%E5%BA%A6%E7%AE%97%E6%B3%95%EF%BC%89.png)
//!
//! 几个函数的运行顺序：`trap_entry` -> `run_task`，`schedule` -> `run_task`，之后在`schedule`和`run_task`中循环，使用`jmp`相互跳转实现循环

use core::{sync::atomic::Ordering, task::Poll};

use crate::{
    arch::assert_disable_irq,
    current::{
        self, get_current_task, get_user_data, set_current_task, STACK_HANDLER, USER_SCHEDULER,
    },
    get_sp,
    interface::{
        Context, ContextVirtImpl, SMPVirtImpl, Task, TaskState, TaskVirtImpl, TrapInfoVirtImpl,
        VSpace, VSpaceVirtImpl, SMP,
    },
    jump_to_trampoline,
    schedule::scheduler::Scheduler,
    set_pre_stack,
    stack::{coroutine_trampoline, thread_trampoline},
    Stack, StackVirtImpl, TrapInfo,
};
use vdso_helper::{
    get_vvar_data,
    log::{info, warn},
};

/// 同步trap入口
///
/// 切换栈，进入`trap_handle`
///
/// 参数：
///
/// - trap_type: trap类型
///     - 0: 不是外部中断
///     - 1: 外部中断
///     - 2: 特殊参数的系统调用，仅用于“从用户态调度器进入内核”的情况。
/// - privilege: （trap后的）当前特权级
///     - 0: 内核态
///     - 1: 用户态
///
/// 返回值：下一步的跳转目标
///
/// - 0: `trap_handle`
/// - 1: `kschedule`
/// - 2: `uschedule`
/// - 3: `utok_schedule`
#[no_mangle]
pub extern "C" fn trap_entry(trap_type: usize, privilege: usize) -> usize {
    assert_disable_irq("trap_entry");
    match privilege {
        0 => get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].store(true, Ordering::Release),
        1 => get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].store(false, Ordering::Release),
        privilege => unreachable!("unknown privilege level: {privilege}"),
    }
    match trap_type {
        // 普通同步 trap，进入 trap 处理流程。
        0 => {
            if privilege == 0 {
                let cpu_id = SMPVirtImpl::cpu_id();
                let mut stacks = get_vvar_data!(KERNEL_STACKS).lock();
                // info!("[trap_entry:sync] old_sscratch={:#x}", old_sscratch);
                let new_stack = stacks.alloc_stack();
                set_pre_stack!(new_stack.base());
                // Recycle the old pre-save stack: set it as current_stack so
                // run_task / krun_utask will reuse or dealloc it.
                // if old_sscratch != 0 {
                //     let old_vsi = StackVirtImpl::from_base(old_sscratch as *mut ());
                //     if !old_vsi.is_null() {
                //         let _old = stacks
                //             .set_current_stack(unsafe { &mut *old_vsi }, SMPVirtImpl::cpu_id());
                //         // info!("[trap_entry:sync] set_current_stack ok");
                //     }
                // }
                let current_stack = stacks.set_trap_stack(new_stack, cpu_id).unwrap();
                let _old = stacks.set_current_stack(current_stack, cpu_id);
                drop(stacks);
                let current_task = get_current_task();
                let prev_state = current_task.set_state(TaskState::Blocked);
                warn!(
                    "trap entry: current task {:#x}, state {:?} -> Blocked",
                    current_task as *const _ as usize, prev_state
                );
                push_prev_task(TaskState::Blocked);
                let scheduler =
                    unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) };
                // trap处理需要传入任务
                scheduler
                    .push_trap(
                        unsafe { &*TrapInfoVirtImpl::from_task(current_task.to_ptr()) },
                        Some(current_task),
                        cpu_id,
                    )
                    .unwrap();

                1
            } else if privilege == 1 {
                // let new_stack_base = STACK_HANDLER.lock().alloc_stack().base;
                // set_user_pre_stack!(new_stack_base);
                unimplemented!("user mode not supported yet");
                2
            } else {
                unreachable!("unknown privilege level: {privilege}")
            }
        }
        // 外部中断，将当前任务重新放回就绪态后进入对应调度器。
        1 => {
            if privilege == 0 {
                let cpu_id = SMPVirtImpl::cpu_id();
                let mut stacks = get_vvar_data!(KERNEL_STACKS).lock();
                // info!("[trap_entry:irq] old_sscratch={:#x}", old_sscratch);
                let new_stack = stacks.alloc_stack();
                set_pre_stack!(new_stack.base());
                // Recycle the old pre-save stack: set it as current_stack so
                // run_task / krun_utask will reuse or dealloc it.
                // if old_sscratch != 0 {
                //     let old_vsi = StackVirtImpl::from_base(old_sscratch as *mut ());
                //     if !old_vsi.is_null() {
                //         let _old = stacks
                //             .set_current_stack(unsafe { &mut *old_vsi }, SMPVirtImpl::cpu_id());
                //         // info!("[trap_entry:irq] set_current_stack ok");
                //     }
                // }
                let current_stack = stacks.set_trap_stack(new_stack, cpu_id).unwrap();
                let _old = stacks.set_current_stack(current_stack, cpu_id);
                drop(stacks);

                let current_task = get_current_task();
                let prev_state = current_task.set_state(TaskState::Ready);
                warn!(
                    "trap entry: current task {:#x}, state {:?} -> Ready",
                    current_task as *const _ as usize, prev_state
                );
                push_prev_task(TaskState::Ready);
                let scheduler =
                    unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) };
                // 外部中断处理不需要传入任务
                scheduler
                    .push_trap(
                        unsafe { &*TrapInfoVirtImpl::from_task(current_task.to_ptr()) },
                        None,
                        SMPVirtImpl::cpu_id(),
                    )
                    .unwrap();
                1
            } else if privilege == 1 {
                // let new_stack_base = STACK_HANDLER.lock().alloc_stack().base;
                // sset_user_pre_stack!(new_stack_base);
                let current_task = get_current_task();
                let prev_state = current_task.set_state(TaskState::Ready);
                warn!(
                    "trap entry: current task {:#x}, state {:?} -> Ready",
                    current_task as *const _ as usize, prev_state
                );
                push_prev_task(TaskState::Ready);
                2
            } else {
                unreachable!("unknown privilege level: {privilege}")
            }
        }
        // 从用户态调度器主动陷入内核，只需要继续进入内核侧调度。
        2 => 3,
        _ => unreachable!("unknown trap type: {trap_type}"),
    }
}

// /// 用户态同步trap入口
// ///
// /// 保存上下文，切换栈，进入`trap_handle`
// #[no_mangle]
// pub extern "C" fn utrap_entry(trap_type: usize) -> usize {
//     todo!()
// }

/// 从线程进入调度器的入口，也就是触发线程重新调度的函数
///
/// 先修改当前任务状态，再判断当前特权级并返回。
#[no_mangle]
pub extern "C" fn thread_entry() -> usize {
    assert_disable_irq("thread_entry");
    let current_task = get_current_task();
    match current_task.state() {
        TaskState::Blocking => {
            current_task.set_state(TaskState::Blocked);
            warn!(
                "thread entry: current task {:#x}, state Blocking -> Blocked",
                current_task as *const _ as usize
            );
            push_prev_task(TaskState::Blocked);
        }
        TaskState::Running => {
            current_task.set_state(TaskState::Ready);
            warn!(
                "thread entry: current task {:#x}, state Running -> Ready",
                current_task as *const _ as usize
            );
            push_prev_task(TaskState::Ready);
        }
        state => {
            warn!(
                "thread entry: current task {:#x}, state {:?}",
                current_task as *const _ as usize, state
            );
            push_prev_task(state);
        }
    }
    let in_kernel = get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()]
        .load(core::sync::atomic::Ordering::Acquire);
    if in_kernel {
        0
    } else {
        1
    }
}

// /// 同步trap处理函数
// ///
// /// 获取trap处理任务并传入当前上下文，设置trap处理任务为当前上下文，进入`run_task`
// #[no_mangle]
// pub extern "C" fn trap_handle() {
//     let current_task = get_current_task();
//     let handler_ptr = TrapInfoVirtImpl::get_handler(current_task.to_ptr());
//     assert!(!handler_ptr.is_null(), "Trap Handler should not be null");
//     let handler_task = unsafe { TaskVirtImpl::from_ptr(handler_ptr) };
//     handler_task.set_pid(current_task.pid());
//     // handler_task.set_state(TaskState::Running);
//     set_current_task(handler_task);
// }

/// 内核的调度与地址空间、特权级切换函数
///
/// 上一任务放回调度器，选定优先级最高的调度器，从调度器取出下一任务，切换到目标地址空间，进入`run_task`或`krun_utask`
///
/// 返回值：
///
/// - 0: 需要进入`run_task`
/// - 1: 需要进入`krun_utask`
#[no_mangle]
pub extern "C" fn kschedule() -> usize {
    let scheduler = unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) };
    let current_pid = scheduler.global_index();
    assert!(current_pid == 0);
    // push_prev_task();
    loop {
        let next_pid = process_schedule(scheduler);
        let res = ktask_schedule(next_pid);
        if res != 2 {
            break res;
        }
        // warn!("do not get task");
    }
}

/// 用户进程的调度与地址空间、特权级切换函数
///
/// 上一任务放回调度器，选定优先级最高的调度器，从调度器取出下一任务并进入`run_task`，或陷入内核
///
/// 参数：
///
/// - `stack_status`: 代表栈的状态，0为空栈，1为非空栈。
#[no_mangle]
pub extern "C" fn uschedule(stack_status: usize) {
    let scheduler = USER_SCHEDULER.get().unwrap();
    // push_prev_task();
    loop {
        let next_pid = process_schedule(scheduler);

        let res = utask_schedule(next_pid, stack_status);
        if res == 0 {
            break;
        }
    }
}

/// 从`uschedule`陷入内核后，执行的调度函数
///
/// 已选出优先级最高的调度器，从调度器取出下一任务，切换到目标地址空间，进入`run_task`或`krun_utask`
///
/// 返回值：
///
/// - 0: 需要进入`run_task`
/// - 1: 需要进入`krun_utask`
#[no_mangle]
pub extern "C" fn utok_schedule() -> usize {
    let mut next_pid =
        get_vvar_data!(CURRENT_VSPACE)[SMPVirtImpl::cpu_id()].load(Ordering::Acquire);
    loop {
        let res = ktask_schedule(next_pid);
        if res != 2 {
            break res;
        }

        let scheduler = unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) };
        warn!("after get scheduler");
        next_pid = process_schedule(scheduler);
        warn!("after process schedule");
    }
}

/// 切换地址空间，只会在内核态调用
fn switch_vspace(vspace_pid: usize) {
    let prev_vspace_pid =
        get_vvar_data!(CURRENT_VSPACE)[SMPVirtImpl::cpu_id()].swap(vspace_pid, Ordering::AcqRel);
    if vspace_pid != prev_vspace_pid {
        let vspace = get_vvar_data!(PROCESS_INFO_TABLE).table[vspace_pid]
            .vspace
            .load(Ordering::Acquire);
        if vspace.is_null() {
            // 代表下一进程可以在所有地址空间下运行，当前实现中只有单页表情况下的内核进程符合该条件，因此切换到该进程时不需要切换地址空间。
            return;
        }
        // 切换地址空间理论上不会影响代码的执行，因为此处的数据均位于内核子空间中，而切换只会影响到用户子空间。
        // 需要确认？
        unreachable!();
        unsafe { VSpaceVirtImpl::from_mut(vspace).into_vspace() };
    }
}

/// 根据上一任务（也就是已运行过的CURRENT_TASK）的状态，
/// 将上一任务放入对应的位置。
fn push_prev_task(state: TaskState) {
    match state {
        TaskState::Ready => {
            // Push to the task's own scheduler, not blindly to current_scheduler.
            // Kernel tasks (pid=0) go to KERNEL_SCHEDULER; user tasks go to their
            // process scheduler so they resume via krun_utask (not run_task).
            let current_task = get_current_task();
            let target = if current_task.is_kernel() {
                unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) }
            } else {
                let pid = current_task.pid();
                let scheduler_ptr = get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
                    .scheduler
                    .load(Ordering::Acquire);
                if scheduler_ptr.is_null() {
                    unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) }
                } else {
                    unsafe { &*scheduler_ptr }
                }
            };
            match target.push_task(current_task) {
                Ok(()) => (),
                Err(task) => {
                    panic!("Failed to push task back to scheduler: {:?}", task.to_ptr());
                }
            };
        }
        TaskState::Exited => {
            let current_task = get_current_task();
            current_task.dealloc();
        }
        _state => {}
    }
}

/// 1. 更新当前调度器最高优先级
/// 2. 选择优先级最高的进程
///
/// 返回值：下一个任务所在的进程id
fn process_schedule(current_scheduler: &Scheduler) -> usize {
    // let prio = current_scheduler.hightest_priority();
    let pid = current_scheduler.global_index();
    // get_vvar_data!(PROCESS_INFO_TABLE).table[pid]
    //     .highest_prio
    //     .store(prio, Ordering::Release);
    get_vvar_data!(PROCESS_INFO_TABLE).highest_prio_process(pid)
}

/// 仅内核态调用，决定了运行next_pid进程后的工作：
///
/// 1. 从调度器获取下一任务、更新调度器的最高优先级
/// 2. 设置CURRENT_VSPACE、切换地址空间（1、2的顺序可能反过来，取决于是用户调度器还是内核调度器）
/// 3. 设置下一任务状态为Running、设置CURRENT_TASK。
///
/// 返回值：
///
/// - 0：接下来调用run_task
/// - 1：接下来调用krun_utask
/// - 2：未获取到任务，需要重新获取任务后重新调用ktask_schedule。
fn ktask_schedule(next_pid: usize) -> usize {
    if next_pid == 0 {
        // 从当前调度器获取下一任务并运行
        let kscheduler = unsafe { &*get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire) };
        warn!("before pop task");
        if let (Some(next_task), new_prio) = kscheduler.pop_task() {
            // get_vvar_data!(PROCESS_INFO_TABLE).table[0]
            //     .highest_prio
            //     .store(new_prio, Ordering::Release);
            warn!("after pop task");
            switch_vspace(next_task.pid());
            // next_task.set_state(TaskState::Running);
            // warn!("after switch vspace");
            set_current_task(next_task);
            // warn!("after set current task");
            return 0; // 一定是内核态任务
        } else {
            warn!("after pop task=null");
            return 2;
        }
    } else {
        unreachable!();
        // 切换地址空间和调度器后获取下一任务并运行
        switch_vspace(next_pid);

        let uscheduler = unsafe { get_user_data(&USER_SCHEDULER, None) };
        if let (Some(next_task), new_prio) = uscheduler.pop_task() {
            // get_vvar_data!(PROCESS_INFO_TABLE).table[next_pid]
            //     .highest_prio
            //     .store(new_prio, Ordering::Release);
            // next_task.set_state(TaskState::Running);
            set_current_task(next_task);
            return 1; // 一定是用户态任务
        } else {
            return 2;
        }
    }
}

/// 仅用户态调用，决定了运行next_pid进程后的工作：
///
/// - 若next_pid == current_pid，则在当前调度器中选择优先级最高任务并运行
/// - 否则更新CURRENT_VSPACE变量、回收栈并进入内核。
///
/// 返回值：
///
/// - 0：接下来调用run_task
/// - 1：未获取到任务，需要重新获取任务后重新调用utask_schedule。
fn utask_schedule(next_pid: usize, stack_status: usize) -> usize {
    let uscheduler = USER_SCHEDULER.get().unwrap();
    let current_pid = uscheduler.global_index();
    if next_pid == current_pid {
        // 从当前调度器获取下一任务并运行
        if let (Some(next_task), new_prio) = uscheduler.pop_task() {
            // get_vvar_data!(PROCESS_INFO_TABLE).table[current_pid]
            //     .highest_prio
            //     .store(new_prio, Ordering::Release);
            // next_task.set_state(TaskState::Running);
            set_current_task(next_task);
            return 0;
        } else {
            return 1;
        }
    } else {
        // 更新CURRENT_VSPACE变量、回收栈并进入内核
        get_vvar_data!(CURRENT_VSPACE)[SMPVirtImpl::cpu_id()].store(next_pid, Ordering::Release);
        // todo: 检查栈切换是否会影响函数返回
        {
            let mut stack_handler = STACK_HANDLER.lock();
            stack_handler.get_thread_stack(None, stack_status);
        };
        ContextVirtImpl::into_kernel();
    }
}

/// 运行当前地址空间和特权级中的任务
///
/// 根据任务类型（线程或协程）切换或回收栈，并恢复上下文
///
/// 参数：
///
/// - privilege: 特权级
///     - 0: 内核态
///     - 1: 用户态
/// - `stack_status`: 代表栈的状态，0为空栈，1为非空栈。
///
/// 返回值（从`run_coroutine`中返回）：
///
/// - 特权级
///     - 0: 内核态
///     - 1: 用户态
///
/// 函数调用过程：
/// ```
/// raw_run_task
///     call run_task
///         ↓ 保存 ra（= li a1, 0）
///         coroutine_trampoline
///             mv sp, new_sp
///             mv ra, ret_addr
///             j run_coroutine
///                 ↓
///                 run_coroutine_inner()
///                 ↓
///                 asm!("ret")
/// → raw_run_task (li a1, 0)
/// ```
#[no_mangle]
pub extern "C" fn run_task(privilege: usize, stack_status: usize) -> usize {
    warn!("run task: {:#x}", get_current_task() as *const _ as usize);
    if get_current_task().is_coroutine() {
        // 切换或回收栈
        let new_sp = {
            let mut stack_handler = if privilege != 0 {
                STACK_HANDLER.lock()
            } else {
                get_vvar_data!(KERNEL_STACKS).lock()
            };
            stack_handler.get_empty_stack(stack_status)
        };
        // unsafe {
        //     core::arch::asm!("call coroutine_trampoline", in("a0") new_sp, in("a1") ret_addr, options(noreturn));
        // }
        jump_to_trampoline!(coroutine_trampoline, new_sp);
    } else {
        let thread_stack = get_current_task().thread_stack();
        {
            let mut stack_handler = if privilege != 0 {
                STACK_HANDLER.lock()
            } else {
                get_vvar_data!(KERNEL_STACKS).lock()
            };
            let thread_stack = unsafe { StackVirtImpl::from_mut(thread_stack) };
            stack_handler.get_thread_stack(Some(thread_stack), stack_status);
        };
        // unsafe {
        //     core::arch::asm!("call thread_trampoline", in("a0") thread_stack, in("a1") ret_addr, options(noreturn));
        // }
        // let thread_stack_base = unsafe { StackVirtImpl::from_mut(thread_stack) }.base();
        // jump_to_trampoline!(thread_trampoline, thread_stack_base);
        unsafe {
            run_thread();
        }
    }
    unreachable!();
}

/// 在内核态运行用户态任务
///
/// 根据任务类型（线程或协程）切换或回收栈，再返回用户态的`run_task`函数（用户协程）或线程上下文（用户线程）中
///
/// 参数：
///
/// - `stack_status`: 代表栈的状态，0为空栈，1为非空栈。
#[no_mangle]
pub extern "C" fn krun_utask(stack_status: usize) {
    if get_current_task().is_coroutine() {
        let user_sp = {
            let stack_handler = unsafe { get_user_data(&STACK_HANDLER, None) };
            let mut stack_handler = stack_handler.lock();
            stack_handler.get_empty_stack(stack_status)
        };
        {
            get_vvar_data!(KERNEL_STACKS)
                .lock()
                .get_thread_stack(None, stack_status);
        }
        unsafe {
            // 这里实际上没有发生换栈，换栈发生在into_user和into_user_context中，因此不需要跳板
            // core::arch::asm!("call coroutine_into_user_trampoline", in("a0") user_sp, options(noreturn));
            run_coroutine_into_user(user_sp);
        }
    } else {
        {
            get_vvar_data!(KERNEL_STACKS)
                .lock()
                .get_thread_stack(None, stack_status);
        }
        // get_current_task().set_state(TaskState::Running);
        unsafe {
            // core::arch::asm!("call coroutine_into_user_trampoline", in("a0") user_sp, options(noreturn));
            run_thread_into_user();
        }
    }
}

// 下面的两个函数我认为也属于调度循环的部分，也放在这里了。
// 跳板代码涉及到寄存器切换，不应该属于这里。

/// 运行协程
///
/// 返回值：
///
/// - 特权级
///     - 0: 内核态
///     - 1: 用户态
#[no_mangle]
pub(crate) unsafe extern "C" fn run_coroutine() -> usize {
    let current_task = get_current_task();
    current_task.set_state(TaskState::Running);
    assert_disable_irq("before run coroutine");
    let res = current_task.poll();
    // ************** 协程主动让权的入口 **************
    let state = current_task.state();
    if let Poll::Ready(val) = res {
        current_task.set_return_value(val);
        current_task.set_state(TaskState::Exited);
        warn!(
            "coroutine entry: current task {:#x}, state Poll::Ready -> Exited",
            current_task as *const _ as usize
        );
        push_prev_task(TaskState::Exited);
    } else if state == TaskState::Blocking || state == TaskState::Running {
        // 协程主动让权时，可能设置了任务状态也可能不设置。
        // 若设置了`Blocking`状态，则在此处改为`Blocked`状态。
        // 在不设置任务状态的情况，在此处设置为`Blocked`状态。
        current_task.set_state(TaskState::Blocked);
        warn!(
            "coroutine entry: current task {:#x}, state {:?} -> Blocked",
            current_task as *const _ as usize, state
        );
        push_prev_task(TaskState::Blocked);
    } else {
        warn!(
            "coroutine entry: current task {:#x}, state {:?}",
            current_task as *const _ as usize, state
        );
        push_prev_task(state);
    }
    let in_kernel = {
        // get_current_task().save_thread_context();
        get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].load(core::sync::atomic::Ordering::Acquire)
    };
    if in_kernel {
        0
    } else {
        1
    }
}

/// 运行线程
#[no_mangle]
pub(crate) unsafe extern "C" fn run_thread() -> ! {
    get_current_task().set_state(TaskState::Running);
    assert_disable_irq("before run thread");
    get_current_task().restore_context();
    unreachable!();
}

/// 从内核态运行用户态的协程
#[no_mangle]
pub(crate) unsafe extern "C" fn run_coroutine_into_user(user_sp: usize) -> ! {
    get_current_task().set_state(TaskState::Running);
    get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].store(false, Ordering::Release);
    ContextVirtImpl::into_user(user_sp);
    unreachable!();
}

/// 从内核态运行用户态的线程
#[no_mangle]
unsafe extern "C" fn run_thread_into_user() -> ! {
    get_current_task().set_state(TaskState::Running);
    get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].store(false, Ordering::Release);
    ContextVirtImpl::into_user_context(get_current_task() as *const TaskVirtImpl as *const ());
    unreachable!();
}
