---
status: draft
applies_to:
  - src/interface.rs
  - src/main_loop.rs
  - src/current.rs
  - src/schedule
  - src/stack
---

# 新版设计草稿：已计划修改项

> 对比 `doc/draft/新版设计/` 与当前 `src/`，只记录草稿里明确计划修改的项；概念说明、完整设计和未决分析仍以草稿/`questions.md` 为准。本文件是差异清单，不是第二份 design。

## 问题

`doc/draft/新版设计/` 混合了“已实现、计划修改、待定方向”，读起来无法判断哪些是接下来真要改的。需要一份清单，把草稿中的修改项、原因、当前实现和状态放在一起，供计划和接口变更使用。

## 方法

- 阅读 `共享调度器模块设计文档.md`、`共享调度器模块代码实现设计.md`、`数据结构修改.md`。
- 三篇文字文档为主；`*.drawio` 用于核对路径。图中额外标注“空栈进入调度模块的多核同步问题”，作为待确认项，不进入修改项表。
- 提取“是否需要修改”“对原有设计做如下修改”“需要进一步考虑”等明确表述；不把纯概念说明当成修改项。
- 对照 `src/interface.rs`、`src/main_loop.rs`、`src/current.rs`、`src/schedule/`、`src/stack/`，标注当前实现。
- 状态只表示“代码与草稿的差距”，不表示优先级；P0/P1 仍由 `plans/0003-scheduler-refactor` 和 `questions.md` 决定。

### 状态图例

- `已实现`：当前代码已有等价实现，可能命名不同。
- `部分实现`：关键部分存在，但草稿要求的语义/接口未完整落地。
- `未实现`：当前代码没有等价实现。
- `待定`：草稿自己标注“需要进一步考虑”，还没有固定目标。
- `文档待更新`：代码与草稿不一致，但不一定是“要改代码”；先确认哪边是目标。

## 数据

### 任务模型与 Task 接口

| 修改项 | 草稿计划 | 原因 | 当前实现 | 状态 | 来源 |
|---|---|---|---|---|---|
| 删除 `TaskState::Blocking` | 去掉过渡态，阻塞入队/状态切换由调度模块在同一临界区完成 | 避免“已入阻塞队列但未保存上下文”的竞态，简化状态机 | `TaskState` 仍含 `Blocking`，`main_loop` 和阻塞队列仍在设置 | 未实现 | [设计·任务](<../draft/新版设计/共享调度器模块设计文档.md#任务>)、[interface.rs:226](../../src/interface.rs#L226) |
| `Task::set_state` 只读化 | 状态转换由调度模块完成，OS 只保留读状态接口 | 避免 OS 与调度模块同时改状态，统一临界区 | `set_state` 仍是公开接口，OS 在 trap/阻塞路径直接调用 | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:16](../../src/interface.rs#L16) |
| 新增 `SchedAction` | 用 action 表达主动让权原因：yield、block、exit；trap 不使用 | 调度器需要知道任务为什么进入调度器 | 已实现为 `SchedAction` + `set_action` | 已实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:273](../../src/interface.rs#L273) |
| `is_coroutine()` → `context_type()` | 从布尔判断扩展为协程/线程/中断三类上下文 | 恢复路径必须区分能否在用户态恢复；中断上下文要在内核恢复 | 仍只有 `is_coroutine() -> bool` | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:32](../../src/interface.rs#L32) |
| 新增 `Task::register_waker` | 向用户任务提供调度模块的 waker | 用户任务在 `poll` 的 `Context` 中保存调度模块 waker，统一唤醒路径 | 没有该接口；waker 在 `block_and_wake/waker.rs` 内部 | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[waker.rs](../../src/block_and_wake/waker.rs) |
| `resched()` → `save_thread_context` | 改名并增加 action 操作 | 只保存线程现场，语义比“重新调度”更窄 | 仍叫 `resched`，API 调用它进入调度 | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:56](../../src/interface.rs#L56) |
| `restore_context()` → `restore_thread_context()` | 只恢复用户态可恢复的线程上下文 | 中断上下文不能由用户态完整恢复，必须由内核恢复 | 仍叫 `restore_context`，注释允许线程或 trap 上下文 | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:60](../../src/interface.rs#L60) |
| `Task::dealloc` 补充清理 | 补充 action，检查出队和释放锁 | 退出任务不能留下队列项/锁 | `dealloc` 存在，但没有 action 参数；清理依赖调用点 | 未实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:82](../../src/interface.rs#L82) |
| 内核/用户 TCB 统一保存 | 明确内核任务与用户任务的 TCB 保存方式 | 目前 TCB 由 OS 定义，内核/用户路径可能不同 | `Task` 不规定 TCB；由 OS 实现 | 待定 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>) |
| 上下文改为可嵌套容器 | TCB 中保存嵌套现场，候选是 `Vec<上下文>` | 调度代码被中断时不能覆盖已有现场；保存/恢复时也可开中断 | `Task` 只有 `thread_stack`/`is_coroutine`，没有上下文容器 | 待定 | [设计·可重入](<../draft/新版设计/共享调度器模块设计文档.md#用户调度代码可重入>)、[数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>) |
| 分清 `pid` 与 `is_kernel` | 地址空间 id 与任务特权级分开 | 任务特权级与当前 CPU 特权级不同，混用会判断错恢复路径 | `pid`/`is_kernel` 已分开，`IN_KERNEL` 单独表示当前特权级 | 已实现 | [数据结构修改·任务](<../draft/新版设计/数据结构修改.md#任务>)、[interface.rs:41](../../src/interface.rs#L41) |

### 调度模块与事件源

| 修改项 | 草稿计划 | 原因 | 当前实现 | 状态 | 来源 |
|---|---|---|---|---|---|
| “事件源”改名 | 给事件源接口换一个更合适的名字 | 它不只“产生事件”，还要保存等待任务、接收事件、提供就绪任务 | 仍叫 `EventSource` / `EventSourceVtable` | 未实现 | [设计·事件源](<../draft/新版设计/共享调度器模块设计文档.md#事件源>)、[event_source.rs:119](../../src/schedule/event_source.rs#L119) |
| 扩展事件源职责 | 事件源对象应保存等待任务、接收并处理事件、提供就绪任务；可动态创建，一个事件源可对应多个任务 | 通信结果必须落到事件源；事件源是就绪任务进入调度器的接口 | 当前 trait 只有 `hightest_priority`/`take_task`；实现可自行保存任务，但接口不表达完整职责 | 部分实现 | [设计·事件源](<../draft/新版设计/共享调度器模块设计文档.md#事件源>)、[event_source.rs:119](../../src/schedule/event_source.rs#L119) |
| 事件源数组加活跃标志 | `sources` 元素包含事件源指针、vtable、活跃标志 | 不活跃的事件源可直接返回最低优先级，降低查询开销 | `EventSourceEntry` 保存内核/用户两份数据指针和 vtable，没有活跃标志 | 文档待更新 | [代码实现设计·数据结构](<../draft/新版设计/共享调度器模块代码实现设计.md>)、[scheduler.rs:52](../../src/schedule/scheduler.rs#L52) |
| VVAR 优先级收敛 | 只共享最高优先级；进程选择在内核完成 | 减少 VVAR 之外的跨边界访问 | 当前按进程/CPU 保存 `highest_prio` 并现场扫描；Step 1 计划收敛 | 未实现 | [0003 Step 1](../plans/0003-scheduler-refactor/README.md#步骤-1调度循环修改)、[process_info.rs:32](../../src/schedule/process_info.rs#L32) |

### 上下文、切换与栈

| 修改项 | 草稿计划 | 原因 | 当前实现 | 状态 | 来源 |
|---|---|---|---|---|---|
| `Task` 不再提供保存上下文接口 | 保存上下文全部交给 OS 的汇编约定；`Task` 只保留恢复接口 | 原设计会在上下文保存函数外再保存一次，且恢复后会再次进入调度器 | 当前 `Task` 没有通用 `save_context`；汇编 `raw_trap_entry` 负责保存 | 部分实现 | [代码实现设计·寄存器上下文](<../draft/新版设计/共享调度器模块代码实现设计.md#寄存器上下文保存的约定>)、[riscv.rs:229](../../src/arch/riscv.rs#L229) |
| `ktrap_entry`/`utrap_entry` 合并 | 合并为一个入口函数 | 统一入口，减少重复分支 | 代码只有 `trap_entry`；`utrap_entry` 已注释掉 | 已实现 | [代码实现设计·寄存器上下文](<../draft/新版设计/共享调度器模块代码实现设计.md#寄存器上下文保存的约定>)、[main_loop.rs:54](../../src/main_loop.rs#L54) |
| `trap_entry` 重新分配预保存栈 | trap 入口把新栈写入 `sscratch`/`uscratch` | trap 处理任务会使用原预保存栈，必须补充 | `trap_entry` 调用 `alloc_stack`/`set_pre_stack!`/`set_trap_stack` | 部分实现 | [代码实现设计·寄存器上下文](<../draft/新版设计/共享调度器模块代码实现设计.md#寄存器上下文保存的约定>)、[main_loop.rs:70](../../src/main_loop.rs#L70) |
| `into_user` 构造用户上下文并管理用户栈 | `into_user` 需构造用户态上下文、分配用户栈，并可回收 | 用户态上下文必须带用户栈，栈生命周期要可管理 | `Context::into_user(ustack)` 接收栈顶；没有 `run_task_user`/`recycle_ustack` 接口 | 部分实现 | [代码实现设计·任务切换](<../draft/新版设计/共享调度器模块代码实现设计.md#任务切换模块>)、[interface.rs:112](../../src/interface.rs#L112) |

### trap

| 修改项 | 草稿计划 | 原因 | 当前实现 | 状态 | 来源 |
|---|---|---|---|---|---|
| `TrapInfo::handle` 只分类并发通信 | 只区分 trap 原因，向对应处理任务发送通信，不直接处理业务 | trap 处理应由专门任务完成，`trap_handler` 只做分发 | `trap_handler` 仍调用 `trap_info.handle(task)`，`handle` 仍接收被 trap 任务 | 未实现 | [数据结构修改·trap](<../draft/新版设计/数据结构修改.md#trap-相关>)、[interface.rs:140](../../src/interface.rs#L140) |
| 删除 `TrapInfo::new_handler` | 不再由 `TrapInfo` 创建独立分发任务 | 分发任务应由事件源/队列管理，不应绑定在 TrapInfo 上 | `new_handler` 仍存在，`api` 和 `trap_wait_queue` 仍在用 | 未实现 | [数据结构修改·trap](<../draft/新版设计/数据结构修改.md#trap-相关>)、[interface.rs:145](../../src/interface.rs#L145) |
| 调整 `TrapItem`/`TrapQueue`/`TrapWaitQueue` | 缓存 trap，`trap_handler` 不额外操作，处理任务再取出 | TrapInfo 属于事件源；处理任务按需取，避免重复入队 | `TrapWaitQueue::push_trap` 缓存，`trap_handler` 从队列取出 | 部分实现 | [数据结构修改·trap](<../draft/新版设计/数据结构修改.md#trap-相关>)、[trap_wait_queue.rs:89](../../src/schedule/trap_wait_queue.rs#L89) |
| 删除 `IdleHandler`/`IdleHandlerQueue` | 去掉空闲分发任务管理 | 与新的队列/任务复用机制重复 | 没有 `IdleHandler` 类型，但 `TrapWaitQueue.idle_handlers` 字段和 `get_idle_handler` 仍在 | 部分实现 | [数据结构修改·trap](<../draft/新版设计/数据结构修改.md#trap-相关>)、[trap_wait_queue.rs:39](../../src/schedule/trap_wait_queue.rs#L39) |
| `trap_handler` 与调度代码合并 | 把 trap_handler 作为调度代码的一部分 | 减少一次任务切换，统一调度 | `trap_handler` 仍是独立任务/函数；是否合并待定 | 待定 | [设计·trap_handler](<../draft/新版设计/共享调度器模块设计文档.md#trap_handler的内容>)、[api.rs:377](../../src/api.rs#L377) |
| TrapInfo 在 trap 入口后入队 | 不在 `trap_handler` 里入队，而在入口后的调度代码中做 | TrapInfo 是硬件通信结果，应进入 trap 事件源 | `trap_entry` 调用 `push_trap` | 已实现 | [设计·trap_handler](<../draft/新版设计/共享调度器模块设计文档.md#trap_handler的内容>)、[main_loop.rs:113](../../src/main_loop.rs#L113) |

### 模块边界与初始化

| 修改项 | 草稿计划 | 原因 | 当前实现 | 状态 | 来源 |
|---|---|---|---|---|---|
| 异步系统调用独立 vDSO | 作为相对独立的模块，不放同一个 vDSO，归入 IPC | 它属于 IPC，不是调度核心 | 当前仓库没有该模块 | 未实现 | [代码实现设计·异步系统调用模块](<../draft/新版设计/共享调度器模块代码实现设计.md#异步系统调用模块>) |
| 内核服务任务也关闭中断 | 考虑 trap_handler、异常处理等内核服务任务关中断，需进一步商讨 | 降低内核服务被中断的开销 | 后续设计未采用；当前按原有中断策略 | 待定 | [设计·中断](<../draft/新版设计/共享调度器模块设计文档.md#中断>) |

### 可重入（步骤 3）

这些项在 `共享调度器模块设计文档.md` 的“备忘”中明确列为尚未完成；它们是计划步骤 3 的核心，全部未实现：

| 修改项 | 草稿计划 | 原因 | 状态 |
|---|---|---|---|
| 嵌套现场 | 明确归属、完整保存的发布时机、恢复顺序、容量边界 | 仅增加上下文容器不足以证明正确 | 未实现 |
| 状态/action 与并发通知 | 恢复旧现场不能回滚期间已发生的唤醒/完成/退出 | 状态和通知必须一起设计 | 未实现 |
| 执行权交接 | 明确入队、当前任务更新、下一任务领取、上下文恢复的顺序 | 防止重复入队、重复运行、并发 poll | 未实现 |
| 跨特权级锁 | 明确挂起调度过程持锁时能否启动新用户调度过程 | 内核不访问用户队列也可能等待原持锁者而死锁 | 未实现 |
| 栈保留/回收 | 仍被当前执行流或挂起现场引用的栈不能提前复用 | 栈复用会覆盖现场 | 未实现 |

## 结论

- 草稿里**已经落地**的主要是：`SchedAction`、合并 `ktrap_entry`/`utrap_entry`、TrapInfo 在入口后入队、`pid`/`is_kernel` 分开。
- **部分落地**：取消通用保存上下文接口、`trap_entry` 补充预保存栈、`TrapWaitQueue` 缓存 trap、`IdleHandler`（类型已删、字段还在）、`into_user` 用户栈管理。
- **未实现且影响接口的**：删除 `Blocking`、`set_state` 只读化、`context_type`、`register_waker`、`resched`/`restore_context` 改名、`dealloc` 清理、事件源改名/职责扩展、`TrapInfo::handle` 收窄、删除 `new_handler`。
- **未实现或待定的设计项**：`trap_handler` 合并、内核/用户 TCB 统一、上下文容器、事件源动态创建、`sources` 活跃标志、VVAR 优先级收敛（Q-038）、`into_user` 用户栈回收。
- **步骤 3（可重入）**：全部未实现，是 0003 计划的最后一步；上面的 Task 接口项是它的前置或后续切片。

## 产生的 Q

已有 Q 承接：

- `Q-001`（已关闭）：`Blocking` 已删除，代码残留由额外步骤清理。
- `Q-002`（已关闭）：事件源删除。
- `Q-003`：Task 接口收窄/改名。
- `Q-004`（已关闭）：不删除 `new_handler`/`IdleHandler`。
- `Q-008`–`Q-014`：可重入判定/上下文/交接/锁；`Q-015` 已关闭（per-CPU 空闲栈池）。
- `Q-020`（已关闭）：保存在各自地址空间，不统一保存。
- `Q-021`：内核服务任务关中断。
- `Q-022`：异步系统调用模块边界。
- `Q-029`/`Q-030`（已关闭）：`trap_handler` 不合并；`TrapInfo` 在 trap 入口入队。
- `Q-031`（已关闭）：不需要 `Task::register_waker`，协程 `poll` 自动注入 waker。
- `Q-032`：`Task::dealloc` 需要做哪些 action/队列/锁清理。
- `Q-033`：`TrapInfo::handle` 收窄后，处理任务通过什么接口接收通信。
- `Q-034`（已关闭）：事件源删除，不再需要动态创建。
- `Q-035`：`Context::into_user` 的用户栈分配/回收接口是否要进入 `Context` trait。
- `Q-036`：空栈进入调度模块是否必须，以及多核同步如何解决（图中标注“理想中可以用非空栈”）。
- `Q-037`（已关闭）：`sources` 删除，活跃标志不再需要。
- `Q-038`：VVAR 只共享最高优先级、进程选择在内核；原问题是是否需要全局最高优先级字段。
- `Q-039`（已关闭）：新 trap 机制保证中断后一定运行 `trap_handler`。
- `Q-040`：`内核态→用户态→中断` 两次特权级切换的性能开销。
- `Q-041`：跨进程唤醒用户任务并携带数据；设计见 `design/cross-process-wake.md`，选型见 `D-002`。
- `Q-042`：同步/异步系统调用是否共用 TrapInfo 发送队列；若共用，队列放 VVAR 的安全/所有权。
- `Q-043`/`Q-045`/`Q-046`（已关闭）：Future 阻塞、共享阻塞队列 + 信号量；`Q-044`（open）：无在线任务时的优先级更新。
