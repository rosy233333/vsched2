/// 封装跳转指令以适配不同架构。
///
/// 跳转指令用于实现调度循环中各函数的切换。
#[macro_export]
macro_rules! jump {
    ($f:ident) => {
        unsafe{core::arch::asm!("j {0}", sym $f);}
    };
}
