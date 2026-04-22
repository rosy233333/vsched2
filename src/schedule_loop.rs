//! 调度循环
//!
//! 结构详见：[调度循环结构图](https://github.com/rosy233333/weekly-progress/blob/dev/26.3.30~26.4.5/%E4%BB%BB%E5%8A%A1%E5%88%87%E6%8D%A2%E6%A8%A1%E5%9E%8B%EF%BC%88%E6%96%B0%E7%89%88%E8%B0%83%E5%BA%A6%E7%AE%97%E6%B3%95%EF%BC%89.png)
//!
//! 几个函数的运行顺序：`trap_entry` -> `run_task`，`schedule` -> `run_task`，之后在`schedule`和`run_task`中循环，使用`jmp`相互跳转实现循环

use core::task::Poll;

use crate::{
    current::{get_current_task, STACK_HANDLER},
    interface::{Context, ContextVirtImpl, SMPVirtImpl, Task, TaskState, TaskVirtImpl, SMP},
    set_pre_stack, set_user_pre_stack,
    jump_to_trampoline,
};
use vdso_helper::get_vvar_data;

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
/// - privilege: 特权级
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
    match trap_type {
        // 普通同步 trap，进入 trap 处理流程。
        0 => {
            if privilege == 0 {
                let new_stack_base = get_vvar_data!(KERNEL_STACKS).lock().alloc_stack().base;
                set_pre_stack!(new_stack_base);
            } else if privilege == 1 {
                // let new_stack_base = STACK_HANDLER.lock().alloc_stack().base;
                // set_user_pre_stack!(new_stack_base);
                unimplemented!("user mode not supported yet")
            } else {
                unreachable!("unknown privilege level: {privilege}")
            }
            0
        }
        // 外部中断，将当前任务重新放回就绪态后进入对应调度器。
        1 => {
            get_current_task().set_state(TaskState::Ready);
            if privilege == 0 {
                let new_stack_base = get_vvar_data!(KERNEL_STACKS).lock().alloc_stack().base;
                set_pre_stack!(new_stack_base);
                1
            } else if privilege == 1 {
                // let new_stack_base = STACK_HANDLER.lock().alloc_stack().base;
                // sset_user_pre_stack!(new_stack_base);
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
/// 目前该函数只有判断当前特权级并返回的功能。
#[no_mangle]
pub extern "C" fn thread_entry() -> usize {
    // // 由于jump在语法上并未结束这个函数，函数内部的局部变量可能无法及时释放。
    // //
    // // 因此，需要将jump之前的代码用代码块包裹起来，仅将jump所需的判断条件以基本类型的形式传出代码块。
    // let in_kernel = {
    //     // 该代码块为除跳转以外的函数主要逻辑
    //     get_current_task().save_thread_context();
    //     get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].load(core::sync::atomic::Ordering::Acquire)
    // };
    // if in_kernel {
    //     reset_stack_and_jump!(kschedule);
    // } else {
    //     reset_stack_and_jump!(uschedule);
    // }
    let in_kernel = get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()]
        .load(core::sync::atomic::Ordering::Acquire);
    if in_kernel {
        0
    } else {
        1
    }
}

/// 同步trap处理函数
///
/// 获取trap处理任务并传入当前上下文，设置trap处理任务为当前上下文，进入`run_task`
#[no_mangle]
pub extern "C" fn trap_handle() {
    todo!()
}

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
    todo!()
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
    todo!()
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
    todo!()
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
    let in_kernel = privilege == 0;
    // let ret_addr: usize;
    // unsafe {
    //     core::arch::asm!("mv {}, ra", out(reg) ret_addr);
    // }
    if get_current_task().is_coroutine() {
        // 切换或回收栈
        let new_sp = {
            let mut stack_handler = if privilege == 0 {
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
        let thread_stack = { get_current_task().thread_stack_base() };
        {
            let mut stack_handler = if privilege == 0 {
                STACK_HANDLER.lock()
            } else {
                get_vvar_data!(KERNEL_STACKS).lock()
            };
            stack_handler.get_thread_stack(Some(thread_stack.into()), stack_status);
        };
        // unsafe {
        //     core::arch::asm!("call thread_trampoline", in("a0") thread_stack, in("a1") ret_addr, options(noreturn));
        // }
        jump_to_trampoline!(thread_trampoline, thread_stack);
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
            let mut stack_handler = STACK_HANDLER.lock();
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
        get_current_task().set_state(TaskState::Running);
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
unsafe extern "C" fn run_coroutine() -> usize {
    get_current_task().set_state(TaskState::Running);
    let res = get_current_task().poll();
    // ************** 协程主动让权的入口 **************
    match res {
        Poll::Ready(val) => {
            // todo：val怎么处理？task里是否需要一个设置返回值的接口？
            get_current_task().set_state(TaskState::Exited);
        }
        Poll::Pending => {
            // todo：这里也有可能是 Ready 状态，需要后续实现中再修改
            get_current_task().set_state(TaskState::Blocked);
        }
    }
    let in_kernel = {
        get_current_task().save_thread_context();
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
unsafe extern "C" fn run_thread() -> ! {
    get_current_task().set_state(TaskState::Running);
    get_current_task().restore_context();
    unreachable!();
}

/// 从内核态运行用户态的协程
#[no_mangle]
unsafe extern "C" fn run_coroutine_into_user(user_sp: usize) -> ! {
    // todo：把user_sp传入into_user里
    ContextVirtImpl::into_user();
    unreachable!();
}

/// 从内核态运行用户态的线程
#[no_mangle]
unsafe extern "C" fn run_thread_into_user() -> ! {
    ContextVirtImpl::into_user_context(get_current_task() as *const TaskVirtImpl as *const ());
    unreachable!();
}
