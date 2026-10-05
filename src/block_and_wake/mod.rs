//! 阻塞唤醒相关模块
//!

#[cfg(feature = "vdso_only")]
pub(crate) mod block_queue;
#[cfg(feature = "vdso_only")]
pub(crate) mod waker;
