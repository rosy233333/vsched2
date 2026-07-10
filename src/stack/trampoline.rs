use crate::{
    get_sp,
    main_loop::{run_coroutine, run_thread, thread_entry_phase2},
    switch_sp_tratrampoline,
};

/// 获取栈类型，即是否是空栈
///
/// 如果返回 0，表示是空栈，否则表示栈使用的大小
///
/// TODO: 这里需要后续修改，用sp或fp判断栈类型不对（由于函数调用和局部变量，一定不等于栈顶）
#[inline(always)]
#[allow(dead_code)] // 之后再看要不要删掉
fn get_stack_type(stack_base: usize) -> usize {
    get_sp!() - stack_base
}

#[no_mangle]
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn coroutine_trampoline() -> ! {
    switch_sp_tratrampoline!(run_coroutine)
}

#[no_mangle]
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn thread_trampoline() -> ! {
    switch_sp_tratrampoline!(run_thread)
}

#[no_mangle]
#[unsafe(naked)]
pub(crate) unsafe extern "C" fn tep2_trampoline() -> ! {
    switch_sp_tratrampoline!(thread_entry_phase2)
}

// #[no_mangle]
// #[unsafe(naked)]
// unsafe extern "C" fn coroutine_into_user_trampoline() -> ! {
//     switch_sp_tratrampoline!(run_coroutine_into_user)
// }
