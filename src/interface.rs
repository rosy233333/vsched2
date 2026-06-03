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
        // /// 保存线程上下文
        // fn save_thread_context(&self);
        // /// 保存trap上下文
        // fn save_trap_context(&self);
        /// 恢复寄存器上下文（可能为线程上下文或trap上下文）
        fn restore_context(&self);
        /// 恢复协程上下文，函数返回时自动保存了协程上下文
        fn poll(&self) -> Poll<isize>;
        /// 获取线程上下文保存的栈底指针
        fn thread_stack_base(&self) -> usize;
        /// 设置协程运行返回值
        fn set_return_value(&self, value: isize);
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
        /// 
        /// 调度器模块保证不会回收初始栈。
        fn dealloc(stack: *mut ());
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
        /// 在内核态切换地址空间
        ///
        /// 以进程号（全局进程表中的索引）表示地址空间。
        ///
        /// 若位于内核，则进程号为当前地址空间所属进程的进程号。
        /// （这一点与调度器中不同，因为同一个调度器可以包含属于多个地址空间的内核态任务。）
        fn switch_vspace(vspace_pid: *const ());
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
    /// （在任务内设置Blocked状态）设置状态为`Blocked` -> 保存上下文 -> 从`CURRENT_TASK`上清除
    ///
    /// （协程未设置Blocked状态即返回Pending）保存上下文 -> 设置状态为`Blocked` -> 从`CURRENT_TASK`上清除
    ///
    /// 注意：本模块未规定设置Blocked状态与加入阻塞队列的时序关系。用户需考虑Blocked状态的更新时机，
    /// 避免出现任务已设置了`Blocked`状态，还在运行时，就被唤醒并放在另一核心上运行的情况。
    Blocked = 2,
    /// 退出状态。
    ///
    /// ### 时序
    ///
    /// （在任务内设置Exited状态）设置状态为`Exited` -> 保存上下文 -> 从`CURRENT_TASK`上清除
    ///
    /// （协程返回Ready）保存上下文（虽然此时的协程的上下文应该为空） -> 设置状态为`Exited` -> 从`CURRENT_TASK`上清除
    Exited = 3,
}
