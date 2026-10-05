#[allow(non_snake_case)]

fn main() {
    vdso_helper::mut_cfg! {
        /// CPU核心数量
        const CPU_NUM: usize = 1;
        /// 单个调度器内的事件源最大数量
        const EVENT_SORCE_NUM: usize = 8;
        /// 就绪队列大小
        const READY_QUEUE_SIZE: usize = 256;
        /// trap等待队列大小
        const TRAP_WAIT_QUEUE_SIZE: usize = 256;
        /// 单个调度器内的阻塞队列数量
        ///
        /// 上限为64，因为阻塞队列号的分配使用一个`AtomicU64`位图。
        const BLOCK_QUEUE_NUM: usize = 16;
        /// 单条阻塞队列中阻塞任务的数量上限
        const BLOCK_QUEUE_SIZE: usize = 64;
        /// 任务最低优先级
        const LOWEST_PRIORITY: isize = 15;
        /// 任务最高优先级
        const HIGHEST_PRIORITY: isize = 0;
        /// 进程数量上限（全局进程表的大小）
        const PROCESS_NUM: usize = 256;
        /// 栈池大小
        ///
        /// TODO: 后续需要讨论调整
        const STACK_POOL_SIZE: usize = 16;
    }
}
