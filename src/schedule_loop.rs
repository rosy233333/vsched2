//! 调度循环
//!
//! 结构详见：[调度循环结构图](https://github.com/rosy233333/weekly-progress/blob/dev/26.3.30~26.4.5/%E4%BB%BB%E5%8A%A1%E5%88%87%E6%8D%A2%E6%A8%A1%E5%9E%8B%EF%BC%88%E6%96%B0%E7%89%88%E8%B0%83%E5%BA%A6%E7%AE%97%E6%B3%95%EF%BC%89.png)
//!
//! 几个函数的运行顺序：`trap_entry` -> `run_task`，`schedule` -> `run_task`，之后在`schedule`和`run_task`中循环，使用`jmp`相互跳转实现循环

use vdso_helper::get_vvar_data;

use crate::{
    current::get_current_task,
    interface::{SMPVirtImpl, Task, SMP},
    jump,
};

/// 内核态同步trap入口
///
/// 保存上下文，切换栈，进入`trap_handle`
///
/// 特殊情况：
///
/// - 外部中断，进入`kschedule`
/// - 特殊的系统调用号，若是则进入`utok_schedule`
#[no_mangle]
pub extern "C" fn ktrap_entry() {
    todo!()
}

/// 用户态同步trap入口
///
/// 保存上下文，切换栈，进入`trap_handle`
#[no_mangle]
pub extern "C" fn utrap_entry() {
    todo!()
}

/// 从线程进入调度器的入口，也就是触发线程重新调度的函数
///
/// 保存上下文，进入`kschedule`或`uschedule`
#[no_mangle]
pub extern "C" fn thread_entry() {
    // 由于jump在语法上并未结束这个函数，函数内部的局部变量可能无法及时释放。
    //
    // 因此，需要将jump之前的代码用代码块包裹起来，仅将jump所需的判断条件以基本类型的形式传出代码块。
    let in_kernel = {
        // 该代码块为除跳转以外的函数主要逻辑
        get_current_task().save_thread_context();
        get_vvar_data!(IN_KERNEL)[SMPVirtImpl::cpu_id()].load(core::sync::atomic::Ordering::Acquire)
    };
    if in_kernel {
        // 不知道此时栈中是否有内容（例如in_kernel）
        jump!(kschedule);
    } else {
        jump!(uschedule);
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
#[no_mangle]
pub extern "C" fn kschedule() {
    todo!()
}

/// 用户进程的调度与地址空间、特权级切换函数
///
/// 上一任务放回调度器，选定优先级最高的调度器，从调度器取出下一任务并进入`run_task`，或陷入内核
#[no_mangle]
pub extern "C" fn uschedule() {
    todo!()
}

/// 从`uschedule`陷入内核后，执行的调度函数
///
/// 已选出优先级最高的调度器，从调度器取出下一任务，切换到目标地址空间，进入`run_task`或`krun_utask`
#[no_mangle]
pub extern "C" fn utok_schedule() {
    todo!()
}

/// 运行当前地址空间和特权级中的任务
///
/// 根据任务类型（线程或协程）切换或回收栈，并恢复上下文
#[no_mangle]
pub extern "C" fn run_task() {
    todo!()
}

/// 在内核态运行用户态任务
///
/// 根据任务类型（线程或协程）切换或回收栈，再返回用户态的`run_task`函数（用户协程）或线程上下文（用户线程）中
#[no_mangle]
pub extern "C" fn krun_utask() {
    todo!()
}
