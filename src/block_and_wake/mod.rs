//! 阻塞唤醒相关模块
//!

#[cfg(feature = "vdso_only")]
pub(crate) mod waker;
