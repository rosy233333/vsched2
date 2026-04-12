//! 可移植的统一任务调度模块

#![no_std]
#![deny(missing_docs)]
#![feature(naked_functions)]

mod arch;
mod current;
mod interface;
mod schedule_loop;
mod scheduler;
mod stack;

use current::VvarData;
