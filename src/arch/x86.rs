//! ## x86汇编简介
//!
//! ### 调用约定
//!
//! 32位使用cdecl，64位使用sysv64
//!
//! - cdecl：参数在栈中传递，从右到左入栈。返回后由被调用者清理栈上的参数。返回值为eax。
//! - sysv64：前6个参数通过rdi、rsi、rdx、rcx、r8、r9传递，其余参数在栈中传递，从右到左入栈。返回值为rax。
//!
//! ### call和ret
//!
//! call将下一条指令的值推入栈中并跳转；ret从栈中弹出跳转地址并跳转。在call之前，sp需要以16字节对齐。
//!
//! ### 栈帧结构与维护（上方为高地址，下方为低地址）
//!
//! |栈帧内容|备注|
//! |-|-|
//! |参数|调用者清理|
//! |返回地址|该位置以16字节对齐|
//! |保存的bp|可选；若启用栈帧指针，则当前bp指向该位置|
//! |局部变量||
//! |保存的其它寄存器|当前sp指向该区域的底端（栈顶侧）|
//! |未分配区域/红区|sysv64下有红区的概念，这部分内存不会被信号和中断处理函数覆写。因此叶函数可以使用红区存储局部变量，省略了移动sp分配空间的过程。|
//!
//! 调用过程：（调用者）参数压栈 -> （调用者）call -> （被调用者）保存和设置bp -> （被调用者）下移sp以分配空间 -> （被调用者）寄存器压栈
//!
//! 返回过程：（被调用者）弹出寄存器 -> （被调用者）将sp赋值为bp -> （被调用者）弹出bp -> （被调用者）ret -> （调用者）上移sp以清理参数

use core::arch::global_asm;

/// 全局宏定义，用于兼容32位和64位的差异
global_asm!(
    #[cfg(target_arch = "x86_32")]
    r#"
    .macro movx
        movl
    .endm
    .macro andx
        andl
    .endm
    .macro XLEN
        4
    .endm
    "#,
    #[cfg(target_arch = "x86_64")]
    r#"
    .macro movx
        movq
    .endm
    .macro andx
        andq
    .endm
    .macro XLEN
        8
    .endm
    "#,
);

/// 切换栈时，为了避免未定义行为，不能在同一个函数中切换栈，
/// 因此使用完全用汇编写成的裸函数切换栈，并将其之前和之后的代码写在两个函数中。
///
/// 切换过程中的状态维护：
///
/// - sp:
///     - 从上一函数jmp到跳板时不变
///     - 在跳板中按照需求切换到另一个栈
///     - 在j到下一个函数后，在下一个函数的prologue中下移以分配下一个函数的栈帧
///     - 在从下一个函数返回前重新设置为切换到的栈底
/// - bp:
///     - 在第一个函数中，bp的原值被保存在栈上，bp指向bp原值被保存的位置
///     - 在jmp过程和跳板中保持不变
///     - 在jmp到下一个函数后，正常地保存、恢复和使用bp。在下一个函数执行期间，bp会指向下一个函数的栈帧的起始位置
///     - 最终回到`schedule_loop`函数时，bp指向第一个函数栈帧的起始位置。不过`schedule_loop`并不使用bp，因此是符合要求的
/// - ip（指令指针）:
///     - 进入上一函数后，上一函数的返回地址被保存在上一个函数栈中
///     - 在跳板中，通过中转寄存器将上一函数的返回地址保存到新sp的位置，再切换sp
///     - 在栈上已保存返回地址的情况下jmp到下一个函数，相当于call下一个函数
///     - 在下一个函数返回时，从栈上获取返回地址并返回到`schedule_loop`中
#[macro_export]
macro_rules! switch_sp_tratrampoline {
    ($f:ident) => {
        // di: 新的sp；si: 返回地址（无论32位或64位）
        // 在放入返回地址之前，需要先将新的sp对齐到16字节。
        core::arch::naked_asm!(r#"
            andx di, -16
            movx (di), si
            movx sp, di
            jmp {}
        "#, sym $f);
    };
}

/// 从第一个函数跳转到跳板的汇编代码。
///
/// 详见`switch_sp_trampoline`的注释。
#[macro_export]
macro_rules! jump_to_trampoline {
    ($trampoline_fn:ident, $new_sp:ident) => {
        unsafe {
            // di: 新的sp；si: 返回地址（无论32位或64位）
            core::arch::asm!(r#"
                movx si, XLEN(bp)
                jmp {}
            "#, sym $trampoline_fn, in("di") $new_sp, options(noreturn))
        }
    };
}
