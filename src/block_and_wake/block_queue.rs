//! 阻塞队列，可用于阻塞线程或协程。
//!
//! 阻塞队列是调度器内部的等待/唤醒原语：
//!
//! - 调度器负责任务的挂起（入队）、唤醒（出队后放回就绪队列）以及“检测通知—入队”的同步；
//! - OS负责阻塞条件的判断与维护，即何时调用阻塞接口、何时修改通知数。
//!
//! ## 通知计数
//!
//! 每条阻塞队列持有一个`notifications`计数（`isize`）：
//!
//! - 负值：阻塞中的任务数量的相反数（例如-3表示有3个任务阻塞）；
//! - 正值：已缓存的通知数，即可以让接下来到来的任务不阻塞的次数；
//! - `isize::MAX`：永久放行的特殊值，表示当前及之后到来的任务都不阻塞，此值不再增减。
//!
//! 通知计数的修改与“加入阻塞队列”在同一个锁临界区内完成，因此不会丢失唤醒。
//!
//! ## 所在位置与生命周期
//!
//! 阻塞队列组以静态变量（[`crate::current::BLOCK_QUEUES`]）的形式存在于vDSO的私有
//! 数据区中，因此它是进程（地址空间）、特权级私有的：不同用户进程、以及同一进程的
//! 用户态和内核态各有一组相互独立的阻塞队列。
//!
//! ## 与Waker的关系
//!
//! 本模块**不使用Waker**。Waker（见[`super::waker`]）用于外部异步函数（例如异步IO）
//! 自行实现的阻塞；阻塞在本模块的队列上的任务必须通过[`block_wake`]、[`block_wake_all`]、
//! [`block_wake_task`]等接口唤醒。若用Waker唤醒一个已挂起在阻塞队列上的任务，会导致该任务
//! 同时出现在阻塞队列与就绪队列中，造成重复唤醒。

use core::sync::atomic::{AtomicU64, Ordering};

use heapless::Deque;
use spin::mutex::Mutex;

use crate::current::{get_current_task, BLOCK_QUEUES};
use crate::interface::{Task, TaskState, TaskVirtImpl, BLOCK_QUEUE_NUM, BLOCK_QUEUE_SIZE};
use crate::SchedAction;

/// 永久放行的通知计数值
const PERMANENT_RELEASE: isize = isize::MAX;

/// 一组阻塞队列，及队列号的分配器。
///
/// 以静态变量形式存在于vDSO私有数据区（[`crate::current::BLOCK_QUEUES`]）。
pub(crate) struct BlockQueues {
    /// 队列数组，下标即为队列号。
    queues: [BlockQueue; BLOCK_QUEUE_NUM],
    /// 分配位图，第i位为1表示队列号为i的队列已分配。
    allocated: AtomicU64,
}

impl BlockQueues {
    /// 队列号的上限为64，因为分配位图使用一个`AtomicU64`。
    const _CHECK_QUEUE_NUM: () = assert!(BLOCK_QUEUE_NUM <= 64);

    /// 创建一组空的阻塞队列。可用作静态变量的初始值。
    pub(crate) const fn new() -> Self {
        Self {
            queues: [const { BlockQueue::new() }; BLOCK_QUEUE_NUM],
            allocated: AtomicU64::new(0),
        }
    }
}

#[cfg(feature = "vdso_only")]
impl BlockQueues {
    /// 分配一条空闲队列，返回其队列号；若无空闲队列则返回`None`。
    pub(crate) fn alloc(&self) -> Option<u32> {
        let result =
            self.allocated
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |allocated| {
                    for index in 0..BLOCK_QUEUE_NUM {
                        if allocated & (1u64 << index) == 0 {
                            return Some(allocated | (1u64 << index));
                        }
                    }
                    None
                });
        result.ok().and_then(|old| {
            (0..BLOCK_QUEUE_NUM)
                .find(|index| old & (1u64 << index) == 0)
                .map(|index| index as u32)
        })
    }

    /// 归还队列号。
    ///
    /// 要求队列为空（即没有任务阻塞在该队列上），否则返回`false`且不归还。
    pub(crate) fn free(&self, id: u32) -> bool {
        let Some(queue) = self.get(id) else {
            return false;
        };
        if !queue.is_empty() {
            return false;
        }
        self.allocated.fetch_and(!(1u64 << id), Ordering::AcqRel);
        true
    }

    /// 取得队列号对应的阻塞队列；越界或未分配时返回`None`。
    pub(crate) fn get(&self, id: u32) -> Option<&BlockQueue> {
        if id as usize >= BLOCK_QUEUE_NUM {
            return None;
        }
        if self.allocated.load(Ordering::Acquire) & (1u64 << id) == 0 {
            return None;
        }
        Some(&self.queues[id as usize])
    }

    /// 尝试阻塞`task`到队列号为`id`的阻塞队列上。
    ///
    /// 返回`true`表示任务需要阻塞（已被加入阻塞队列，状态已被置为`Blocked`）；
    /// 返回`false`表示任务取得了通知、无需阻塞（状态已被置为`Ready`，调用方应把
    /// 任务放回就绪队列）。
    ///
    /// 阻塞队列号不对应已分配的队列时，同样返回`false`并置为`Ready`，
    /// 以避免任务永久阻塞在无效队列上。
    pub(crate) fn block_or_resume(&self, id: u32, task: &'static TaskVirtImpl) -> bool {
        let blocked = match self.get(id) {
            Some(queue) => queue.block_task(task),
            None => false,
        };
        if blocked {
            task.set_state(TaskState::Blocked);
            true
        } else {
            task.set_state(TaskState::Ready);
            false
        }
    }
}

/// 一条阻塞队列。
pub(crate) struct BlockQueue {
    /// 同时保护阻塞任务队列与通知计数。
    ///
    /// 用锁而不是原子变量，是为了把“检测通知”与“加入阻塞队列”放入同一个临界区。
    state: Mutex<BlockQueueInner>,
}

struct BlockQueueInner {
    /// 通知计数，语义见模块文档。
    notifications: isize,
    /// 阻塞中的任务，先阻塞的先被唤醒。
    tasks: Deque<&'static TaskVirtImpl, BLOCK_QUEUE_SIZE>,
}

impl BlockQueue {
    pub(crate) const fn new() -> Self {
        Self {
            state: Mutex::new(BlockQueueInner {
                notifications: 0,
                tasks: Deque::new(),
            }),
        }
    }
}

#[cfg(feature = "vdso_only")]
impl BlockQueue {
    /// 尝试阻塞`task`。
    ///
    /// 返回`true`表示任务已经被加入本队列（调用方应把任务置为`Blocked`）；
    /// 返回`false`表示取得了一个通知、无需阻塞（调用方应把任务置为`Ready`）。
    ///
    /// 通知计数的判断与入队在同一个临界区内完成，期间不会释放队列锁。
    pub(crate) fn block_task(&self, task: &'static TaskVirtImpl) -> bool {
        let mut inner = self.state.lock();
        if inner.notifications == PERMANENT_RELEASE {
            // 永久放行：不递减计数，直接让任务不阻塞。
            return false;
        }
        let old = inner.notifications;
        inner.notifications = old - 1;
        if old > 0 {
            // 取得通知，不阻塞。
            return false;
        }
        if inner.tasks.push_back(task).is_err() {
            // 队列已满：恢复计数并让任务不阻塞，避免计数与实际队列不一致。
            inner.notifications += 1;
            return false;
        }
        true
    }

    /// 唤醒一个任务。
    ///
    /// 返回被唤醒（已从本队列取出）的任务；
    /// `None`表示没有阻塞中的任务，本次调用只是缓存了一个通知或队列处于永久放行状态。
    pub(crate) fn unpark_one(&self) -> Option<&'static TaskVirtImpl> {
        let mut inner = self.state.lock();
        if inner.notifications != PERMANENT_RELEASE {
            inner.notifications += 1;
        }
        if inner.notifications > 0 {
            // 没有阻塞中的任务：本次调用只缓存了通知。
            return None;
        }
        inner
            .tasks
            .pop_front()
            .or_else(|| panic!("unpark_one: queue empty when it shouldn't!"))
    }

    /// 唤醒一个任务，但若未能唤醒任务，则不缓存通知。
    pub(crate) fn unpark_one_without_notification(&self) -> Option<&'static TaskVirtImpl> {
        let mut inner = self.state.lock();
        if inner.notifications < 0 {
            inner.notifications += 1;
        } else {
            // 没有阻塞中的任务：本次调用不缓存通知。
            return None;
        }
        inner
            .tasks
            .pop_front()
            .or_else(|| panic!("unpark_one_without_notification: queue empty when it shouldn't!"))
    }

    /// 取出全部当前阻塞的任务，并按情况调整通知计数。
    ///
    /// `including_future`为`true`时，把通知计数置为永久放行，之后到来的任务也不会阻塞；
    /// 为`false`时，只在通知计数为负（即确实有阻塞任务）时置为0。
    ///
    /// 每次取出一个任务后都会释放队列锁，因此可以在取出任务后安全地调用`wake`
    /// （其内部会获取就绪队列锁）。
    pub(crate) fn notify_all<F>(&self, including_future: bool, mut wake: F) -> usize
    where
        F: FnMut(&'static TaskVirtImpl),
    {
        {
            let mut inner = self.state.lock();
            if including_future {
                inner.notifications = PERMANENT_RELEASE;
            } else if inner.notifications < 0 {
                inner.notifications = 0;
            }
        }
        let mut count = 0;
        loop {
            // 在锁内只取出任务，锁外再唤醒，避免在持有队列锁时获取就绪队列锁。
            let task = {
                let mut inner = self.state.lock();
                inner.tasks.pop_front()
            };
            match task {
                Some(task) => {
                    wake(task);
                    count += 1;
                }
                None => break,
            }
        }
        count
    }

    /// 定向唤醒`task`：把`task`从本队列中取出。
    ///
    /// 成功时通知计数加1（该任务不再占用阻塞计数），返回`true`；若`task`不在本队列中，
    /// 返回`false`且不修改通知计数。
    pub(crate) fn unpark_task(&self, task: &'static TaskVirtImpl) -> bool {
        let mut inner = self.state.lock();
        let Some(index) = inner.tasks.iter().position(|t| core::ptr::eq(*t, task)) else {
            return false;
        };
        // position已保证index有效；swap_remove_front保持O(1)，但会把原队尾的任务换到index处。
        inner.tasks.swap_remove_front(index);
        if inner.notifications != PERMANENT_RELEASE {
            inner.notifications += 1;
        }
        true
    }

    /// 阻塞在该队列上的任务数量。该值只用于调试与OS判断，可能随时间变化。
    pub(crate) fn len(&self) -> usize {
        self.state.lock().tasks.len()
    }

    /// 该队列上是否没有阻塞的任务。
    pub(crate) fn is_empty(&self) -> bool {
        self.state.lock().tasks.is_empty()
    }
}

/// 把任务放回它所属的就绪队列，返回是否成功。
///
/// 通过调用vDSO的`push_task`接口完成，因此不依赖调度器实例。
/// 调用时不应持有任务状态锁或阻塞队列锁。
#[cfg(feature = "vdso_only")]
fn push_ready(task: &'static TaskVirtImpl) -> bool {
    use crate::push_task;
    push_task(task.to_ptr())
}

/// 把任务置为`Ready`并放回就绪队列。
///
/// 该任务已经不在阻塞队列中（已被取出），因此只需处理状态与就绪队列。
/// 与[`prepare_ready`]一样，不会清除当前任务自己的action。
#[cfg(feature = "vdso_only")]
fn wake_ready(task: &'static TaskVirtImpl) {
    prepare_ready(task);
    push_ready_or_panic(task);
}

/// 把任务置为`Ready`。
///
/// 若任务是当前任务（自我唤醒），则只检查状态：此时任务处于运行态，既不能再次获取
/// 自己的状态锁，也不能清除自己的action（调度器入口还要读取它）。
#[cfg(feature = "vdso_only")]
fn prepare_ready(task: &'static TaskVirtImpl) {
    if core::ptr::eq(task, get_current_task()) {
        debug_assert!(
            matches!(task.state(), TaskState::Running | TaskState::Blocking),
            "block queue: self wake with unexpected state"
        );
        return;
    }
    let guard = task.state_lock_acquire();
    task.set_action(SchedAction::JustBlock);
    task.set_state(TaskState::Ready);
    task.state_lock_release(guard);
}

/// 把任务放回就绪队列，失败时panic。
#[cfg(feature = "vdso_only")]
fn push_ready_or_panic(task: &'static TaskVirtImpl) {
    assert!(
        push_ready(task),
        "block queue: failed to push task back to scheduler: {:#x}",
        task.to_ptr() as usize
    );
}

/// 唤醒一个阻塞在指定队列上的任务，或缓存一个通知。
///
/// 返回值：是否取出了任务。返回`false`表示没有阻塞中的任务，本次调用只缓存了一个通知。
#[cfg(feature = "vdso_only")]
pub(crate) fn wake(id: u32) -> bool {
    let Some(queue) = BLOCK_QUEUES.get(id) else {
        return false;
    };
    match queue.unpark_one() {
        Some(task) => {
            wake_ready(task);
            true
        }
        None => false,
    }
}

/// 唤醒当前阻塞在指定队列上的全部任务，返回被唤醒的任务数。
///
/// `including_future`为`true`时，之后到来的任务也不会阻塞，直到队列被重置
/// （见`block_wake_all`的说明）。
#[cfg(feature = "vdso_only")]
pub(crate) fn wake_all(id: u32, including_future: bool) -> usize {
    let Some(queue) = BLOCK_QUEUES.get(id) else {
        return 0;
    };
    queue.notify_all(including_future, wake_ready)
}

/// 定向唤醒`task`。
///
/// 返回`false`表示该任务没有阻塞在指定队列上，此时不修改任何状态。
/// 本函数不做降级处理（不会改为投递一个通知），是否需要降级由OS决定。
#[cfg(feature = "vdso_only")]
pub(crate) fn wake_task(id: u32, task: &'static TaskVirtImpl) -> bool {
    let Some(queue) = BLOCK_QUEUES.get(id) else {
        return false;
    };
    if queue.unpark_task(task) {
        // 任务已被取出且不再存在于任何阻塞队列中，可以安全地置为Ready。
        wake_ready(task);
        return true;
    } else {
        return false;
    }
}
