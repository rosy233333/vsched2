# 问题台账

> 本文件是索引，不是分析文档。详细分析放在 `详见` 指向的文档；地图见 [README.md](README.md)，约定见 [conventions.md](conventions.md)。

## 状态与排序

- `open`：未决，没有倾向。
- `leaning`：有倾向，未拍板。
- `decided`：已决定；`详见` 指向 decision/design。
- `dropped`：考虑过并放弃；保留，记录负结论。
- 当前切片（暂定）：步骤 1——调度循环修改，以修正后的“任务切换模型（新版调度算法）”图为准。
- 优先级：`P0` 不解决就无法写当前切片的 design/plan；`P1` 当前切片可绕过、但下一片前必须解决；`P2` 重构完成前；`P3` 清理/以后。
- 排序：按主题分节；`## Open`（open/leaning，同等）在前，`## Closed`（decided/dropped，同等）在后；每个主题内按 `P0 → P3`，同优先级按 `Q-ID` 升序。
- P0 速查：Q-005、Q-006。

## Open

### 任务模型与状态

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-005 | P0 | 任务进入/离开事件源、就绪队列、阻塞队列的时机？入队/出队与状态修改的先后？ | 设计 | 队列/状态 | 未定；后续步骤还会修改调度循环 | 状态机/可重入 | [设计·调度循环](design/scheduling-loop.md) | open |
| Q-003 | P1 | 任务接口是否按 `数据结构修改` 收窄/改名（`set_state`、`is_coroutine`、`resched`、`restore_context`）？ | 接口 | `Task` trait/OS 适配 | 有提案，未拍板 | 接口 | [数据结构修改·任务](<draft/新版设计/数据结构修改.md#任务>) | leaning |
| Q-010 | P1 | 任务状态是否作为上下文的一部分，随嵌套现场保存/恢复？ | 设计 | 状态覆盖 | 有提案，未定 | 可重入 | [可重入·状态覆盖](<draft/可重入分析/调度器可重入设计文档.md#2-任务状态覆盖>) | open |
| Q-032 | P2 | `Task::dealloc` 需要补充哪些 action、出队和锁清理？ | 实现 | 生命周期/清理 | `dealloc` 无 action 参数；清理依赖调用点 | 生命周期 | [数据结构修改·任务](<draft/新版设计/数据结构修改.md#任务>)、[interface.rs:82](../src/interface.rs#L82) | open |

### 接口与模块边界

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-041 | P1 | 如何实现跨进程唤醒用户任务，并让唤醒携带数据？ | 设计 | 跨进程唤醒/IPC | 方案选定 VVAR 接收队列；数据/无在线任务/抢占等未决 | IPC/Step 2 | [设计·跨进程唤醒](design/cross-process-wake.md)、[0003 Step 2](plans/0003-scheduler-refactor/README.md) | leaning |
| Q-042 | P1 | 同步系统调用与异步系统调用是否共用 TrapInfo 发送队列？ | 设计 | 队列/安全/所有权 | 可能复用；队列需放 VVAR，安全/所有权未定 | IPC/Step 2 | [设计·通信](design/communication.md)、[0003 Step 2](plans/0003-scheduler-refactor/README.md) | open |
| Q-044 | P1 | 接收方进程没有在线任务时，接收队列如何搬运任务并更新进程优先级？ | 设计 | 优先级/接收队列 | 接收队列不参与优先级；无在线任务时无法及时搬运 | 通信/Step 2 | [设计·通信](design/communication.md) | open |
| Q-048 | P1 | 内核态可能同时操作用户态任务和内核态任务两种 `Task` 实现，但 `TaskVirtImpl` 不支持运行时多态（只能绑定一个实现），如何统一/选择？ | 接口 | 任务多态/内核访问 | 未定；`TaskVirtImpl` 只能绑定一个实现 | 可重入/Step 3 | [可重入·空间维度](<draft/可重入分析/调度器可重入设计文档.md#空间维度共享数据列表>)、[interface.rs:10](../src/interface.rs#L10) | open |
| Q-022 | P2 | 异步系统调用模块是否独立 vDSO / 归入 IPC 模块？ | 设计 | 模块边界 | 倾向独立 | 模块拆分 | [代码实现设计·异步系统调用模块](<draft/新版设计/共享调度器模块代码实现设计.md#异步系统调用模块>) | leaning |

### 可重入与并发

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-006 | P0 | 调度模块的加锁/释放锁时机、锁序，以及开关中断的时机/范围？哪些锁允许在用户态持有？ | 并发 | 锁/IRQ/可重入 | 未定；IRQ 时机可能再改 | 可重入 | [设计·调度循环](design/scheduling-loop.md)、[设计·路径枚举](<draft/新版设计/共享调度器模块设计文档.md#全部路径枚举>) | open |
| Q-008 | P1 | 可重入判定标准的 P3/P4 是什么？ | 设计 | 可重入评审 | 只写到 P2 | 可重入 | [可重入·判定标准](<draft/可重入分析/调度器可重入设计文档.md#状态一致判定标准>) | open |
| Q-009 | P1 | 上下文栈的结构、同步、容量、恢复顺序？恢复时的拷贝能否优化掉？ | 设计 | 上下文栈/可重入 | 候选 `Vec`/无锁栈，未定 | 可重入 | [可重入·上下文覆盖](<draft/可重入分析/调度器可重入设计文档.md#1-任务上下文覆盖>) | open |
| Q-011 | P1 | 如何防止重复入队/重复唤醒？上一任务入队、`CURRENT_TASK` 更新、下一任务领取、上下文恢复的交接协议是什么？ | 并发 | 队列/交接 | 暂无 | 可重入 | [可重入·重复唤醒](<draft/可重入分析/调度器可重入设计文档.md#3-任务重复唤醒>) | open |
| Q-012 | P1 | 用户态持锁被中断、内核取同一把锁的死锁如何消除？哪些锁必须移出跨特权级路径？ | 并发 | 锁/死锁 | 暂无 | 可重入 | [可重入·锁的重入](<draft/可重入分析/调度器可重入设计文档.md#4-锁的重入>) | open |
| Q-014 | P1 | 是否保留“内核直接运行用户任务”路径？ | 设计 | 锁重入/切换开销 | 倾向不改 | 可重入 | [可重入·锁的重入](<draft/可重入分析/调度器可重入设计文档.md#4-锁的重入>) | leaning |
| Q-018 | P1 | 实现可重入后，`SCHEDULER_WAIT_CONTEXT` 是否可删除/优化？ | 设计 | 等待上下文 | 待可重入设计 | 可重入 | [可重入·共享数据](<draft/可重入分析/调度器可重入设计文档.md#空间维度共享数据列表>) | open |

### 栈与切换

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-016 | P1 | 如何最终解决调度模块与线程共享同一个栈的同步问题（`take_current_stack` 临时设计）？线程主动让出后以非空栈进入调度模块时，同一栈可能被调度模块和其它核心上恢复的线程同时使用。 | 实现 | 栈/调度循环 | 临时设计；Step 3 可能重排栈保存顺序 | 栈 | [api.rs:409](../src/api.rs#L409)、[设计·调度循环](design/scheduling-loop.md) | open |
| Q-027 | P1 | `main_loop` 中栈切换是否影响函数返回？如何验证？ | 实现 | 栈/切换 | 未验证 | 栈 | [main_loop.rs:641](../src/main_loop.rs#L641) | open |
| Q-035 | P2 | `Context::into_user` 的用户栈分配/回收是否进入 `Context` trait？ | 接口 | 用户栈 | 在内核分配用户栈会增加“内核访问用户数据”情景，不利于可重入；`into_user(ustack)` 接收栈顶，无回收接口 | 栈 | [代码实现设计·任务切换](<draft/新版设计/共享调度器模块代码实现设计.md#任务切换模块>)、[interface.rs:112](../src/interface.rs#L112) | open |

### trap

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-049 | P1 | `trap_handler` 完成后，是直接恢复对应的内核影子任务（不走调度、无抢占），还是正常走调度（有抢占但需管理多个影子任务）？ | 设计 | trap 恢复/抢占 | 倾向直接恢复；代价是无抢占 | 可重入/Step 3 | [设计·内核影子任务](design/kernel-shadow-task.md)、[0003 Step 3](plans/0003-scheduler-refactor/README.md) | leaning |
| Q-024 | P2 | `trap_wait_queue` 中“用户态任务一定位于当前地址空间”的假设是否成立？ | 实现 | trap/地址空间 | 未确认 | trap | [trap_wait_queue.rs:160](../src/schedule/trap_wait_queue.rs#L160) | open |
| Q-033 | P2 | `TrapInfo::handle` 收窄后，处理任务通过什么接口接收通信？ | 接口 | trap 通信 | `handle` 仍直接接收被 trap 任务 | trap | [数据结构修改·trap](<draft/新版设计/数据结构修改.md#trap-相关>)、[interface.rs:140](../src/interface.rs#L140) | open |
| Q-040 | P2 | “内核态→用户态”的“线程/协程→中断”路径需要先回用户态选任务再陷入内核恢复，两次特权级切换的性能开销是否可接受？ | 设计 | 切换/性能 | 路径表已列，未评估 | 性能/Step 1 | [设计·调度循环](design/scheduling-loop.md) | open |
| Q-021 | P3 | 内核态服务任务（`trap_handler`、异常处理等）是否也关闭中断以优化性能？ | 设计 | 中断/性能 | 后续设计未考虑 | 设计 | [设计·中断](<draft/新版设计/共享调度器模块设计文档.md#中断>) | open |

### 共享数据与初始化

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-038 | P1 | VVAR 中进程优先级如何收敛为仅最高优先级？进程选择如何统一到内核？ | 设计 | 优先级/共享数据 | 方向已定：仅共享最高优先级、内核选择进程；细节待 Step 1.1 | 调度器 | [0003 Step 1](plans/0003-scheduler-refactor/README.md#步骤-1调度循环修改)、[process_info.rs:32](../src/schedule/process_info.rs#L32) | leaning |
| Q-019 | P3 | `KERNEL_SCHEDULER` / `KERNEL_STACKS` 是否还需要放在 VVAR？能否直接用 `USER_SCHEDULER` / `STACK_HANDLER`？ | 实现 | 共享数据/初始化 | 倾向不必要，未改 | Phase 2 | [可重入·共享数据](<draft/可重入分析/调度器可重入设计文档.md#空间维度共享数据列表>) | leaning |
| Q-028 | P3 | `user_init` 中“用户态不需要初始化 `CURRENT_TASK`/`IN_KERNEL`/`STACK_HANDLER`/`CURRENT_VSPACE`”是否成立？ | 实现 | 初始化 | 存疑 | 初始化 | [api.rs:267](../src/api.rs#L267) | open |
| Q-047 | P3 | 删除事件源后，是否可简化进程的初始化过程，合并`process_init`和`user_init`？ | 实现 | 初始化 | 存疑 | 初始化 | [api.rs:153](../src/api.rs#L153)、[api.rs:258](../src/api.rs#L258) | open |

### 多核与休眠

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-036 | P1 | 空栈进入调度模块是否必须？多核同步如何解决？非空栈路径也有同一栈被调度器/其它核心线程同时使用的风险。 | 设计 | 多核/栈 | 当前有空栈/非空栈入口；Step 3 可能重排栈保存顺序 | 多核 | [设计·调度循环](design/scheduling-loop.md)、[main_loop.rs:54](../src/main_loop.rs#L54) | open |
| Q-017 | P2 | 多核唤醒/IPI 策略：恢复主动 IPI，还是只靠时钟中断？ | 实现 | 多核/唤醒 | 当前不主动 IPI | 多核 | [scheduler.rs:373](../src/schedule/scheduler.rs#L373) | open |

## Closed

| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
|---|---|---|---|---|---|---|---|---|
| Q-001 | P1 | `TaskState::Blocking` 能否删除？删除后如何避免“已入阻塞队列但未保存上下文”的竞态？ | 设计 | 状态机/接口 | 已在“清晰调度器边界修改”中删除 Blocking；代码残留由额外步骤清理 | — | [方案2流程图](<archive/2.清晰调度器边界修改/任务及其状态变化流程图（新版，方案2）.drawio>) | decided |
| Q-002 | P2 | “事件源”接口是否改名？改成什么？ | 接口 | 接口/OS 适配 | 事件源已删除；旧问题关闭 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | dropped |
| Q-004 | P2 | 删除 `TrapInfo::new_handler` / `IdleHandler` 后，trap 处理任务和 `SCHEDULER_WAIT_CONTEXT` 如何创建？ | 接口 | trap/初始化 | 不删除；保留 `new_handler` / `IdleHandler` | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | decided |
| Q-007 | P0 | 内核恢复无法在用户态恢复的任务（中断上下文）的路径？状态更新与任务所有权如何保证？ | 设计 | 中断上下文/所有权 | 路径已写入 `design/scheduling-loop.md`；任务归属由 D-001 固定 | — | [设计·调度循环](design/scheduling-loop.md)、[D-001](decisions/0001-user-task-interrupt-ownership.md) | decided |
| Q-013 | P1 | `CURRENT_VSPACE` 在用户调度器提前设置、内核看到错误地址空间，如何修？ | 并发 | 地址空间 | 新调度循环中 `CURRENT_VSPACE` 只由内核修改；用户调度器不再提前设置 | — | [0003 Step 1](plans/0003-scheduler-refactor/README.md#步骤-1调度循环修改)、[设计·调度循环](design/scheduling-loop.md) | decided |
| Q-015 | P1 | 栈的保留与回收时机？仍被当前执行流/挂起现场引用的栈如何防止提前复用？ | 设计 | 栈生命周期 | 空闲栈池设计为 per-CPU，防止提前复用 | — | [任务切换模型](plans/0003-scheduler-refactor/任务切换模型（0003版）.drawio) | decided |
| Q-020 | P2 | 内核任务与用户任务的 TCB 是否统一保存？如何保存？ | 设计 | TCB/接口 | 保存在各自地址空间，不统一保存 | — | [数据结构修改·任务](<draft/新版设计/数据结构修改.md#任务>) | decided |
| Q-023 | P2 | 内核是否访问调度器的 `sources` 字段？`init_except_sources` 的假设是否成立？ | 实现 | 初始化/共享数据 | `Scheduler.sources` 删除；旧问题关闭 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | dropped |
| Q-025 | P2 | `get_stack_type` 用 sp/fp 判断空栈是否正确？如何替换？ | 实现 | 栈/调度循环 | 不再使用 `get_stack_type` 判断栈类型；代码残留由额外步骤清理 | — | [trampoline.rs:11](../src/stack/trampoline.rs#L11)、[0003 额外步骤](plans/0003-scheduler-refactor/README.md) | decided |
| Q-026 | P2 | `wait_for_runnable_task` 的休眠循环 bug 如何解决？ | 实现 | 多核/休眠 | 与 Q-017 是同一问题，已合并跟踪 | — | [Q-017](questions.md)、[main_loop.rs:378](../src/main_loop.rs#L378) | dropped |
| Q-029 | P0 | `trap_handler` 是否与调度代码合并？ | 设计 | trap/调度循环 | 不合并；handler 独立，简单 trap 内联，复杂才建任务 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | decided |
| Q-030 | P0 | `TrapInfo` 在 trap 入口还是 `trap_handler` 中入队？ | 设计 | trap 队列/职责 | 在 trap 入口入队 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | decided |
| Q-031 | P2 | `Task::register_waker` 的语义和所有权是什么？与 `block_and_wake/waker.rs` 如何统一？ | 接口 | 唤醒/接口 | 不需要该接口；协程 `poll` 会自动注入 waker | — | [数据结构修改·任务](<draft/新版设计/数据结构修改.md#任务>)、[waker.rs](../src/block_and_wake/waker.rs) | decided |
| Q-034 | P3 | 事件源是否需要动态创建？生命周期由谁管理？ | 设计 | 事件源生命周期 | 事件源删除；动态创建不再需要 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | dropped |
| Q-037 | P3 | 调度器 `sources` 是否需要活跃标志？ | 设计 | 调度查询/性能 | `sources` 删除；活跃标志不再需要 | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | dropped |
| Q-039 | P1 | Step 1 能否阻止“中断→线程/中断”非法路径？是否必须在 Step 2 让中断处理不再走调度？ | 设计 | trap/Step 2 | 新 trap 机制保证中断后一定运行 trap_handler | — | [0003 Step 2](plans/0003-scheduler-refactor/README.md) | decided |
| Q-043 | P1 | 一对一接收方用 `SchedAction::JustBlock` 等待发送方时，如何避免丢失唤醒？ | 并发 | 唤醒/所有权 | 改用 Future 阻塞流程；接收队列任务按唤醒流程，未阻塞只改 action 为 `Yield` | — | [设计·通信·一对一](design/communication.md#接收方无批处理要求一对一)、[方案2流程图](<archive/2.清晰调度器边界修改/任务及其状态变化流程图（新版，方案2）.drawio>) | decided |
| Q-045 | P1 | 批处理接收方任务如何公布自身任务指针？ | 接口 | 指针/所有权 | 不再单独公布；VVAR 共享阻塞队列 + 信号量，发送唤醒后放入接收队列 | — | [设计·通信·批处理](design/communication.md#接收方有批处理要求多对一) | decided |
| Q-046 | P1 | 如何判断批处理接收方任务不在线，并与唤醒同步以避免丢失/重复唤醒？ | 并发 | 离线检测/唤醒 | 由共享阻塞队列信号量同步；发送唤醒、处理递减、0/负阻塞 | — | [设计·通信·批处理](design/communication.md#接收方有批处理要求多对一) | decided |

关闭项保留原行；`decided` 是已决定，`dropped` 是考虑过并放弃。超过约 300 行或 100 条时，把较早的关闭项移到 `questions-closed.md`。
