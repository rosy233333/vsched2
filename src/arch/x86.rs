//! ## x86汇编简介
//!
//! ### 语法
//!
//! rust内联汇编使用指定了`.intel_syntax noprefix`的gas语法，在x86中即为intel语法。
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
//! |参数|调用者清理，该位置的底端（栈顶侧）以16字节对齐|
//! |返回地址||
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
#[cfg(target_arch = "x86_32")]
global_asm!(
    r#"
    .macro movx
        movl
    .endm
    .macro andx
        andl
    .endm
    .macro addx
        addl
    .endm
    .macro subx
        subl
    .endm
    .macro cmpx
        cmpl
    .endm
    .macro reg0
        di
    .endm
    .macro reg1
        si
    .endm
    # push_x_arg: 函数调用前的设置参数、平衡堆栈
    .macro push_0_arg
        subl sp, 12
    .endm
    .macro push_1_arg arg0
        subl sp, 8
        pushl \arg0
    .endm
    .macro push_2_arg arg0, arg1
        subl sp, 4
        pushl \arg1
        pushl \arg0
    .endm
    # push_x_arg: 函数调用后的平衡堆栈
    .macro pop_0_arg
        addl sp, 12
    .endm
    .macro pop_1_arg
        addl sp, 12
    .endm
    .macro pop_2_arg
        addl sp, 12
    .endm
    .macro XLEN
        4
    .endm
    "#,
);

/// 全局宏定义，用于兼容32位和64位的差异
#[cfg(target_arch = "x86_64")]
global_asm!(
    r#"
    .macro movx
        movq
    .endm
    .macro andx
        andq
    .endm
    .macro addx
        addq
    .endm
    .macro subx
        subq
    .endm
    .macro cmpx
        cmpq
    .endm
    .macro reg0
        r12
    .endm
    .macro reg1
        r13
    .endm
    # push_x_arg: 函数调用前的设置参数、平衡堆栈
    .macro push_0_arg
        subq sp, 8
    .endm
    .macro push_1_arg arg0
        subq sp, 8
        movq di, \arg0
    .endm
    .macro push_2_arg arg0, arg1
        subq sp, 8
        movq di, \arg0
        movq si, \arg1
    .endm
    # push_x_arg: 函数调用后的平衡堆栈
    .macro pop_0_arg
        addq sp, 8
    .endm
    .macro pop_1_arg
        addq sp, 8
    .endm
    .macro pop_2_arg
        addq sp, 8
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
        // 在call前，sp需要对齐到16字节。也就是说，存放返回地址的位置需要模16余16-XLEN。
        // 在放入返回地址之前，需要先使新的sp（存放返回地址的位置）满足对齐要求。
        core::arch::naked_asm!(r#"
            addx di, XLEN
            andx di, -16
            subx di, XLEN
            movx (di), si
            movx sp, di
            jmp {}
        "#, sym $f);
    };
}

/// 从第一个函数跳转到跳板的汇编代码。
///
/// 如果不需换栈，则new_sp为当前栈的栈底。
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

global_asm!(
    r#"
    .globl raw_trap_entry, raw_thread_entry, raw_run_task, raw_kschedule

    # 调度循环中使用的寄存器（均为callee-saved）及其含义：
    # - reg0（di（32位）/r12（64位））: 代表当前特权级，1为用户态，0为内核态。
    # - reg1（si（32位）/r13（64位））: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
    # 因为运行过程中可能换栈，因此不能用栈存储局部变量。
    # 从外部进入`schedule_loop`前，需要对齐sp并使用`call`进入。
    # `schedule_loop`执行的过程中，除了函数调用前后以外，sp都保持在“pop返回地址后对齐到16字节”的状态。
    schedule_loop:

    # `raw_trap_entry`为os发生trap、保存上下文并进行一定的解析后进入的入口。
    # 在x86中，需要用call进入`raw_trap_entry`，以保持sp的对齐。
    # os传递给调度器的参数：
    # - ax: trap类型
    #   - 0: 不是外部中断
    #   - 1: 外部中断
    #   - 2: 特殊参数的系统调用，仅用于“从用户态调度器进入内核”的情况。
    # - bx: 代表当前特权级，1为用户态，0为内核态。
    raw_trap_entry:
        movx reg0, bx
        movx reg1, 0
        # `trap_entry`为`schedule_loop.rs`中的rust函数。
        # 参数：
        # - \#1: trap类型，与os传入的参数格式相同。
        # - \#2: 代表当前特权级，1为用户态，0为内核态。
        # 返回值：
        # - ax: 下一步的跳转目标
        #   - 0: trap_handle
        #   - 1: kschedule
        #   - 2: uschedule
        #   - 3: utok_schedule
        push_2_arg ax, bx
        call trap_entry
        pop_2_arg
        cmpx ax, 0
        je raw_trap_handle
        cmpx ax, 1
        je raw_kschedule
        cmpx ax, 2
        je raw_uschedule
        cmpx ax, 3
        je raw_utok_schedule
        # 不可达
        .word 0xdeadbeef

    # `raw_thread_entry`为os进行线程主动让权，保存上下文后进入的入口。
    raw_thread_entry:
        movx reg1, 1
        # `thread_entry`为`schedule_loop.rs`中的rust函数。
        # 判断当前特权级后返回。
        # 返回值：
        # - ax: 当前特权级，决定下一步的跳转目标
        #   - 0: 内核态，跳转至kschedule
        #   - 1: 用户态，跳转至uschedule
        push_0_arg
        call thread_entry
        pop_0_arg
        movx reg0, ax
        cmpx ax, 0
        je raw_kschedule
        cmpx ax, 1
        je raw_uschedule
        # 不可达
        .word 0xdeadbeef

    raw_trap_handle:
        # `trap_handle`为`schedule_loop.rs`中的rust函数。
        push_0_arg
        call trap_handle
        pop_0_arg
        jmp raw_run_task
        # 不可达
        .word 0xdeadbeef

    # `raw_kschdule`为内核初始化时进入调度器的入口。
    # 进入时，需设置reg0=0, reg1=0，
    # 使用`call`进入`raw_kschdule`以对齐堆栈
    raw_kschedule:
        # `kschedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - ax: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        push_0_arg
        call kschedule
        pop_0_arg
        cmpx ax, 0
        je raw_run_task
        cmpx ax, 1
        je raw_krun_utask
        # 不可达
        .word 0xdeadbeef

    raw_uschedule:
        # `uschedule`为`schedule_loop.rs`中的rust函数。
        # 仅在下一任务在本进程中时，会从该函数返回。
        # 参数：
        # - ax: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg reg1
        call uschedule
        pop_1_arg
        jmp raw_run_task
        # 不可达
        .word 0xdeadbeef

    raw_utok_schedule:
        # `utok_schedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - ax: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        push_0_arg
        call utok_schedule
        pop_0_arg
        cmpx ax, 0
        je raw_run_task
        cmpx ax, 1
        je raw_krun_utask
        # 不可达
        .word 0xdeadbeef

    # `raw_run_task`为从内核态调度器返回用户态调度器时返回的pc。
    # 从内核返回用户态时，需要设置正确的s1和s2。
    #
    # 从`run_task`中返回后，需要重新设置s1和s2寄存器，因为`run_task`使用跳板切换了栈，再从另一个函数返回。
    # 此时，被调用者不再能可靠地保存s1和s2。
    # `uschedule`和`krun_utask`也涉及跳板换栈，但它们在换栈后一定不会返回，因此不需重新设置s1和s2。
    raw_run_task:
        # `run_task`为`schedule_loop.rs`中的rust函数。
        # 仅在运行协程时，会从该函数返回。
        # 参数：
        # - \#1: 代表当前特权级，1为用户态，0为内核态。
        # - \#2: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        # 返回值：
        # - ax: 特权级
        #     - 0: 内核态
        #     - 1: 用户态
        push_2_arg reg0, reg1
        call run_task
        pop_2_arg
        movx reg0, ax # 通过`run_task`（实际是`run_coroutine`）的返回值设置reg0
        movx reg1, 0 # 从`run_task`（实际是`run_coroutine`）中返回则一定是协程，因此是空栈
        cmpx reg0, 0
        je raw_kschedule
        cmpx reg0, 1
        je raw_uschedule
        # 不可达
        .word 0xdeadbeef

    raw_krun_utask:
        # `krun_utask`为`schedule_loop.rs`中的rust函数。
        # 不会从该函数返回。
        # 参数：
        # - \#1: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg reg1
        call krun_utask
        # 不可达
        .word 0xdeadbeef
        
"#
);
