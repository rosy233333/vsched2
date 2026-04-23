use core::{ptr, task::Poll};
use vdso_helper::{trait_interface, use_mut_cfg};

use_mut_cfg!();

trait_interface! {
    /// 任务的描述，以及执行流切换
    pub trait Task {
        /// 任务状态
        fn state(&self) -> TaskState;
        /// 设置任务状态
        fn set_state(&self, state: TaskState) -> TaskState;
        /// 判断任务为线程或协程，依据是保存的上下文类型
        ///
        /// 根据最新保存的上下文类型不同，线程和协程可以互相转化
        fn is_coroutine(&self) -> bool;
        /// 获取任务所处的进程id，也就是任务所处地址空间的所属进程的id，
        /// 因此某些内核态任务也可能属于某个进程。
        ///
        /// 如果之前未对该任务调用过`set_pid`，则返回0。否则，返回上一次`set_pid`传入的值。
        ///
        /// 目前，因为该字段仅用于获取内核任务所处的地址空间，
        /// 且因为任务的创建由os负责，无法在每个任务创建时均设置其pid，
        /// 所以仅对于由进程创建的内核态任务（如同步/异步trap处理任务），
        /// 我们会使用`set_pid`设置其pid，
        /// 也只有对这些任务调用`pid`才能获得有效的值。
        ///
        /// 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引
        fn pid(&self) -> usize;
        /// 设置任务的pid，也就是任务所处地址空间的所属进程的id，
        /// 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引。
        ///
        /// 目前仅对于由进程创建的内核态任务（如同步/异步trap处理任务）调用，
        /// 因此只有对这些任务调用`pid`才能获得有效的值。
        fn set_pid(&self, pid: usize);
        /// 保存线程上下文
        fn save_thread_context(&self);
        /// 保存trap上下文
        fn save_trap_context(&self);
        /// 恢复寄存器上下文（可能为线程上下文或trap上下文）
        fn restore_context(&self);
        /// 恢复协程上下文，函数返回时自动保存了协程上下文
        fn poll(&self) -> Poll<usize>;
        /// 获取线程上下文保存的栈底指针
        fn thread_stack_base(&self) -> usize;
    }
}

trait_interface! {
    /// 栈的分配和回收。
    /// 以栈底指针为标识，分配时返回栈底指针，回收时传入栈底指针
    ///
    /// 只会在栈所在的地址空间中调用。
    pub trait Stack {
        /// 分配栈
        fn alloc() -> *mut ();
        /// 回收栈
        fn dealloc(stack: *mut ());
    }
}

trait_interface! {
    /// 特权级切换和地址空间切换
    pub trait Context {
        /// 在调度中陷入内核态，在空栈中进入`ktrap_entry`函数并在后续进入`utok_schedule`函数
        fn into_kernel();
        /// 在调度中进入用户态，在空栈中进入`run_task`函数
        ///
        /// 在内核态调度到用户协程后使用
        fn into_user();
        /// 在调度中进入用户态寄存器上下文
        ///
        /// 参数中的指针指向外部定义的Task类型
        ///
        /// 在内核态调度到用户线程后使用
        fn into_user_context(task: *const ());
        /// 在内核态切换地址空间
        ///
        /// 目前还不清楚使用什么类型代表地址空间
        fn switch_vspace(vspace: *const ());
    }
}

trait_interface! {
    /// 同步trap处理
    pub trait TrapHandle {
        /// 通过一个阻塞队列获取阻塞于其上的trap处理任务，如果没有足够的任务则创建一个。
        ///
        /// 参数中的指针指向外部定义的Task类型，代表被trap的任务。获取的trap处理任务需要接收这个任务，解析trap原因并处理。
        ///
        /// trap处理任务的执行流程应如图所示：[trap处理任务执行流程.png](https://github.com/rosy233333/weekly-progress/blob/dev/26.3.23~26.3.29/trap%E5%A4%84%E7%90%86%E4%BB%BB%E5%8A%A1%E6%89%A7%E8%A1%8C%E6%B5%81%E7%A8%8B.png)
        fn get_handler(task: *const ()) -> *const ();
    }
}

trait_interface! {
    /// 多核接口
    ///
    /// 除了此处以外，还包括编译时通过环境变量修改的核心数`CPU_NUM`。
    pub trait SMP {
        /// 获取当前cpuid
        fn cpu_id() -> usize;
    }
}

trait_interface! {
    /// 地址空间相关接口
    pub trait VSpace {
        /// 切换地址空间
        ///
        /// 地址空间使用`*mut ()`表示，即为`ProcessInfo`中的`vspace`中的内容。
        fn into_vspace(vspace: *mut ());
    }
}

// 这里不该是trait_interface，而应该是模块提供给外界的接口
// trait_interface! {
//     /// 调度器
//     pub trait Scheduler {
//         /// 注册事件源
//         ///
//         /// index参数为事件源的插入位置，在获取到的最高优先级相同时，优先选择位置靠前的事件源。
//         ///
//         /// index为0或正数时在index位置插入事件源，index为负数时在倒数第index位置插入事件源。插入成功则返回true。
//         ///
//         /// 若index>len或index<-len-1（len为当前事件源数量），则插入失败，返回false。
//         fn register_event_source(&self, event_source: *const (), vtable: *const EventSorceVtable, index: isize) -> bool;
//         /// 取消注册事件源
//         fn unregister_event_source(&self, event_source: *const ());
//     }
// }

#[repr(u8)]
#[derive(PartialEq)]
pub enum TaskState {
    Ready = 0,
    Running = 1,
    Blocked = 2,
    Exited = 3,
}

/// 事件源的接口，因为每个事件源的接口不同，因此使用Vtable而非trait_interface定义接口
#[repr(C)]
pub struct EventSorceVtable {
    /// 获取当前事件源中就绪任务的最高优先级。
    ///
    /// 要求优先级数值越低，优先级越高。若实际调度算法与之相反，可以取相反数后传入接口。
    ///
    /// 参数的指针指向事件源结构体，返回值为最高优先级，若没有就绪任务则返回比最低优先级更低一级的优先级。
    ///
    /// 参数中没有CPU id，因为就算使用了per-cpu设计，最高优先级也是所有cpu上任务的最高优先级。
    /// 这意味着若使用per-cpu设计，就需要同时实现工作窃取。
    pub hightest_priority: fn(*const ()) -> isize,
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
