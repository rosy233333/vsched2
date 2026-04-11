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
