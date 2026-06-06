//! 就绪队列，实现为一个事件源

use core::sync::atomic::{AtomicU64, Ordering};

use heapless::Deque;
use spin::mutex::Mutex;

use crate::{
    interface::{Task, TaskVirtImpl, HIGHEST_PRIORITY, LOWEST_PRIORITY, READY_QUEUE_SIZE},
    schedule::event_source::EventSource,
};

const PRIORITY_LEVELS: usize = (LOWEST_PRIORITY - HIGHEST_PRIORITY) as usize + 1;

fn highest_one(bitmap: u64) -> Option<usize> {
    let mut index = 63;
    loop {
        if bitmap & (1u64 << index) != 0 {
            return Some(index);
        }
        if index == 0 {
            return None;
        }
        index -= 1;
    }
}

pub(crate) struct ReadyQueue {
    /// index = priority - HIGHEST_PRIORITY
    queues: [Mutex<Deque<&'static TaskVirtImpl, READY_QUEUE_SIZE>>; PRIORITY_LEVELS],
    /// 最高优先级（HIGHEST_PRIORITY）安排在最高位（2^63），依次顺延
    ///
    /// 2^(63-PRIORITY_LEVELS)位（有效位之后的第一位）恒为1，之后的每位恒为0。
    ///
    /// 有效值：[2^(63-PRIORITY_LEVELS), 2^64-2^(63-PRIORITY_LEVELS)]
    prio_bitmap: AtomicU64,
}

impl ReadyQueue {
    pub(crate) const fn new() -> Self {
        assert!(PRIORITY_LEVELS <= 64);
        let bitmap_init: u64 = if PRIORITY_LEVELS <= 63 {
            1 << (63 - PRIORITY_LEVELS)
        } else {
            0
        };
        Self {
            queues: [const { Mutex::new(Deque::new()) }; PRIORITY_LEVELS],
            // hightest_prio: AtomicIsize::new(LOWEST_PRIORITY + 1),
            prio_bitmap: AtomicU64::new(bitmap_init),
        }
    }

    pub(crate) fn push_task(
        &self,
        task: &'static TaskVirtImpl,
    ) -> Result<(), &'static TaskVirtImpl> {
        // 放入队列 -> 更新优先级
        let prio = task.priority();
        self.queues[(prio - HIGHEST_PRIORITY) as usize]
            .lock()
            .push_back(task)?;
        self.prio_bitmap
            .fetch_or(1 << (63 - prio + HIGHEST_PRIORITY), Ordering::AcqRel);
        Ok(())
    }
}

impl EventSource for ReadyQueue {
    fn hightest_priority(&self, _cpu_id: usize) -> isize {
        let bitmap = self.prio_bitmap.load(Ordering::Acquire);
        let highest_one_index = highest_one(bitmap).map_or(-1, |i| i as isize);
        63 + HIGHEST_PRIORITY - highest_one_index
    }

    fn take_task(&self, _cpu_id: usize) -> (*const (), isize) {
        // let original_prio = self.hightest_prio.load(Ordering::Acquire);
        // let mut prio = original_prio;
        let mut prio = HIGHEST_PRIORITY;
        while prio <= LOWEST_PRIORITY {
            let queue = &self.queues[(prio - HIGHEST_PRIORITY) as usize];
            if let Some(task) = queue.lock().pop_front() {
                // 更新优先级
                let next_prio = if queue.lock().is_empty() {
                    let new_bitmap = self
                        .prio_bitmap
                        .fetch_and(!(1 << (63 - prio + HIGHEST_PRIORITY)), Ordering::AcqRel);
                    // 再次检查队列为空，避免期间队列中插入了任务，但在位图上的置位被上文的fetch_and覆盖
                    if queue.lock().is_empty() {
                        let highest_one_index = highest_one(new_bitmap).map_or(-1, |i| i as isize);
                        63 + HIGHEST_PRIORITY - highest_one_index
                    } else {
                        self.prio_bitmap
                            .fetch_or(1 << (63 - prio + HIGHEST_PRIORITY), Ordering::AcqRel);
                        prio
                    }
                } else {
                    prio
                };
                return (task.to_ptr(), next_prio);
            }
            prio += 1;
        }
        (core::ptr::null(), prio)
    }
}
