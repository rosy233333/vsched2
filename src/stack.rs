use core::task::Poll;

use crate::{current::get_current_task, get_sp, interface::{STACK_POOL_SIZE, Stack, StackVirtImpl, Task, TaskState}, set_sp};
use heapless::vec::Vec;

/// 获取栈类型，即是否是空栈
///
/// 如果返回 0，表示是空栈，否则表示栈使用的大小
#[inline(always)]
fn get_stack_type(stack_base: usize) -> usize {
    get_sp!() - stack_base
}

#[no_mangle]
#[naked]
unsafe extern "C" fn coroutine_trampoline() -> ! {
    core::arch::naked_asm!(
        "mv sp, a0",
        "j run_coroutine",
    );
}

#[no_mangle]
unsafe extern "C" fn run_coroutine() {
    let res = get_current_task().poll();
    match res {
        Poll::Ready(val) => {
            get_current_task().set_state(TaskState::Exited);
            todo!()
        }
        Poll::Pending => {
            get_current_task().set_state(TaskState::Ready); //TODO：这里也有可能是blocked状态
            todo!()
        }
    }
    // TODO：跳转到schedule函数
}

/// 对栈进行封装
///
/// 用来储存栈基址，后续可以加入新的内容。通过调用 alloc 和 dealloc 接口进行栈的分配和回收
///
/// ### 注意
///
/// 这里需要保证之后对 Task 进行封装时，线程的栈使用的是 StackWapper，否则在栈池中的`switch_to_thread_stack`会错误的调用 Drop
///
/// ### TODO
///
/// 考虑使用 manually drop 来解决上述问题
pub struct StackWapper {
    /// 栈基址
    pub base: usize,
}

impl StackWapper {
    /// 分配一个新的栈
    pub fn new() -> Self {
        Self {
            base: StackVirtImpl::alloc() as usize,
        }
    }

    /// 从一个已有的栈中获取一个新实例
    pub fn from_raw(base: usize) -> Self {
        Self { base }
    }
}

impl Drop for StackWapper {
    /// 回收栈
    fn drop(&mut self) {
        StackVirtImpl::dealloc(self.base as *mut ());
    }
}

/// 用户态的栈池管理器
///
/// 管理栈的分配和回收，提供栈切换的接口
pub struct StackHandler {
    /// 空栈的集合
    pub free_stacks: Vec<StackWapper, STACK_POOL_SIZE>,
    /// 当前使用的栈
    pub current_stack: Option<StackWapper>,
}

impl StackHandler {
    /// 创建一个新的栈管理器
    pub fn new() -> Self {
        let mut free_stacks = Vec::new();
        for _ in 0..STACK_POOL_SIZE - 1 {
            free_stacks.push(StackWapper::new());
        }
        Self {
            free_stacks,
            current_stack: Some(StackWapper::new()),
        }
    }

    /// 切换到空栈
    ///
    /// 对应黄色框部分
    pub fn get_empty_stack(&mut self) -> usize {
        if get_stack_type(self.current_stack.as_ref().unwrap().base) != 0 {
            let empty_stack = self.free_stacks.pop().expect("no free stack left");
            // set_sp(empty_stack.base);
            let current_stack = self.current_stack.replace(empty_stack).unwrap();
            self.free_stacks.push(current_stack);
        }
        self.current_stack.as_ref().unwrap().base
    }

    /// 切换到线程栈
    ///
    /// 对应蓝色框部分
    ///
    /// TODO: 在`StackWapper`使用`manually_drop`后需要修改这里的实现
    pub fn switch_to_thread_stack(&mut self, thread_stack: StackWapper) {
        let thread_stack_base = thread_stack.base;
        let current_stack = self.current_stack.replace(thread_stack).unwrap();
        if get_stack_type(current_stack.base) == 0 {
            self.free_stacks.push(current_stack);
        }
        set_sp!(thread_stack_base);
    }
}
