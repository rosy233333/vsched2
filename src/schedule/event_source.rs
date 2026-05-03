//! 事件源
//!
//! 为了实现泛型，采用“指针+vtable”表示事件源。

/// 事件源的接口，因为每个事件源的接口不同，因此使用Vtable而非trait_interface定义接口
///
/// 事件源需要实现内部可变性和与之适配的同步机制
#[repr(C)]
#[derive(Debug)]
pub struct EventSorceVtable {
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
    pub hightest_priority: fn(*const (), usize) -> isize,
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
    pub take_task: fn(*const (), usize) -> (*const (), isize),
}

/// 事件源接口的trait形式，实现这个trait可以自动生成vtable
pub trait EventSorce {
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

    /// 生成vtable
    fn vtable() -> EventSorceVtable
    where
        Self: Sized,
    {
        EventSorceVtable {
            hightest_priority: |ptr, cpu_id| {
                let es = unsafe { &*(ptr as *const Self) };
                es.hightest_priority(cpu_id)
            },
            take_task: |ptr, cpu_id| {
                let es = unsafe { &*(ptr as *const Self) };
                es.take_task(cpu_id)
            },
        }
    }
}
