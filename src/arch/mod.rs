#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
mod riscv;

#[cfg(any(target_arch = "riscv32", target_arch = "riscv64"))]
pub use riscv::*;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod x86;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub use x86::*;
