//! 可移植的统一任务调度模块

#![no_std]
#![deny(missing_docs)]
#![feature(naked_functions)]

mod arch;
#[allow(non_snake_case)]
mod current;
#[allow(non_camel_case_types)]
mod interface;
mod schedule_loop;
mod scheduler;
mod stack;

use current::VvarData;
