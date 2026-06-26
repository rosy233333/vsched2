// 启用了 #![deny(missing_docs)]，但这里缺少注释，先允许missing_docs，后续再补充注释。
#![allow(missing_docs)]

use core::task::Poll;
use vdso_helper::{trait_interface, use_mut_cfg};

use_mut_cfg!();

trait_interface! {
    /// 任务的描述，以及执行流切换
    pub trait Task {
        /// 任务状态
        fn state(&self) -> TaskState;
        /// 设置任务状态
        fn set_state(&self, state: TaskState) -> TaskState;
        /// 任务优先级
        fn priority(&self) -> isize;
        /// 判断任务为线程或协程，依据是保存的上下文类型
        ///
        /// 根据最新保存的上下文类型不同，线程和协程可以互相转化
        fn is_coroutine(&self) -> bool;
        /// 判断任务是否为内核态任务
        ///
        /// ## 两种特权级
        ///
        /// - 若需获取当前所处的特权级，使用`IN_KERNEL`，也可以使用`schedule_loop`中设置的特定寄存器。
        /// - 若需获取`current_task`的特权级，使用`get_current_task().is_kernel()`。
        ///
        /// 两者不一定相同，例如从用户态任务trap到内核的情况，`IN_KERNEL = true`，`get_current_task().is_kernel() = false`。
        fn is_kernel(&self) -> bool;
        /// 获取任务所处的进程id，也就是任务所处地址空间的所属进程的id，
        /// 因此某些内核态任务也可能属于某个进程。
        ///
        /// 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引
        fn pid(&self) -> usize;
        /// 设置任务的pid，也就是任务所处地址空间的所属进程的id，
        /// 此处的进程id即为全局进程表`PROCESS_INFO_TABLE`的索引。
        fn set_pid(&self, pid: usize);
        /// 保存线程上下文，然后进入`api::raw_thread_entry`。
        /// 下次从上下文中恢复后，从该函数中返回。
        ///
        /// 不需修改任务状态。
        ///
        /// 调用此函数时，`self`一定是当前任务。
        fn resched(&self);
        /// 恢复寄存器上下文（可能为线程上下文或trap上下文）
        ///
        /// 不需切换地址空间，不需设置当前栈。
        fn restore_context(&self);
        /// 恢复协程上下文，函数返回时自动保存了协程上下文
        fn poll(&self) -> Poll<isize>;
        /// 获取线程上下文保存的`Stack`指针
        fn thread_stack(&self) -> *mut ();
        /// 设置协程运行返回值
        fn set_return_value(&self, value: isize);
        /// 释放一个已经退出的任务
        ///
        /// 如果要提供对任务join的支持，则该函数中还需实现通知等待该任务退出的任务的机制。
        fn dealloc(&self);
    }
}

trait_interface! {
    /// 栈的描述
    ///
    /// 只会在栈所在的地址空间中调用。
    pub trait Stack {
        /// 分配栈
        fn alloc() -> *mut ();
        /// 回收栈
        ///
        /// 调用该函数后，不应再使用该栈数据结构
        fn dealloc(&mut self);
        /// 栈底指针
        fn base(&self) -> *mut ();
    }
}

trait_interface! {
    /// 特权级切换和地址空间切换
    pub trait Context {
        /// 在调度中陷入内核态，在空栈中进入`ktrap_entry`函数并在后续进入`utok_schedule`函数
        fn into_kernel() -> !;
        /// 在调度中进入用户态，在空栈中进入`run_task`函数
        ///
        /// 参数为进入用户态时应该使用的用户栈的栈顶地址，即sp寄存器的值
        ///
        /// 在内核态调度到用户协程后使用
        fn into_user(ustack: usize);
        /// 在调度中进入用户态寄存器上下文
        ///
        /// 参数中的指针指向外部定义的Task类型
        ///
        /// 在内核态调度到用户线程后使用
        fn into_user_context(task: *const ());
    }
}

trait_interface! {
    /// 同步trap处理接口，同时规定了trap信息数据结构的接口。
    ///
    /// TrapInfo的生命周期在模块内部管理，为了避免直接使用时大小不确定而使用引用形式。
    ///
    /// 外部需在`from_task`中创建`TrapInfo`的`Box`等堆分配实例，并将其转换为引用并返回；
    /// 在`dealloc`中将引用转换回`Box`等堆分配实例并进行释放。
    pub trait TrapInfo {
        /// 从被trap的任务中获取trap信息
        ///
        /// 传入的任务一定是被trap的任务，因此具有trap上下文类型的寄存器上下文。
        fn from_task(task: *const ()) -> *const Self;
        /// 处理trap。参数为被trap的任务。
        /// 当被trap的任务与trap处理无关时（例如外部中断），参数为None。
        fn handle(&self, task: Option<*const ()>);
        /// 释放trap信息。
        ///
        /// 在调度模块中，每个由`from_task`创建的`TrapInfo`实例都必须调用一次`dealloc`进行释放。
        fn dealloc(&self);
        /// 创建一个新的trap处理任务，并返回TCB地址
        /// （指向impl Task的指针）
        ///
        /// trap处理任务使用`trap_handler`作为执行的函数，且将该函数的参数传入`trap_handler`中。
        fn new_handler(queue: *const ()) -> *const ();
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

trait_interface! {
    pub trait UserData {
        /// 从内核中访问用户态vDSO私有数据
        ///
        /// 因为即使在一个地址空间中，用户态和内核态的vDSO私有数据也是分开的，因此需要借助这个函数进行地址运算，获得用户态对应数据的引用。
        ///
        /// - `pos` ：为私有数据对象的地址。
        /// - `len` ：为对象的字节长度。
        /// - `vspace` ：为要访问的地址空间，如果为None则访问当前地址空间。
        /// - 返回值：为用户态vDSO私有数据区中对应对象的地址，调用方可以将其转换为对应类型的引用。
        ///
        /// 地址运算的方式如下：
        ///
        /// 以下描述可以参考![此流程图](../doc/assets/get_user_data流程图.png)。
        ///
        /// 1. 获取内核态中对应变量（`A`）相对于vdso基址（`KBASE`）的虚拟地址偏移量（`offset`）
        /// 2. 获取用户空间中的vdso基址（虚拟地址）（`UBASE`）
        /// 3. 计算得到对应变量在用户空间中的虚拟地址（`B`）
        /// 4. 查询页表将3中的虚拟地址转化为物理地址（`b`）
        /// 5. 通过平移变换将物理地址转化为内核空间中的虚拟地址（`C`）
        ///
        /// 也就是说，`get_user_data(A, vspace) = C`
        ///
        /// # Safety
        ///
        /// - 外界实现的地址翻译必须保证返回的地址在用户态vDSO私有数据区内，且`[addr, addr + size_of::<T>())`完整可访问。
        /// - 因为访问的是用户态子空间的数据，因此不能在切换地址空间前后访问该函数返回的同一份引用。
        fn get_user_data(pos: usize, len: usize, vspace: Option<*mut ()>) -> *mut ();
    }
}

#[repr(u8)]
#[derive(PartialEq)]
pub enum TaskState {
    /// 就绪状态，可以从就绪队列中取出运行。
    ///
    /// ### 时序
    ///
    /// （被抢占）保存上下文 -> 设置状态为`Ready` -> 放回就绪队列 -> 从`CURRENT_TASK`上清除
    ///
    /// （主动让出）设置状态为`Ready` -> 保存上下文 -> 放回就绪队列 -> 从`CURRENT_TASK`上清除
    Ready = 0,
    /// 运行状态。
    ///
    /// ### 时序
    ///
    /// 设置为`CURRENT_TASK` -> 设置状态为`Running` -> 恢复上下文
    Running = 1,
    /// 阻塞状态。
    ///
    /// ### 时序
    ///
    /// （在任务内设置Blocking状态）设置状态为`Blocking` -> 保存上下文 -> 设置状态为`Blocked` -> 从`CURRENT_TASK`上清除
    ///
    /// （协程未设置Blocked状态即返回Pending）保存上下文 -> 设置状态为`Blocked` -> 从`CURRENT_TASK`上清除
    Blocked = 2,
    /// 退出状态。
    ///
    /// ### 时序
    ///
    /// （在任务内设置Exited状态）设置状态为`Exited` -> 保存上下文 -> 从`CURRENT_TASK`上清除
    ///
    /// （协程返回Ready）保存上下文（虽然此时的协程的上下文应该为空） -> 设置状态为`Exited` -> 从`CURRENT_TASK`上清除
    Exited = 3,
    /// 任务已加入阻塞队列，但还未保存上下文的状态。
    ///
    /// 增加此状态是为了避免在任务中设置了阻塞状态后，还未保存上下文就被取出执行的同步问题。
    ///
    /// 该状态只会由OS设置，在调度器中检测到`Blocking`状态后即会改为`Blocked`。
    ///
    /// ### 时序
    ///
    /// （在任务内设置Blocking状态）设置状态为`Blocking` -> 保存上下文 -> 设置状态为`Blocked` -> 从`CURRENT_TASK`上清除
    Blocking = 4,
}
