//! trap等待队列，实现为一个事件源。
//!
//! 在该队列上存储当前核心收到的trap，以及阻塞于trap上的任务。
//!
//! 从该队列取出的任务为trap处理任务，由其负责处理trap，并在处理完成后唤醒阻塞的任务。调度器会在适当的时候运行。
//!
//! trap等待队列实现为per-cpu。

use core::{marker::PhantomPinned, pin::Pin, sync::atomic::Ordering};

use heapless::Deque;
use kernel_guard::{BaseGuard, IrqSave};
use lazyinit::LazyInit;
use spin::mutex::Mutex;
use vdso_helper::{get_vvar_data, log::warn};

#[cfg(feature = "vdso_only")]
use crate::main_loop::switch_vspace;

#[cfg(not(feature = "vdso_only"))]
fn switch_vspace(vspace_pid: usize) { /*先空着吧*/
}

use crate::{
    current::{get_current_task, BLOCK_QUEUES, USER_SCHEDULER},
    schedule::{event_source::EventSource, scheduler::Scheduler},
    SMPVirtImpl, Task, TaskState, TaskVirtImpl, TrapInfo, TrapInfoVirtImpl,
    TrapInfo_FnIndex::handle,
    CPU_NUM, HIGHEST_PRIORITY, LOWEST_PRIORITY, SMP, TRAP_WAIT_QUEUE_SIZE,
};

const INACTIVE_PRIORITY: isize = LOWEST_PRIORITY + 1;
const ACTIVE_PRIORITY: isize = HIGHEST_PRIORITY;

/// 前两个只在 TrapWaitQueue 中使用
type TrapItem = (&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>);
type TrapQueue = Deque<TrapItem, TRAP_WAIT_QUEUE_SIZE>;

pub(crate) struct TrapWaitQueue {
    // /// 当前核心收到的trap的数量
    // trap_count: AtomicUsize,
    /// per-cpu的队列
    queues: [Mutex<TrapQueue>; CPU_NUM], // 这里和之前是一样的，我看着太长了，用 type 在上面重新定义了一下
    /// 所有CPU共享的空闲trap处理任务队列id。
    idle_handlers: LazyInit<u32>,
    // /// 每个核心上的trap处理任务
    // /// 只记录按CPU数量预创建的初始handler；handler本身不绑定CPU。
    // handlers: [LazyInit<&'static TaskVirtImpl>; CPU_NUM],
    /// 因为handlers中的trap处理任务的Future持有queues中队列的引用，因此需要固定该结构。
    /// 当前Future实际持有整个TrapWaitQueue的指针，以便在换了CPU后处理当前CPU的队列。
    _pin: PhantomPinned,
}

impl TrapWaitQueue {
    /// 注意：在`new()`之后还需调用`init()`，之后才能投入使用。
    pub(crate) const fn new() -> Self {
        Self {
            // trap_count: AtomicUsize::new(0),
            queues: [const { Mutex::new(Deque::new()) }; CPU_NUM],
            idle_handlers: LazyInit::new(),
            // handlers: [const { LazyInit::new() }; CPU_NUM],
            _pin: PhantomPinned,
        }
    }
}

#[cfg(feature = "vdso_only")]
impl TrapWaitQueue {
    /// 初始化trap处理任务和它的阻塞队列
    pub(crate) fn init(self: Pin<&Self>, scheduler: &Scheduler) {
        let queue_id = BLOCK_QUEUES.alloc().unwrap();
        self.idle_handlers.init_once(queue_id);

        let queue = self.as_ref().get_ref() as *const Self as *const ();
        for cpuid in 0..CPU_NUM {
            // 该函数不一定在初始化的调度器对应的地址空间中调用，因此需要传入调度器的指针，而不是直接使用USER_SCHEDULER获取调度器。
            let handler = unsafe {
                TaskVirtImpl::from_ptr(TrapInfoVirtImpl::new_handler(
                    scheduler as *const Scheduler as *const (),
                ))
            };
            // self.handlers[cpuid].init_once(handler);
            // let handler = *self.handlers[cpuid].get().unwrap();
            assert!(BLOCK_QUEUES.block_or_resume(*self.idle_handlers, handler));
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

#[cfg(feature = "vdso_only")]
fn get_idle_handler(queue: u32) -> Option<&'static TaskVirtImpl> {
    BLOCK_QUEUES.get(queue).unwrap().unpark_one()
}

/// 在trap处理任务中运行的函数。
///
/// OS需在`TrapInfo::new_handler`的实现中，用这个函数创建trap处理任务。
///
/// 该函数只能通过api调用，不能直接调用。
#[inline]
#[cfg(feature = "vdso_only")]
pub(crate) fn trap_handler(scheduler: &Scheduler) {
    use crate::block;

    let _state = IrqSave::acquire(); // 关中断
    let queue = &scheduler.trap_wait_queue;
    // let cpuid = SMPVirtImpl::cpu_id();
    // let queue = &self.queues[cpuid];
    let handler = get_current_task();
    loop {
        let cpuid = SMPVirtImpl::cpu_id();
        let mut queue_lock = queue.queues[cpuid].lock();
        let res = queue_lock.pop_front();
        let flag = res.is_some() && queue_lock.is_empty();
        drop(queue_lock);
        if flag {
            // trap_wait_queue的优先级需要从ACTIVE_PRIORITY降为INACTIVE_PRIORITY，以便调度器可以选择其他事件源的任务。
            scheduler.get_and_update_current_prio();
        }

        if let Some((trap_info, task)) = res {
            // 处理trap
            // self.trap_count
            //     .fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
            // 根据task切换地址空间？还是把切换地址空间放在handle接口的逻辑里？
            // 我暂时先放在了这里切换地址空间，如果后续验证不行，再改到handle接口里吧。
            let pid = task.map_or(0, Task::pid);
            handler.set_pid(pid);
            switch_vspace(pid);
            trap_info.handle(task.map(|t| t.to_ptr()));
            if let Some(task) = &task {
                let guard = task.state_lock_acquire();
                let old_state = task.state();
                if old_state == TaskState::Blocked {
                    task.set_state(TaskState::Ready);
                }
                // 系统调用exit等情况会在处理过程中将任务设置为Exited，不应再次入队。
                let exited = old_state == TaskState::Exited;
                task.state_lock_release(guard);

                // 这里多加的这层判断是为了避免在任务已经退出的情况下，仍然将其放入调度器的就绪队列中。
                // 没有验证过删掉是不是也可以，但是逻辑上看应该是需要的。因为有exit系统调用。
                if !exited {
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
                    // // TODO: 这里真的需要更新一下吗？
                    // if !task.is_kernel() {
                    //     let new_prio = unsafe { (*scheduler).hightest_priority() };
                    //     get_vvar_data!(PROCESS_INFO_TABLE).table[task.pid()]
                    //         .highest_prio
                    //         .store(new_prio, Ordering::Release);
                    // }
                } else {
                    // 这里应该加上释放任务的逻辑，否则无法处理exit系统调用的情况。
                    task.dealloc();
                }
            }
            trap_info.dealloc();
        } else {
            // 没有trap，等待
            // 不需要存储Waker，因为总是可以从`TrapWaitQueue`中获取该任务。
            block(*queue.idle_handlers);
        }
    }
}

#[cfg(feature = "vdso_only")]
#[cfg(feature = "vdso_only")]
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
        let pid = {
            let queue = self.queues[cpu_id].lock();
            let Some((_, task)) = queue.front() else {
                return (core::ptr::null(), INACTIVE_PRIORITY);
            };
            // 只要有TrapInfo，就可以取出trap_handler。
            // 因为trap_handler只会在当前核心上运行，所以取出trap_handler时，其一定不在运行，也就是保存好了上下文。
            // 共享handler队列由调用take_task的核心第一次运行取出的handler；
            task.map_or(0, Task::pid)
        };

        let handler = match get_idle_handler(*self.idle_handlers) {
            Some(handler) => handler,
            // 创建任务可能分配内存，不能持有TrapWaitQueue的自旋锁。
            None => {
                let scheduler = USER_SCHEDULER.get().unwrap();
                // 该函数一定在self所属的地址空间中调用，因此可以直接使用USER_SCHEDULER获取调度器。
                let handler = unsafe {
                    TaskVirtImpl::from_ptr(TrapInfoVirtImpl::new_handler(
                        scheduler as *const Scheduler as *const (),
                    ))
                };
                warn!(
                    "trap handler pool grow: handler={:#x}, cpu={cpu_id}",
                    handler.to_ptr() as usize
                );
                handler
            }
        };
        handler.set_pid(pid);

        // 原实现说明：
        // 无论有多少个TrapInfo，任务都会将它们处理完之后再让出，
        // 因此唤醒任务后，就可以将优先级设置为INACTIVE_PRIORITY。
        // 共享池中handler可能阻塞在内核资源上，因此保留ACTIVE_PRIORITY，让剩余TrapInfo可以继续取出其它handler处理。
        (handler.to_ptr(), ACTIVE_PRIORITY)
    }

    const IS_PRIO_PER_CPU: bool = true;
}
