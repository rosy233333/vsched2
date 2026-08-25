//! 事件源
//!
//! 为了实现泛型，采用“指针+vtable”表示事件源。

use vdso_helper::log::warn;

use crate::{UserData, UserDataVirtImpl};

/// 事件源的接口，因为每个事件源的接口不同，因此使用Vtable而非trait_interface定义接口
///
/// 事件源需要实现内部可变性和与之适配的同步机制
#[repr(C)]
#[derive(Debug)]
pub struct EventSourceVtable {
    /// 获取当前事件源中就绪任务的最高优先级。
    ///
    /// 要求优先级数值越低，优先级越高。若实际调度算法与之相反，可以取相反数后传入接口。
    ///
    /// 参数：
    ///
    /// - 指针指向事件源结构体。
    /// - usize代表当前CPU id，用于兼容per-cpu的队列设计
    ///
    /// 返回值为最高优先级，若没有就绪任务则返回比最低优先级更低一级的优先级。
    kernel_hightest_priority: fn(*const (), usize) -> isize,
    /// 用户态调用的最高优先级查询函数。
    user_hightest_priority: fn(*const (), usize) -> isize,
    /// 取出当前事件源中最高优先级的就绪任务
    ///
    /// 参数：
    ///
    /// - 指针指向事件源结构体。
    /// - usize代表当前CPU id，用于兼容per-cpu的队列设计
    ///
    /// 返回值：
    ///
    /// - 就绪任务的指针，指向外部定义，实现`Task` trait的类型，若没有就绪任务则返回空指针；
    /// - 取出就绪任务后事件源中就绪任务的最高优先级，若没有就绪任务则返回比最低优先级更低一级的优先级
    kernel_take_task: fn(*const (), usize) -> (*const (), isize),
    /// 用户态调用的任务获取函数。
    user_take_task: fn(*const (), usize) -> (*const (), isize),
    /// 该类型的优先级是否是per-cpu的，即每个CPU有独立的优先级。
    ///
    /// 若为false，则所有CPU共享一个优先级。
    pub is_prio_per_cpu: bool,
}

fn hightest_priority_impl<T: EventSource>(ptr: *const (), cpu_id: usize) -> isize {
    let es = unsafe { &*(ptr as *const T) };
    es.hightest_priority(cpu_id)
}

fn take_task_impl<T: EventSource>(ptr: *const (), cpu_id: usize) -> (*const (), isize) {
    let es = unsafe { &*(ptr as *const T) };
    es.take_task(cpu_id)
}

impl EventSourceVtable {
    /// 使用显式的内核态和用户态函数创建vtable。
    ///
    /// 外部事件源不一定位于vsched2的vDSO中，因此不能统一使用相对vDSO代码基址的偏移。
    /// 
    /// 使用时需要保证能在对应特权级执行。
    pub const fn new(
        kernel_hightest_priority: fn(*const (), usize) -> isize,
        user_hightest_priority: fn(*const (), usize) -> isize,
        kernel_take_task: fn(*const (), usize) -> (*const (), isize),
        user_take_task: fn(*const (), usize) -> (*const (), isize),
        is_prio_per_cpu: bool,
    ) -> Self {
        Self {
            kernel_hightest_priority,
            user_hightest_priority,
            kernel_take_task,
            user_take_task,
            is_prio_per_cpu,
        }
    }

    pub(crate) fn with_user(mut self, vspace: *mut ()) -> Self {
        let hightest_priority = UserDataVirtImpl::get_user_addr(
            self.kernel_hightest_priority as usize,
            1,
            Some(vspace),
        );
        let take_task =
            UserDataVirtImpl::get_user_addr(self.kernel_take_task as usize, 1, Some(vspace));
        assert!(
            !hightest_priority.is_null() && !take_task.is_null(),
            "EventSource vtable user address translation failed"
        );
        self.user_hightest_priority = unsafe { core::mem::transmute(hightest_priority) };
        self.user_take_task = unsafe { core::mem::transmute(take_task) };
        self
    }

    /// 调用当前特权级对应的最高优先级查询函数。
    pub fn hightest_priority(&self, ptr: *const (), cpu_id: usize, in_kernel: bool) -> isize {
        let function = if in_kernel {
            self.kernel_hightest_priority
        } else {
            self.user_hightest_priority
        };
        function(ptr, cpu_id)
    }

    /// 调用当前特权级对应的任务获取函数。
    pub fn take_task(&self, ptr: *const (), cpu_id: usize, in_kernel: bool) -> (*const (), isize) {
        let function = if in_kernel {
            self.kernel_take_task
        } else {
            self.user_take_task
        };
        function(ptr, cpu_id)
    }
}

/// 事件源接口的trait形式，实现这个trait可以自动生成vtable
pub trait EventSource {
    /// 获取当前事件源中就绪任务的最高优先级。
    ///
    /// 要求优先级数值越低，优先级越高。若实际调度算法与之相反，可以取相反数后传入接口。
    ///
    /// 参数：
    ///
    /// - usize代表当前CPU id，用于兼容per-cpu的队列设计
    ///
    /// 返回值为最高优先级，若没有就绪任务则返回比最低优先级更低一级的优先级。
    fn hightest_priority(&self, cpu_id: usize) -> isize;
    /// 取出当前事件源中最高优先级的就绪任务
    ///
    /// 参数：
    ///
    /// - usize代表当前CPU id，用于兼容per-cpu的队列设计
    ///
    /// 返回值：
    ///
    /// - 就绪任务的指针，指向外部定义，实现`Task` trait的类型，若没有就绪任务则返回空指针；
    /// - 取出就绪任务后事件源中就绪任务的最高优先级，若没有就绪任务则返回比最低优先级更低一级的优先级
    fn take_task(&self, cpu_id: usize) -> (*const (), isize);
    /// 该类型的优先级是否是per-cpu的，即每个CPU有独立的优先级。
    ///
    /// 若为false，则所有CPU共享一个优先级。
    const IS_PRIO_PER_CPU: bool;

    /// 生成vtable
    fn vtable() -> EventSourceVtable
    where
        Self: Sized,
    {
        let vtable = EventSourceVtable::new(
            hightest_priority_impl::<Self>,
            hightest_priority_impl::<Self>,
            take_task_impl::<Self>,
            take_task_impl::<Self>,
            Self::IS_PRIO_PER_CPU,
        );
        // info!(
        //     "vtable: hightest_priority: {:#x}, take_task: {:#x}",
        //     vtable.hightest_priority as *const () as usize, vtable.take_task as *const () as usize
        // );
        vtable
    }
}
