use core::task::Poll;

use crate::{
    current::get_current_task,
    get_sp,
    interface::{
        SMPVirtImpl, Stack, StackVirtImpl, Task, TaskState, CPU_NUM, SMP, STACK_POOL_SIZE,
    },
    set_sp, switch_sp_tratrampoline,
};
use heapless::vec::Vec;
use vdso_helper::get_vvar_data;

/// 获取栈类型，即是否是空栈
///
/// 如果返回 0，表示是空栈，否则表示栈使用的大小
///
/// TODO: 这里需要后续修改，用sp或fp判断栈类型不对（由于函数调用和局部变量，一定不等于栈顶）
#[inline(always)]
#[allow(dead_code)] // 之后再看要不要删掉
fn get_stack_type(stack_base: usize) -> usize {
    get_sp!() - stack_base
}

#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn coroutine_trampoline() -> ! {
    switch_sp_tratrampoline!(run_coroutine)
}

#[no_mangle]
#[unsafe(naked)]
unsafe extern "C" fn thread_trampoline() -> ! {
    switch_sp_tratrampoline!(run_thread)
}

// #[no_mangle]
// #[unsafe(naked)]
// unsafe extern "C" fn coroutine_into_user_trampoline() -> ! {
//     switch_sp_tratrampoline!(run_coroutine_into_user)
// }
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
        let base = StackVirtImpl::alloc() as usize;
        Self { base }
    }

    /// 从一个已有的栈中获取一个新实例
    pub fn from_raw(base: usize) -> Self {
        Self { base }
    }

    /// 回收栈
    pub fn dealloc(self) {
        StackVirtImpl::dealloc(self.base as *mut ());
    }
}

// impl Drop for StackWapper {
//     /// 回收栈
//     fn drop(&mut self) {
//         StackVirtImpl::dealloc(self.base as *mut ());
//     }
// }

impl Default for StackWapper {
    fn default() -> Self {
        Self { base: 0 }
    }
}

impl From<usize> for StackWapper {
    fn from(base: usize) -> Self {
        Self { base }
    }
}

impl From<StackWapper> for usize {
    fn from(stack: StackWapper) -> Self {
        stack.base
    }
}

/// 用户态的栈池管理器
///
/// 管理栈的分配和回收，提供栈切换的接口
pub struct StackHandler {
    /// 空栈的集合
    pub free_stacks: Vec<StackWapper, STACK_POOL_SIZE>,
    /// 当前使用的栈
    pub current_stack: [Option<StackWapper>; CPU_NUM],
}

impl StackHandler {
    /// 创建一个新的栈管理器
    pub fn new() -> Self {
        let mut stacks = Vec::new();
        for _ in 0..STACK_POOL_SIZE - CPU_NUM {
            stacks.push(StackWapper::new());
        }
        Self {
            free_stacks: stacks,
            current_stack: [Some(StackWapper::new()); CPU_NUM],
        }
    }

    pub fn alloc_stack(&mut self) -> StackWapper {
        self.free_stacks.pop().unwrap_or_else(|| {
            // 如果没有空栈，则分配一个新的栈
            StackWapper::new()
        })
    }

    pub fn dealloc_stack(&mut self, stack: StackWapper) {
        self.free_stacks.push(stack).unwrap_or_else(|stack| {
            // 如果栈池已满，则回收栈
            stack.dealloc();
        });
    }

    pub fn set_current_stack(&mut self, stack: StackWapper, cpu_id: usize) -> StackWapper {
        self.current_stack[cpu_id]
            .replace(stack)
            .expect("Error: Failed to set current stack")
    }

    /// 切换到空栈
    ///
    /// 对应黄色框部分
    ///
    /// 参数：
    /// - `stack_status`: 代表当前栈的状态，0为空栈，1为非空栈。
    pub fn get_empty_stack(&mut self, stack_type: usize) -> usize {
        let cpu_id = SMPVirtImpl::cpu_id();
        if stack_type != 0 {
            let empty_stack = self.alloc_stack();
            let old_stack = self.set_current_stack(empty_stack, cpu_id);
            self.dealloc_stack(old_stack);
        }
        self.current_stack[cpu_id].as_ref().unwrap().base
    }

    /// 切换到线程栈
    ///
    /// 对应蓝色框部分
    ///
    /// 参数：
    /// - `thread_stack`: 代表线程栈，如果为 None，则不设置当前栈，否则将当前栈设置为 thread_stack。
    /// - `stack_status`: 代表当前栈的状态，0为空栈，1为非空栈。
    pub fn get_thread_stack(&mut self, thread_stack: Option<StackWapper>, stack_type: usize) {
        let old_stack = {
            if let Some(stack) = thread_stack {
                self.set_current_stack(stack, SMPVirtImpl::cpu_id())
            } else {
                self.current_stack[SMPVirtImpl::cpu_id()].take().unwrap()
            }
        };
        if stack_type == 0 {
            self.dealloc_stack(old_stack);
        }
    }
}

impl Default for StackHandler {
    fn default() -> Self {
        // Self::new() 这样合适还是现在这样全0之后再初始化合适？
        Self {
            free_stacks: Vec::new(),
            current_stack: [None; CPU_NUM],
        }
    }
}
