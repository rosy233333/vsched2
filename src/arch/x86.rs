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

// /// 全局宏定义，用于兼容32位和64位的差异
// #[cfg(target_arch = "x86")]
// global_asm!(
//     r#"
//     .macro movx arg1, arg2
//         movl \arg1, \arg2
//     .endm
//     .macro andx arg1, arg2
//         andl \arg1, \arg2
//     .endm
//     .macro addx arg1, arg2
//         addl \arg1, \arg2
//     .endm
//     .macro subx arg1, arg2
//         subl \arg1, \arg2
//     .endm
//     .macro cmpx arg1, arg2
//         cmpl \arg1, \arg2
//     .endm
//     .macro reg0
//         di
//     .endm
//     .macro reg1
//         si
//     .endm
//     # push_x_arg: 函数调用前的设置参数、平衡堆栈
//     .macro push_0_arg
//         subl sp, 12
//     .endm
//     .macro push_1_arg arg1
//         subl sp, 8
//         pushl \arg1
//     .endm
//     .macro push_2_arg arg1, arg2
//         subl sp, 4
//         pushl \arg2
//         pushl \arg1
//     .endm
//     # push_x_arg: 函数调用后的平衡堆栈
//     .macro pop_0_arg
//         addl sp, 12
//     .endm
//     .macro pop_1_arg
//         addl sp, 12
//     .endm
//     .macro pop_2_arg
//         addl sp, 12
//     .endm
//     .set XLEN, 4
//     "#,
// );

// /// 全局宏定义，用于兼容32位和64位的差异
// #[cfg(target_arch = "x86_64")]
// global_asm!(
//     r#"
//     .macro movx arg1, arg2
//         movq \arg1, \arg2
//     .endm
//     .macro andx arg1, arg2
//         andq \arg1, \arg2
//     .endm
//     .macro addx arg1, arg2
//         addq \arg1, \arg2
//     .endm
//     .macro subx arg1, arg2
//         subq \arg1, \arg2
//     .endm
//     .macro cmpx arg1, arg2
//         cmpq \arg1, \arg2
//     .endm
//     .macro reg0
//         r12
//     .endm
//     .macro reg1
//         r13
//     .endm
//     # push_x_arg: 函数调用前的设置参数、平衡堆栈
//     .macro push_0_arg
//         subq sp, 8
//     .endm
//     .macro push_1_arg arg1
//         subq sp, 8
//         movq di, \arg1
//     .endm
//     .macro push_2_arg arg1, arg2
//         subq sp, 8
//         movq di, \arg1
//         movq si, \arg2
//     .endm
//     # push_x_arg: 函数调用后的平衡堆栈
//     .macro pop_0_arg
//         addq sp, 8
//     .endm
//     .macro pop_1_arg
//         addq sp, 8
//     .endm
//     .macro pop_2_arg
//         addq sp, 8
//     .endm
//     .set XLEN, 8
//     "#,
// );

/// 函数调用前后的准备工作
#[cfg(target_arch = "x86")]
global_asm!(
    r#"
    # push_x_arg: 函数调用前的设置参数、平衡堆栈
    .macro push_0_arg
        sub esp, 12
    .endm
    .macro push_1_arg arg1
        sub esp, 8
        push \arg1
    .endm
    .macro push_2_arg arg1, arg2
        sub esp, 4
        push \arg2
        push \arg1
    .endm
    # push_x_arg: 函数调用后的平衡堆栈
    .macro pop_0_arg
        add esp, 12
    .endm
    .macro pop_1_arg
        add esp, 12
    .endm
    .macro pop_2_arg
        add esp, 12
    .endm
    "#,
);

/// 函数调用前后的准备工作
#[cfg(target_arch = "x86_64")]
global_asm!(
    r#"
    # push_x_arg: 函数调用前的设置参数、平衡堆栈
    .macro push_0_arg
        sub rsp, 8
    .endm
    .macro push_1_arg arg1
        sub rsp, 8
        mov rdi, \arg1
    .endm
    .macro push_2_arg arg1, arg2
        sub rsp, 8
        mov rdi, \arg1
        mov rsi, \arg2
    .endm
    # push_x_arg: 函数调用后的平衡堆栈
    .macro pop_0_arg
        add rsp, 8
    .endm
    .macro pop_1_arg
        add rsp, 8
    .endm
    .macro pop_2_arg
        add rsp, 8
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
#[cfg(target_arch = "x86")]
#[macro_export]
macro_rules! switch_sp_tratrampoline {
    ($f:ident) => {
        // di: 新的sp；si: 返回地址（无论32位或64位）
        // 在call前，sp需要对齐到16字节。也就是说，存放返回地址的位置需要模16余16-XLEN。
        // 在放入返回地址之前，需要先使新的sp（存放返回地址的位置）满足对齐要求。
        core::arch::naked_asm!(r#"
            add edi, 4
            and edi, -16
            sub edi, 4
            mov [edi], esi
            mov esp, edi
            jmp {}
        "#, sym $f)
    };
}

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
#[cfg(target_arch = "x86_64")]
#[macro_export]
macro_rules! switch_sp_tratrampoline {
    ($f:ident) => {
        // di: 新的sp；si: 返回地址（无论32位或64位）
        // 在call前，sp需要对齐到16字节。也就是说，存放返回地址的位置需要模16余16-XLEN。
        // 在放入返回地址之前，需要先使新的sp（存放返回地址的位置）满足对齐要求。
        core::arch::naked_asm!(r#"
            add rdi, 8
            and rdi, -16
            sub rdi, 8
            mov [rdi], rsi
            mov rsp, rdi
            jmp {}
        "#, sym $f)
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
        #[cfg(target_arch = "x86")]
        unsafe {
            // di: 新的sp；si: 返回地址（无论32位或64位）
            core::arch::asm!(r#"
                mov esi, [ebp+4]
                jmp {}
            "#, sym $trampoline_fn, in("edi") $new_sp, options(noreturn))
        }
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // di: 新的sp；si: 返回地址（无论32位或64位）
            core::arch::asm!(r#"
                mov rsi, [rbp+8]
                jmp {}
            "#, sym $trampoline_fn, in("rdi") $new_sp, options(noreturn))
        }
    };
}

/// 获取sp寄存器的值。
#[macro_export]
macro_rules! get_sp {
    () => {
        unsafe {
            let sp: usize;
            #[cfg(target_arch = "x86")]
            core::arch::asm!("
                mov {}, esp
            ", out(reg) sp, options(nostack));
            #[cfg(target_arch = "x86_64")]
            core::arch::asm!("
                mov {}, rsp
            ", out(reg) sp, options(nostack));
            sp
        }
    };
}

/// 设置新的sscratch寄存器的值。
#[macro_export]
macro_rules! set_pre_stack {
    ($f:expr) => {
        todo!();
        // unsafe {
        //     core::arch::asm!("
        //         csrw sscratch, {}
        //     ", in(reg) $f);
        // }
    };
}

#[cfg(target_arch = "x86")]
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
    # - bx: 代表（trap后的）当前特权级，1为用户态，0为内核态。
    raw_trap_entry:
        mov edi, ebx
        mov esi, 0
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
        push_2_arg eax, ebx
        call trap_entry
        pop_2_arg
        cmp eax, 0
        je raw_trap_handle
        cmp eax, 1
        je raw_kschedule
        cmp eax, 2
        je raw_uschedule
        cmp eax, 3
        je raw_utok_schedule
        # 不可达
        .long 0xdeadbeef

    # `raw_thread_entry`为os进行线程主动让权，保存上下文后进入的入口。
    raw_thread_entry:
        # `thread_entry`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - 通过第0和第1位分别存储当前特权级（1为用户态，0为内核态）和栈状态（0为空栈，1为非空栈）
        #   - 同时，特权级也决定了下一步的跳转目标。
        #     - 0: 内核态，跳转至kschedule
        #     - 1: 用户态，跳转至uschedule
        push_0_arg
        call thread_entry
        pop_0_arg
        and edi, rax, 1
        shr esi, rax, 1
        cmp edi, 0
        je raw_kschedule
        cmp edi, 1
        je raw_uschedule
        # 不可达
        .long 0xdeadbeef

    # raw_trap_handle:
    #     # `trap_handle`为`schedule_loop.rs`中的rust函数。
    #     push_0_arg
    #     call trap_handle
    #     pop_0_arg
    #     jmp raw_run_task
    #     # 不可达
    #     .long 0xdeadbeef

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
        cmp eax, 0
        je raw_run_task
        cmp eax, 1
        je raw_krun_utask
        # 不可达
        .long 0xdeadbeef

    raw_uschedule:
        # `uschedule`为`schedule_loop.rs`中的rust函数。
        # 仅在下一任务在本进程中时，会从该函数返回。
        # 参数：
        # - ax: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg esi
        call uschedule
        pop_1_arg
        jmp raw_run_task
        # 不可达
        .long 0xdeadbeef

    raw_utok_schedule:
        # `utok_schedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - ax: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        push_0_arg
        call utok_schedule
        pop_0_arg
        cmp eax, 0
        je raw_run_task
        cmp eax, 1
        je raw_krun_utask
        # 不可达
        .long 0xdeadbeef

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
        # - ax: 代表当前特权级，1为用户态，0为内核态。
        # - bx: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        # 返回值：
        # - ax: 特权级
        #     - 0: 内核态
        #     - 1: 用户态
        push_2_arg edi, esi
        call run_task
        pop_2_arg
        mov edi, eax # 通过`run_task`（实际是`run_coroutine`）的返回值设置reg0
        mov esi, 0 # 从`run_task`（实际是`run_coroutine`）中返回则一定是协程，因此是空栈
        cmp edi, 0
        je raw_kschedule
        cmp edi, 1
        je raw_uschedule
        # 不可达
        .long 0xdeadbeef

    raw_krun_utask:
        # `krun_utask`为`schedule_loop.rs`中的rust函数。
        # 不会从该函数返回。
        # 参数：
        # - \#1: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg esi
        call krun_utask
        # 不可达
        .long 0xdeadbeef    
"#
);

#[cfg(target_arch = "x86_64")]
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
        mov r12, rbx
        mov r13, 0
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
        push_2_arg rax, rbx
        call trap_entry
        pop_2_arg
        cmp rax, 0
        je raw_trap_handle
        cmp rax, 1
        je raw_kschedule
        cmp rax, 2
        je raw_uschedule
        cmp rax, 3
        je raw_utok_schedule
        # 不可达
        .long 0xdeadbeef

    # `raw_thread_entry`为os进行线程主动让权，保存上下文后进入的入口。
    raw_thread_entry:
        # `thread_entry`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - 通过第0和第1位分别存储当前特权级（1为用户态，0为内核态）和栈状态（0为空栈，1为非空栈）
        #   - 同时，特权级也决定了下一步的跳转目标。
        #     - 0: 内核态，跳转至kschedule
        #     - 1: 用户态，跳转至uschedule
        push_0_arg
        call thread_entry
        pop_0_arg
        and r12, rax, 1
        shr r13, rax, 1
        cmp r12, 0
        je raw_kschedule
        cmp r12, 1
        je raw_uschedule
        # 不可达
        .long 0xdeadbeef

    raw_trap_handle:
        # `trap_handle`为`schedule_loop.rs`中的rust函数。
        push_0_arg
        call trap_handle
        pop_0_arg
        jmp raw_run_task
        # 不可达
        .long 0xdeadbeef

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
        cmp rax, 0
        je raw_run_task
        cmp rax, 1
        je raw_krun_utask
        # 不可达
        .long 0xdeadbeef

    raw_uschedule:
        # `uschedule`为`schedule_loop.rs`中的rust函数。
        # 仅在下一任务在本进程中时，会从该函数返回。
        # 参数：
        # - ax: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg r13
        call uschedule
        pop_1_arg
        jmp raw_run_task
        # 不可达
        .long 0xdeadbeef

    raw_utok_schedule:
        # `utok_schedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - ax: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        push_0_arg
        call utok_schedule
        pop_0_arg
        cmp rax, 0
        je raw_run_task
        cmp rax, 1
        je raw_krun_utask
        # 不可达
        .long 0xdeadbeef

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
        # - ax: 代表当前特权级，1为用户态，0为内核态。
        # - bx: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        # 返回值：
        # - ax: 特权级
        #     - 0: 内核态
        #     - 1: 用户态
        push_2_arg r12, r13
        call run_task
        pop_2_arg
        mov r12, rax # 通过`run_task`（实际是`run_coroutine`）的返回值设置reg0
        mov r13, 0 # 从`run_task`（实际是`run_coroutine`）中返回则一定是协程，因此是空栈
        cmp r12, 0
        je raw_kschedule
        cmp r12, 1
        je raw_uschedule
        # 不可达
        .long 0xdeadbeef

    raw_krun_utask:
        # `krun_utask`为`schedule_loop.rs`中的rust函数。
        # 不会从该函数返回。
        # 参数：
        # - ax: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        push_1_arg r13
        call krun_utask
        # 不可达
        .long 0xdeadbeef      
"#
);
