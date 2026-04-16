use core::arch::global_asm;

/// 封装跳转指令以适配不同架构。
///
/// 跳转指令用于实现调度循环中各函数的切换。
///
/// 在跳转前，先要重置sp寄存器到函数调用前的位置，相当于释放当前函数的栈帧。
///
/// 在函数a的最后`reset_sp_and_jump`到函数b，相当于先后调用了函数a和函数b。
///
/// risc-v架构：栈帧范围为[fp（高地址）, sp（低地址）)，
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

global_asm!(
    r#"
    .globl raw_trap_entry, raw_thread_entry, raw_run_task

    # 调度循环中使用的寄存器（均为callee-saved）及其含义：
    # - s1: 代表当前特权级，1为用户态，0为内核态。
    # - s2: 代表`schedule_loop`函数所在栈的状态，0为空栈，1为非空栈。
    schedule_loop:

    # `raw_trap_entry`为os发生trap、保存上下文并进行一定的解析后进入的入口。
    # os传递给调度器的参数：
    # - a0: trap类型
    #   - 0: 非外部中断
    #   - 1: 外部中断
    #   - 2: 特殊参数的系统调用，仅用于“从用户态调度器进入内核”的情况。
    # - a1: 代表当前特权级，1为用户态，0为内核态。
    raw_trap_entry:
        mv s1, a1
        li s2, 0
        # `trap_entry`为`schedule_loop.rs`中的rust函数。
        # 参数：
        # - a0: trap类型，与os传入的参数格式相同。
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
        beq a0, a1, raw_trap_handle
        li a1, 1
        beq a0, a1, raw_kschedule
        # 不可达
        .word 0xdeadbeef

    raw_trap_handle:
        # `trap_handle`为`schedule_loop.rs`中的rust函数。
        call trap_handle
        j raw_run_task
        # 不可达
        .word 0xdeadbeef

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
    # 返回时，需要设置正确的s1和s2。
    raw_run_task:
        # `run_task`为`schedule_loop.rs`中的rust函数。
        # 仅在运行协程时，会从该函数返回。
        # 返回值：
        # - a0: 下一步的跳转目标
        #   - 0: kschedule
        #   - 1: uschedule
        call run_task
        li a1, 0
        beq a0, a1, raw_kschedule
        li a1, 1
        beq a0, a1, raw_uschedule
        # 不可达
        .word 0xdeadbeef
        
"#
);
