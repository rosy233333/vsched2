//! trap等待队列，实现为一个事件源。
//!
//! 在该队列上存储当前核心收到的trap，以及阻塞于trap上的任务。
//!
//! 从该队列取出的任务为trap处理任务，由其负责处理trap，并在处理完成后唤醒阻塞的任务。调度器会在适当的时候运行。
//!
//! trap等待队列实现为per-cpu。

use core::{marker::PhantomPinned, pin::Pin, sync::atomic::Ordering};

use heapless::Deque;
use lazyinit::LazyInit;
use spin::mutex::Mutex;
use vdso_helper::{get_vvar_data, log::warn};

#[cfg(feature = "vdso_only")]
use crate::main_loop::switch_vspace;

#[cfg(not(feature = "vdso_only"))]
fn switch_vspace(vspace_pid: usize) {/*先空着吧*/}

use crate::{
    current::get_current_task, schedule::event_source::EventSource, SMPVirtImpl, Task, TaskState,
    TaskVirtImpl, TrapInfo, TrapInfoVirtImpl, CPU_NUM, HIGHEST_PRIORITY, LOWEST_PRIORITY, SMP,
    TRAP_WAIT_QUEUE_SIZE,
};

const INACTIVE_PRIORITY: isize = LOWEST_PRIORITY + 1;
const ACTIVE_PRIORITY: isize = HIGHEST_PRIORITY;

/// 前两个只在 TrapWaitQueue 中使用
type TrapItem = (&'static TrapInfoVirtImpl, Option<&'static TaskVirtImpl>);
type TrapQueue = Deque<TrapItem, TRAP_WAIT_QUEUE_SIZE>;
/// 储存空闲的 trap 处理任务的队列。所有 CPU 共享
type IdleHandlerQueue = Deque<&'static TaskVirtImpl, TRAP_WAIT_QUEUE_SIZE>;

enum IdleHandler {
    Runnable(&'static TaskVirtImpl),
    WakeInProgress,
    Empty,
}

pub(crate) struct TrapWaitQueue {
    // /// 当前核心收到的trap的数量
    // trap_count: AtomicUsize,
    /// per-cpu的队列
    queues: [Mutex<TrapQueue>; CPU_NUM], // 这里和之前是一样的，我看着太长了，用 type 在上面重新定义了一下
    /// 所有CPU共享的空闲trap处理任务队列。
    idle_handlers: Mutex<IdleHandlerQueue>,
    /// 每个核心上的trap处理任务
    /// 只记录按CPU数量预创建的初始handler；handler本身不绑定CPU。
    handlers: [LazyInit<&'static TaskVirtImpl>; CPU_NUM],
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
            idle_handlers: Mutex::new(Deque::new()),
            handlers: [const { LazyInit::new() }; CPU_NUM],
            _pin: PhantomPinned,
        }
    }

    /// 初始化trap处理任务
    pub(crate) fn init(self: Pin<&Self>) {
        let queue = self.as_ref().get_ref() as *const Self as *const ();
        for cpuid in 0..CPU_NUM {
            let handler = unsafe { TaskVirtImpl::from_ptr(TrapInfoVirtImpl::new_handler(queue)) };
            self.handlers[cpuid].init_once(handler);
            let handler = *self.handlers[cpuid].get().unwrap();
            self.idle_handlers
                .lock()
                .push_back(handler)
                .expect("initial trap handler queue is full");
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

fn reschedule_handler(queue: &TrapWaitQueue, handler: &'static TaskVirtImpl) {
    handler.set_pid(0);
    handler.set_state(TaskState::Blocking);
    queue
        .idle_handlers
        .lock()
        .push_back(handler)
        // TODO：满了怎么处理
        .expect("trap idle handler queue is full");
    handler.resched();
}

fn get_idle_handler(queue: &Mutex<IdleHandlerQueue>) -> IdleHandler {
    let mut queue = queue.lock();
    let len = queue.len();
    for _ in 0..len {
        let handler = queue.pop_front().unwrap();
        // handler会在resched保存上下文前进入共享idle_handlers。Blocked可以直接转为Ready；
        // Blocking转为Ready后由正在保存上下文的CPU负责将它放入就绪队列，本次不直接运行。
        match handler.match_set_state(
            TaskState::Ready,
            TaskState::Running,
            TaskState::Ready,
            TaskState::Exited,
            TaskState::Ready,
        ) {
            TaskState::Blocked => return IdleHandler::Runnable(handler),
            TaskState::Blocking => return IdleHandler::WakeInProgress,
            TaskState::Exited => {}
            state => panic!("idle trap handler has invalid state {state:?}"),
        }
    }
    IdleHandler::Empty
}

fn create_new_handler(handler: &'static TaskVirtImpl) -> &'static TaskVirtImpl {
    match handler.match_set_state(
        TaskState::Ready,
        TaskState::Running,
        TaskState::Ready,
        TaskState::Exited,
        TaskState::Ready,
    ) {
        TaskState::Ready | TaskState::Blocked => handler,
        state => panic!("new trap handler has invalid state {state:?}"),
    }
}

/// 在trap处理任务中运行的函数。
///
/// OS需在`TrapInfo::new_handler`的实现中，用这个函数创建trap处理任务。
/// 该函数的参数即为`new_handler`接口中传入的参数，即指向trap等待队列中某个核心的队列的指针。
/// 参数改为指向完整TrapWaitQueue；handler每次按当前CPU选择TrapInfo队列。
///
/// 该函数只能通过api调用，不能直接调用。
#[inline]
pub(crate) fn trap_handler(queue: *const ()) {
    let queue = unsafe { &*(queue as *const TrapWaitQueue) };
    // let cpuid = SMPVirtImpl::cpu_id();
    // let queue = &self.queues[cpuid];
    let handler = get_current_task();
    loop {
        let cpuid = SMPVirtImpl::cpu_id();
        let res = queue.queues[cpuid].lock().pop_front();
        if let Some((trap_info, task)) = res {
            // 处理trap
            // self.trap_count
            //     .fetch_sub(1, core::sync::atomic::Ordering::AcqRel);
            // TODO: 根据task切换地址空间？还是把切换地址空间放在handle接口的逻辑里？
            // 我暂时先放在了这里切换地址空间，如果后续验证不行，再改到handle接口里吧。
            let pid = task.map_or(0, Task::pid);
            handler.set_pid(pid);
            switch_vspace(pid);
            trap_info.handle(task.map(|t| t.to_ptr()));
            if let Some(task) = &task {
                let exited = match task.match_set_state(
                    TaskState::Ready,
                    TaskState::Running,
                    TaskState::Ready,
                    TaskState::Exited,
                    TaskState::Blocking,
                ) {
                    TaskState::Blocked => false,
                    // 系统调用exit等情况会在处理过程中将任务设置为Exited，不应再次入队。
                    TaskState::Exited => true,
                    _ => panic!("trap_handler: task state is not Blocked!")
                };
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
                }
            }
            trap_info.dealloc();
        } else {
            // 没有trap，等待
            // 不需要存储Waker，因为总是可以从`TrapWaitQueue`中获取该任务。
            reschedule_handler(queue, handler);
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
        let pid = {
            let queue = self.queues[cpu_id].lock();
            let Some((_, task)) = queue.front() else {
                return (core::ptr::null(), INACTIVE_PRIORITY);
            };
            // 只要有TrapInfo，就可以取出trap_handler。
            // 因为trap_handler只会在当前核心上运行，所以取出trap_handler时，其一定不在运行，也就是保存好了上下文。
            // 共享handler队列由调用take_task的核心第一次运行取出的handler；
            //
            // TODO: 这种情况还未处理：
            // trap_handler阻塞在某个内核资源上，但任务重调度时，trap_wait_queue仍会取出以最高优先级取出该任务。
            // 导致trap_handler被一直执行，出现近似忙等待的状况。
            // 我觉得这个TODO现在解决了，因为有了共享idle_handlers，阻塞在内核资源上的handler不在这个队列中了。
            task.map_or(0, Task::pid)
        };

        let handler = match get_idle_handler(&self.idle_handlers) {
            IdleHandler::Runnable(handler) => handler,
            IdleHandler::WakeInProgress => {
                return (core::ptr::null(), ACTIVE_PRIORITY);
            }
            // 创建任务可能分配内存，不能持有TrapWaitQueue的自旋锁。
            IdleHandler::Empty => {
                let handler = create_new_handler(unsafe {
                    TaskVirtImpl::from_ptr(TrapInfoVirtImpl::new_handler(
                        self as *const Self as *const (),
                    ))
                });
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
}
