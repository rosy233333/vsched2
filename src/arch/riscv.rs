//! ## risc-v汇编简介
//!
//! ### 调用约定
//!
//! 使用risc-v abi。前8个参数通过a0~a7传递，其余参数在栈中传递，从右到左入栈。返回值为a0。
//!
//! ### call和ret
//!
//! call将下一条指令的值保存在ra中并跳转；ret即为跳转到ra存储的地址。在call之前，sp需要以16字节对齐。
//!
//! ### 栈帧结构与维护（上方为高地址，下方为低地址）
//!
//! |栈帧内容|备注|
//! |-|-|
//! |参数|调用者清理；若启用栈帧指针，则当前fp指向该区域的底端（栈顶侧）|
//! |保存的ra|可选；叶函数可不保存ra|
//! |保存的fp|可选|
//! |保存的其它寄存器||
//! |局部变量|当前sp指向该区域的底端（栈顶侧）|
//! |未分配区域||
//!
//! 调用过程：（调用者）参数压栈 -> （调用者）call -> （被调用者）下移sp以分配空间 -> （被调用者）保存ra和fp -> （被调用者）保存其它寄存器
//!
//! 返回过程：（被调用者）恢复ra、fp和其它寄存器 -> （被调用者）上移sp以清理栈帧 -> （被调用者）ret -> （调用者）上移sp以清理参数

use core::arch::global_asm;

// 全局宏定义，用于兼容32位和64位的差异
#[cfg(target_arch = "riscv32")]
global_asm!(
    r#"
    .macro lx
        lw
    .endm
    .macro XLEN
        4
    .endm
    "#,
);

// 全局宏定义，用于兼容32位和64位的差异
#[cfg(target_arch = "riscv64")]
global_asm!(
    r#"
    .macro lx
        ld
    .endm
    .macro XLEN
        8
    .endm
    "#,
);

/// 封装跳转指令以适配不同架构。
///
/// 跳转指令用于实现调度循环中各函数的切换。
///
/// 在跳转前，先要重置sp寄存器到函数调用前的位置，相当于释放当前函数的栈帧。
///
/// 在函数a的最后`reset_sp_and_jump`到函数b，相当于先后调用了函数a和函数b。
///
/// risc-v架构：栈帧范围为(fp（高地址）, sp（低地址）]，
/// 但ra和fp的先前值分别存放在了(fp-8)和(fp-16)处（64位）或(fp-4)和(fp-8)处（32位），空出了(fp)的位置。
/// 暂不清楚原因，并且在运行时需要确认编译出的函数是否有如此的行为。
///
/// TODO: 检查是否符合内联汇编的规则，是否会出现未定义行为。
#[macro_export]
macro_rules! reset_stack_and_jump {
    ($f:ident) => {
        unsafe{
            #[cfg(target_arch="riscv32")]
            core::arch::asm!("
                mv sp, fp
                lw ra, -4(fp)
                lw fp, -8(fp)
                j {}
            ", sym $f);
            #[cfg(target_arch="riscv64")]
            core::arch::asm!("
                mv sp, fp
                ld ra, -8(fp)
                ld fp, -16(fp)
                j {}
            ", sym $f);
        }
    };
}

/// 设置sp寄存器的值。
#[macro_export]
macro_rules! set_sp {
    ($f:ident) => {
        unsafe {
            core::arch::asm!("
                mv sp, {}
            ", in(reg) $f, options(nostack));
        }
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
/// - ra（需要保持：进入第一个函数时的ra==退出第二个函数时的ra）:
///     - 若第一个函数为非叶函数（需要保证这点），则ra的原值被保存到栈上
///     - （需要修改为这样）在jmp之前，需要将栈上的值恢复到ra寄存器中
///     - 从上一函数jmp到跳板时不变
///     - 在jmp到下一个函数后，因为下一个函数会完整执行，因此可以进行ra的保存与恢复
/// - fp:
///     - 在第一个函数中，fp的原值被保存在栈上，fp指向第一个函数栈帧的起始位置
///     - 在jmp过程和跳板中保持不变
///     - 在jmp到下一个函数后，正常地保存、恢复和使用fp
///     - 最终回到`schedule_loop`函数时，fp指向第一个函数栈帧的起始位置。不过`schedule_loop`并不使用fp，因此是符合要求的
/// - pc: 在维护了ra的前提下，涉及跳板的两次跳转与下一个函数的返回均可正常切换控制流。
#[macro_export]
macro_rules! switch_sp_tratrampoline {
    ($f:ident) => {
        // a0: 新的sp
        // ra: 已恢复为上一个函数的ra
        core::arch::naked_asm!(r#"
            mv sp, a0
            j {}
        "#, sym $f) // 这里我的编译器（2025-12-12）提示不能加分号，否则报错：
                    // “railing semicolon in macro used in expression position.
                    // this was previously accepted by the compiler but is being phased out; it will become a hard error in a future release!”
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
            // a0: 新的sp；ra: 返回地址
            core::arch::asm!(r#"
                lx ra, -XLEN(fp)
                j {}
            "#, sym $trampoline_fn, in("a0") $new_sp, options(noreturn))
        }
    };
}

/// 获取sp寄存器的值。
#[macro_export]
macro_rules! get_sp {
    () => {
        unsafe {
            let sp: usize;
            core::arch::asm!("
                mv {}, sp
            ", out(reg) sp, options(nostack));
            sp
        }
    };
}

/// 设置新的sscratch寄存器的值。
#[macro_export]
macro_rules! set_pre_stack {
    ($f:expr) => {
        unsafe {
            core::arch::asm!("
                csrw sscratch, {}
            ", in(reg) $f);
        }
    };
}

// TODO：？
/// 设置新的uscratch寄存器的值。
#[macro_export]
macro_rules! set_user_pre_stack {
    ($f:expr) => {
        // unsafe {
        // core::arch::asm!("
        //     csrw uscratch, {}
        // ", in(reg) $f);
        // }
    };
}

global_asm!(
    r#"
    .globl raw_trap_entry, raw_thread_entry, raw_run_task, raw_kschedule

    # 调度循环中使用的寄存器（均为callee-saved）及其含义：
    # - s1: 代表当前特权级，1为用户态，0为内核态。
    # - s2: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
    # 因为运行过程中可能换栈，因此不能用栈存储局部变量。
    schedule_loop:

    # `raw_trap_entry`为os发生trap、保存上下文并进行一定的解析后进入的入口。
    # os传递给调度器的参数：
    # - a0: trap类型
    #   - 0: 不是外部中断
    #   - 1: 外部中断
    #   - 2: 特殊参数的系统调用，仅用于“从用户态调度器进入内核”的情况。
    # - a1: 代表当前特权级，1为用户态，0为内核态。
    raw_trap_entry:
        mv s1, a1
        li s2, 0
        # `trap_entry`为`schedule_loop.rs`中的rust函数。
        # 参数：
        # - a0: trap类型，与os传入的参数格式相同。
        # - a1: 代表当前特权级，1为用户态，0为内核态。
        # 返回值：
        # - a0: 下一步的跳转目标
        #   - 0: trap_handle
        #   - 1: kschedule
        #   - 2: uschedule
        #   - 3: utok_schedule
        call trap_entry
        li a1, 0
        beq a0, a1, raw_trap_handle
        li a1, 1
        beq a0, a1, raw_kschedule
        li a1, 2
        beq a0, a1, raw_uschedule
        li a1, 3
        beq a0, a1, raw_utok_schedule
        # 不可达
        .word 0xdeadbeef

    # `raw_thread_entry`为os进行线程主动让权，保存上下文后进入的入口。
    raw_thread_entry:
        li s2, 1
        # `thread_entry`为`schedule_loop.rs`中的rust函数。
        # 判断当前特权级后返回。
        # 返回值：
        # - a0: 当前特权级，决定下一步的跳转目标
        #   - 0: 内核态，跳转至kschedule
        #   - 1: 用户态，跳转至uschedule
        call thread_entry
        mv s1, a0
        li a1, 0
        beq a0, a1, raw_kschedule
        li a1, 1
        beq a0, a1, raw_uschedule
        # 不可达
        .word 0xdeadbeef

    raw_trap_handle:
        # `trap_handle`为`schedule_loop.rs`中的rust函数。
        call trap_handle
        j raw_run_task
        # 不可达
        .word 0xdeadbeef

    # `raw_kschdule`为内核初始化时进入调度器的入口。
    # 进入时，需设置s1=0, s2=0
    raw_kschedule:
        # `kschedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - a0: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        call kschedule
        li a1, 0
        beq a0, a1, raw_run_task
        li a1, 1
        beq a0, a1, raw_krun_utask
        # 不可达
        .word 0xdeadbeef

    raw_uschedule:
        # `uschedule`为`schedule_loop.rs`中的rust函数。
        # 仅在下一任务在本进程中时，会从该函数返回。
        # 参数：
        # - a0: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        mv a0, s2
        call uschedule
        j raw_run_task
        # 不可达
        .word 0xdeadbeef

    raw_utok_schedule:
        # `utok_schedule`为`schedule_loop.rs`中的rust函数。
        # 返回值：
        # - a0: 下一步的跳转目标
        #   - 0: run_task
        #   - 1: krun_utask
        call utok_schedule
        li a1, 0
        beq a0, a1, raw_run_task
        li a1, 1
        beq a0, a1, raw_krun_utask
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
        # - a0: 代表当前特权级，1为用户态，0为内核态。
        # - a1: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        # 返回值：
        # - a0: 特权级
        #     - 0: 内核态
        #     - 1: 用户态
        mv a0, s1
        mv a1, s2
        call run_task
        mv s1, a0 # 通过`run_task`（实际是`run_coroutine`）的返回值设置s1
        li s2, 0 # 从`run_task`（实际是`run_coroutine`）中返回则一定是协程，因此是空栈
        li a1, 0
        beq s1, a1, raw_kschedule
        li a1, 1
        beq s1, a1, raw_uschedule
        # 不可达
        .word 0xdeadbeef

    raw_krun_utask:
        # `krun_utask`为`schedule_loop.rs`中的rust函数。
        # 不会从该函数返回。
        # 参数：
        # - a0: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
        mv a0, s2
        call krun_utask
        # 不可达
        .word 0xdeadbeef
        
"#
);
