//! 协程的 waker 实现，参考了[AsyncOS的实现](https://github.com/AsyncModules/async-os/blob/main/modules/taskctx/src/waker.rs)
//!
//! 如果阻塞后任务只可能被一个Waker唤醒，则按照任务的生命周期，会先通过Waker唤醒任务，任务才会继续执行到结束，
//! 因此Waker指向的任务一定是有效的。
//!
//! 但如果阻塞后任务可能被多种方式唤醒，而其中包含一个或多个Waker（例如同时等待多个事件，并在第一个事件到来时唤醒），
//! 则可能出现Waker指向的任务已经结束甚至释放，导致唤醒无效的任务。
//!
//! 因此，该Waker实现暂不支持在一次阻塞中使用多个Waker，或同时使用Waker和其它唤醒方式的情况。

use core::task::{RawWaker, RawWakerVTable, Waker};

use crate::{push_task, SchedAction, Task, TaskState, TaskVirtImpl};

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop);

/// 直接根据 Task 的指针重新构造 Waker
unsafe fn clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &VTABLE)
}

/// 根据 Waker 内部的无类型指针，得到 Task 的指针，唤醒任务
unsafe fn wake(p: *const ()) {
    assert!(!p.is_null());
    wakeup_task(unsafe { &*(p as *const TaskVirtImpl) })
}

/// 创建 waker 时没有增加引用计数，因此不需要实现 Drop
unsafe fn drop(_p: *const ()) {}

/// 根据 Task 的引用创建 Waker
pub(crate) fn waker_from_task(task: &TaskVirtImpl) -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(task as *const _ as *const (), &VTABLE)) }
}

pub(crate) fn wakeup_task(task: &TaskVirtImpl) {
    let task = unsafe { &*task };
    let guard = task.state_lock_acquire();
    match task.state() {
        TaskState::Blocked => {
            task.set_state(TaskState::Ready);
            assert!(push_task(task as *const _ as *const ()));
        }
        TaskState::Running => {
            task.set_action(SchedAction::Yield);
        }
        s => {
            panic!(
                "wakeup_task: task state is not Blocked or Running, but {:?}",
                s
            );
        }
    }
    task.state_lock_release(guard);
}
