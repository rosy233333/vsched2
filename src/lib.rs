//! 可移植的统一任务调度模块

#![no_std]
#![deny(missing_docs)]

mod arch;
mod current;
mod interface;
mod schedule_loop;
mod scheduler;

use current::VvarData;
