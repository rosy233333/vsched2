//! 可移植的统一任务调度模块

#![no_std]
#![warn(missing_docs)]
#![feature(naked_functions)]

#[cfg(feature = "vdso_only")]
mod api;
#[cfg(feature = "vdso_only")]
mod arch;
#[allow(non_snake_case)]
#[allow(missing_docs)]
pub mod current;
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
pub mod interface;
#[cfg(feature = "vdso_only")]
mod main_loop;
pub mod schedule;
mod stack;

pub use current::VvarData;
pub use interface::*;
