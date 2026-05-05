//! 可移植的统一任务调度模块

#![no_std]
#![warn(missing_docs)]
#![feature(naked_functions)]

mod api;
mod arch;
#[allow(non_snake_case)]
#[allow(missing_docs)]
pub mod current;
#[allow(non_camel_case_types)]
#[allow(missing_docs)]
pub mod interface;
mod main_loop;
mod schedule;
mod stack;

pub use current::VvarData;
pub use interface::*;
