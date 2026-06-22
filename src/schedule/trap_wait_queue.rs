//! trap等待队列，实现为一个事件源。
//!
//! 在该队列上存储当前核心收到的trap，以及阻塞于trap上的任务。
//!
//! 从该队列取出的任务为trap处理任务，由其负责处理trap，并在处理完成后唤醒阻塞的任务。调度器会在适当的时候运行。
//!
//! trap等待队列实现为per-cpu。

use core::{
    future::poll_fn,
    marker::PhantomPinned,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering},
    task::Poll,
};

use heapless::Deque;
use lazyinit::LazyInit;
use spin::mutex::Mutex;
use vdso_helper::get_vvar_data;

use crate::{
    current::get_current_task, schedule::event_source::EventSource, SMPVirtImpl, Task, TaskState,
    TaskVirtImpl, TrapInfo, TrapInfoVirtImpl, CPU_NUM, HIGHEST_PRIORITY, LOWEST_PRIORITY, SMP,
    TRAP_WAIT_QUEUE_SIZE,
};

const INACTIVE_PRIORITY: isize = LOWEST_PRIORITY + 1;
const ACTIVE_PRIORITY: isize = HIGHEST_PRIORITY;

pub(crate) struct TrapWaitQueue {
    // /// 当前核心收到的trap的数量
    // trap_count: AtomicUsize,
    /// per-cpu的队列
    queues: [Mutex<
        Deque<(&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>), TRAP_WAIT_QUEUE_SIZE>,
    >; CPU_NUM],
    /// 每个核心上的trap处理任务
    handlers: [LazyInit<&'static TaskVirtImpl>; CPU_NUM],
    /// 因为handlers中的trap处理任务的Future持有queues中队列的引用，因此需要固定该结构。
    _pin: PhantomPinned,
}

impl TrapWaitQueue {
    /// 注意：在`new()`之后还需调用`init()`，之后才能投入使用。
    pub(crate) const fn new() -> Self {
        Self {
            // trap_count: AtomicUsize::new(0),
            queues: [const { Mutex::new(Deque::new()) }; CPU_NUM],
            handlers: [const { LazyInit::new() }; CPU_NUM],
            _pin: PhantomPinned,
        }
    }

    /// 初始化trap处理任务
    pub(crate) fn init(self: Pin<&Self>) {
        for cpuid in 0..CPU_NUM {
            let handler = unsafe {
                TaskVirtImpl::from_ptr(TrapInfoVirtImpl::new_handler(
                    &self.as_ref().queues[cpuid] as *const _ as *const (),
                ))
            };
            self.handlers[cpuid].init_once(handler);
        }
    }

    /// 将一个trap信息和一个可选的被trap的任务放入队列
    pub(crate) fn push_trap(
        &self,
        trap_info: &'static TrapInfoVirtImpl,
        task: Option<&'static TaskVirtImpl>,
        cpuid: usize,
    ) -> Result<(), (&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>)> {
        self.queues[cpuid].lock().push_back((trap_info, task))?;
        // self.trap_count
        //     .fetch_add(1, core::sync::atomic::Ordering::AcqRel);
        Ok(())
    }
}

/// 在trap处理任务中运行的函数。
///
/// OS需在`TrapInfo::new_handler`的实现中，用这个函数创建trap处理任务。
/// 该函数的参数即为`new_handler`接口中传入的参数，即指向trap等待队列中某个核心的队列的指针。
///
/// 该函数只能通过api调用，不能直接调用。
#[inline]
pub(crate) fn trap_handler(queue: *const ()) {
    let queue = unsafe {
        &*(queue
            as *const Mutex<
                Deque<
                    (&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>),
                    TRAP_WAIT_QUEUE_SIZE,
                >,
            >)
    };
    // let cpuid = SMPVirtImpl::cpu_id();
    // let queue = &self.queues[cpuid];
    loop {
        let res = queue.lock().pop_front();
        if let Some((trap_info, task)) = res {
            // 处理trap
            // self.trap_count
            //     .fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
            // TODO: 根据task切换地址空间？还是把切换地址空间放在handle接口的逻辑里？
            trap_info.handle(task.map(|t| t.to_ptr()));
            if let Some(task) = &task {
                // 唤醒被trap的任务
                task.set_state(TaskState::Ready);
                let scheduler = if task.is_kernel() {
                    get_vvar_data!(KERNEL_SCHEDULER).load(Ordering::Acquire)
                } else {
                    // 用户态任务的调度器指针存储在全局进程表中
                    // TODO: 此处假定用户态任务一定位于当前地址空间。是否是这样？
                    let process_info_table = get_vvar_data!(PROCESS_INFO_TABLE);
                    let process_info = &process_info_table.table[task.pid()];
                    process_info.scheduler.load(Ordering::Acquire)
                };
                unsafe {
                    (*scheduler).push_task(task).unwrap();
                }
            }
        } else {
            // 没有trap，等待
            // 不需要存储Waker，因为总是可以从`TrapWaitQueue`中获取该任务。
            let task = get_current_task();
            task.set_state(TaskState::Blocked);
            task.resched();
        }
    }
}

impl EventSource for TrapWaitQueue {
    fn hightest_priority(&self, cpu_id: usize) -> isize {
        // 只要队列非空就返回ACTIVE_PRIORITY，否则返回INACTIVE_PRIORITY
        if self.queues[cpu_id].lock().is_empty() {
            INACTIVE_PRIORITY
        } else {
            ACTIVE_PRIORITY
        }
    }

    fn take_task(&self, cpu_id: usize) -> (*const (), isize) {
        if self.queues[cpu_id].lock().is_empty() {
            (core::ptr::null(), INACTIVE_PRIORITY)
        } else {
            let handler = self.handlers[cpu_id].get().unwrap();
            // 无论有多少个TrapInfo，任务都会将它们处理完之后再让出，
            // 因此唤醒任务后，就可以将优先级设置为INACTIVE_PRIORITY。
            (handler.to_ptr(), INACTIVE_PRIORITY)
        }
    }
}
