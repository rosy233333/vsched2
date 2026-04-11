fn main() {
    vdso_helper::mut_cfg! {
        /// CPU核心数量
        const CPU_NUM: usize = 1;
        /// 单个调度器内的事件源最大数量
        const EVENT_SORCE_LEN: usize = 8;
        /// 进程数量上限（全局进程表的大小）
        const PROCESS_NUM: usize = 256;
        /// 栈池大小
        ///
        /// TODO: 后续需要讨论调整
        const STACK_POOL_SIZE: usize = 16;
    }
}
