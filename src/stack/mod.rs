#[cfg(feature = "vdso_only")]
mod trampoline;
#[cfg(feature = "vdso_only")]
pub(crate) use trampoline::*;
mod handler;
pub use handler::*;
