---
status: draft
applies_to:
  - src/stack/handler.rs
  - src/block_and_wake/block_queue.rs
  - src/schedule/ready_queue.rs
  - src/schedule/trap_wait_queue.rs
  - src/interface.rs
---

# 调度相关锁盘点与无锁化分析

## 问题

调度器代码可能在持锁时被中断。需要先盘点当前代码中的所有调度相关锁，判断哪些可以直接替换为无锁结构，哪些还需要专门设计。

本文考虑**所有锁**，不只考虑跨特权级访问的锁：跨特权级锁可能导致死锁，而任何锁在中断时被持有都会影响性能。

## 方法

- 静态阅读当前实现，列出锁、保护的数据和访问方式。
- 按“锁保护的数据是否只有一条队列”分类：
  - 只保护一条队列：可直接换无锁队列；
  - 保护多个相关字段/计数：需要额外设计。
- 对照 [方案2 任务/状态变化流程图](../../archive/2.清晰调度器边界修改/任务及其状态变化流程图（新版，方案2）.drawio) 中的阻塞/唤醒流程，检查临界区边界。

## 数据

### 当前锁

| 数据 | 锁 | 内容 | 代码锚点 |
|---|---|---|---|
| `StackHandler` | `SpinMutex<StackHandler>` | 空栈（`free_stacks`，当前按 CPU 分组的 `FnvIndexMap`）、当前栈（`current_stack`）、trap 暂存栈（`trap_stack`） | [handler.rs:92](../../../src/stack/handler.rs#L92-L100)、[current.rs:93](../../../src/current.rs#L93)、[current.rs:126](../../../src/current.rs#L126) |
| `BlockQueue` | `Mutex<BlockQueueInner>` | 通知计数 `notifications` 与阻塞任务队列 `tasks` | [block_queue.rs:136](../../../src/block_and_wake/block_queue.rs#L136-L145) |
| `ReadyQueue` | 每个优先级一个 `Mutex<Deque>` | 就绪任务队列；`prio_bitmap` 已是原子变量 | [ready_queue.rs:28](../../../src/schedule/ready_queue.rs#L28-L32) |
| `TrapWaitQueue` | 每 CPU 一个 `Mutex<TrapQueue>` | 每 CPU 的 trap 队列 | [trap_wait_queue.rs:43](../../../src/schedule/trap_wait_queue.rs#L43) |
| 任务状态锁 | `interface` 约定，无具体类型 | `TaskState` 与 `SchedAction` | [interface.rs:21](../../../src/interface.rs#L21-L26) |

当前版本还有事件源的锁，但事件源后续会删除，因此未计入。

### per-CPU 派生状态的写回

per-CPU 数据中有一类“派生状态写回”：读取队列/事件源的当前状态，计算新的优先级，再用普通 `store` 写回 `highest_prio[cpu]`。

同步的单位不是代码中的原子变量，而是逻辑上需要保持一致的数据集合。本节只确认 `highest_prio` 这个集合；其它集合的边界待后续分析。

需要区分：

- `highest_prio[cpu]` 自身只有一次 `store`，不是对同一个原子变量的读取-修改-写回，也没有 `compare_exchange`；
- 风险不是“同一原子变量的 RMW 丢失”，而是写入的是基于其它状态算出的派生值，可能已经过期。

如果只有一个写者、其它模式只读，则不存在后写者覆盖前写者，只可能读到旧值。若有多个写者，后写者可能基于旧状态覆盖新值；这属于派生状态的一致性/多写者问题，不是单个原子变量本身的 RMW 问题。

其它 per-CPU 变量（如当前任务、当前地址空间、休眠标志）多为单次 `load`/`store`/`swap`，不属于这类派生写回；但它们仍不能解决持锁跨 trap、旧 `cpu_id` 缓存等重入问题。

在 per-CPU K 候选方案下，同核用户-用户交错可以被消除；同一核上 `highest_prio` 只有一个用户写者，内核只读，因此不存在同核覆盖，只可能读到旧值。旧值表示该任务的优先级还未完全发布，属于正常情况。

因此，对 `highest_prio` 这个逻辑数据集合没有同步影响，可以保留当前的派生写回。其它多字段逻辑集合仍需按同一单位单独分析；K 只消除同核用户-用户交错，不消除内核或其它核心对同一集合的访问。

### 无锁化分析

#### 只保护一条队列的锁

`ReadyQueue` 的每优先级队列、`TrapWaitQueue` 的每 CPU 队列只保护一条队列，可以直接替换为无锁队列。替换时仍需保留：

- 优先级位图/最高优先级的原子更新；
- 队列满、空和并发出队/入队的语义；
- 与任务状态/唤醒路径的交接顺序。

#### `StackHandler`

`StackHandler` 的锁保护三个字段，且不同方法的访问范围不同：

- 初始化时访问并初始化 `current_stack`；
- `alloc_stack` / `dealloc_stack` 只访问 `free_stacks`（当前是 `FnvIndexMap`，可以改成队列）；
- `set_current_stack` / `take_current_stack` 只访问 `current_stack`；
- `set_trap_stack` 只访问 `trap_stack`；
- `alloc_trap_stack` 访问了 `free_stack` 和 `trap_stack`，但该函数只会在内核态初始化时调用，用户态不会调用。
- `get_empty_stack` / `get_thread_stack` 访问了 `current_stack` 和 `free_stacks`，因此不能简单按“单队列”替换，需要额外设计。

##### `get_empty_stack` / `get_thread_stack`被中断时的同步问题分析

首先，内核与用户态使用两份不同的`STACK_HANDLER`变量。因此内核态的中断不会影响用户态的`STACK_HANDLER`中的数据。如果使用了“per-CPU K 停放”方案，则不会出现同核用户-用户同步问题（见[相应design文档分析](../../design/kernel-shadow-task.md#4-同核用户-用户同步)）。此时，可以将几个字段分别实现为原子变量与无锁队列，使用多步更新。

实际使用的栈：在trap入口中切换为内核栈，同时通过上下文中保存的sp保存用户栈；任务恢复时，再恢复为保存的sp。因为中断上下文保存了完整的寄存器与栈，而栈切换过程改变的是寄存器，所以即使用户调度模块在切换栈过程中被中断，也能正确恢复。

#### `BlockQueue`

`BlockQueue` 的锁同时保护通知计数和任务阻塞队列。按照方案2流程图：

- 唤醒侧“改变信号量”和“唤醒任务”两步可以分开；
- 阻塞侧“改变队列信号量”和“阻塞任务”两步必须在同一个临界区，否则可能丢失唤醒。

因此不能只把任务队列换成无锁队列，还需要设计通知计数与入队/阻塞的原子交接。

#### 任务状态锁

`interface` 约定的任务状态锁保护 `TaskState` 与 `SchedAction`。方案2流程图中借助该锁进行同步；该锁可能被多个核心访问（另一个任务可能拿到阻塞任务并将其唤醒），因此需要额外的无锁化设计，不能直接用单队列无锁结构替代。

## 结论

- **可直接无锁化**：只保护一条队列的 `ReadyQueue` 队列、`TrapWaitQueue` 队列。
- **需要额外设计**：`StackHandler`（多字段、`get_empty_stack`/`get_thread_stack` 跨字段访问）、`BlockQueue`（通知计数与阻塞入队同一临界区）、任务状态锁（跨核任务状态/action 同步）。
- 本文不选型；具体无锁算法、容量、内存回收和接口影响留给后续设计。

## 产生的 Q

本次分析不新增 Q；相关既有 Q：

- Q-006：锁/IRQ 时机与锁序。
- Q-009：上下文栈结构、同步、容量。
- Q-011：重复入队/重复唤醒的交接协议。
- Q-012：用户态持锁被中断、内核取同一把锁的死锁。
- Q-014：是否保留“内核直接运行用户任务”路径。
- Q-036：多核栈同步。
- Q-050：per-CPU 变量与 CPU 迁移一致性。
