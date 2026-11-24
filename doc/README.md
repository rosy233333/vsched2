# 文档地图

> 本文件是 `doc/` 的入口：说明文档放在哪、生命周期是什么、从哪里读。每篇怎么写见 [conventions.md](conventions.md)，改代码时同步哪篇见 [maintenance.md](maintenance.md)；文档落地顺序见 [文档整理计划.md](文档整理计划.md)，重构计划见 [plans/README.md](plans/README.md)。

## 1. 文档地图

### 1.1 生命周期分层与两个横切件

按**生命周期**分三层，导航是入口：

| 层 | 文档 | 生命周期 | 归档触发 |
|---|---|---|---|
| 导航 | 根 `README.md`、`AGENTS.md`、本文件 | 始终对应当前地图 | — |
| 活文档 | `reference/`、`design/`、`decisions/`、`analysis/` | 常驻；reference/design/analysis 原地更新，decisions 只追加不改写 | 被取代、对象删除、整轮迭代结束 |
| 行动 | `plans/` | 与修改计划同生命周期 | 完成 / 放弃 |
| 历史 | `archive/` | 只读 | — |

活文档内部按**角色**分，而不是按时态分：

| 角色 | 回答 | 目录 |
|---|---|---|
| 事实 | 现在是什么，已验证的契约 | `reference/` |
| 规范 | 应该是什么，目标模型 | `design/` |
| 承诺 | 为什么选这个，替代方案与后果 | `decisions/` |
| 证据 | 查到了什么，调查与结论 | `analysis/` |

`decisions/` 是 append-only：不改写旧 ADR 的正文，只更新 `status` / `superseded_by` 并追加新 ADR；它不因 plan 完成而归档。

一句话分工：

> - **reference = 事实**：现在是什么
> - **design = 规范**：应该是什么
> - **decisions = 承诺**：为什么选这个
> - **analysis = 证据**：查到了什么
> - **plan = 行动**：这次怎么改
> - **archive = 历史**：过去怎样，不可作为现状参考

变化写 plan，目标写 design，现状写 reference，选择写 decisions，调查写 analysis，实现留代码。

**plan 完成的标准不是代码合入，而是活文档已经反映变化**：reference 已更新（或标 `stale`）、design/decisions/analysis/Q 已同步、测试通过，然后 plan 才标 `done` 并归档。其他目录不因 plan 完成而归档；它们只按自己的生命周期归档。

两个横切件：`GLOSSARY.md`（术语 ↔ 代码符号）与 `doc/questions.md`（唯一 TODO 台账）。

#### 入口分工

| 文件 | 读者 | 唯一职责 | 不该写 |
|---|---|---|---|
| 根 `README.md` | 第一次来的人 | 项目是什么、为什么、怎么跑 | 文档细节、AI 规则 |
| [AGENTS.md](../AGENTS.md) | AI agent | 怎么工作、先读什么、硬规则 | 项目长介绍、完整文档地图 |
| 本文件 | 文档作者/维护者/AI | 文档地图、生命周期、阅读路径 | 项目介绍、AI 规则 |

#### 该看哪一篇（也是事实所有权）

同一事实只在“唯一来源”里写全文；其他文档最多一句摘要 + 链接。

| 问题/事实 | 去看（也是唯一来源） | 其他文档怎么写 |
|---|---|---|
| 项目是什么、为什么 | 根 `README.md` | 一句话 + 链接 |
| 术语/命名？ | [GLOSSARY.md](../GLOSSARY.md) | 引用术语名，不另造同义词 |
| 当前阶段、下一步 | [文档整理计划.md](文档整理计划.md)（未来 `status.md`） | 一句话 + 链接 |
| 现在是什么样？ | `reference/README.md` → `reference/<topic>.md` | 链接到条目 |
| 应该是什么样？ | `design/README.md` → `design/<topic>.md` | 链接到设计节 |
| 新版设计打算改什么？ | [analysis/new-design-planned-changes.md](analysis/new-design-planned-changes.md) | 引用修改项 |
| 为什么这么决定？ | [decisions/README.md](decisions/README.md) | 引用 `D-xxx` |
| 我们查到了什么？ | `analysis/<topic>/...md` | 链接 |
| 还没定？ | [questions.md](questions.md) | 引用 `Q-xxx` |
| 怎么写/有哪些状态？ | [conventions.md](conventions.md) | 链接类型卡 |
| 改完要更新什么？ | [maintenance.md](maintenance.md) | 链接场景 |
| 下一步做什么？ | [plans/README.md](plans/README.md) | 链接 plan |
| 以前怎么做的？ | `archive/README.md` | 链接历史 |
| 构建/验证 | [编译命令.md](编译命令.md)（未来 `reference/build-and-test.md`） | 一条命令 + 链接 |

### 1.2 现有文档（迁移前，在迁移完成后删除）

下列文档是迁移的起点。`draft` 表示内容仍混合“已实现/目标/待定”，读时以文内标记为准；迁移后以 `design/`、`reference/` 为准。

| 路径 | 类型 | 回答什么 | 状态 | 去向 |
|---|---|---|---|---|
| [编译命令.md](编译命令.md) | reference 素材 | 怎么编译调度模块 | current | 并入 `reference/build-and-test.md` |
| [文档整理计划.md](文档整理计划.md) | plan | 文档落地的阶段与顺序 | draft | 保留在 doc/ 作为文档落地记录；完成后归档 |
| [共享调度器模块设计文档](draft/新版设计/共享调度器模块设计文档.md) | design | 任务模型、事件源、调度路径、可重入方向 | draft | 拆到 `design/*` |
| [共享调度器模块代码实现设计](draft/新版设计/共享调度器模块代码实现设计.md) | design | 模块职责、接口、数据结构、上下文保存约定 | draft | 接口→`reference/interfaces.md`；结构→`reference/module-map.md`；设计→`design/*` |
| [数据结构修改](draft/新版设计/数据结构修改.md) | design 素材 | TCB、Trap、状态字段的修改提案 | draft | 转成接口变更表或 design 的差异表，不单独保留 |
| [调度器可重入设计文档](draft/可重入分析/调度器可重入设计文档.md) | analysis | 可重入问题、共享数据、时点分析 | draft | 目标→`design/reentrancy.md` + `design/kernel-shadow-task.md`；分析→`analysis/reentrancy/` |
| [二维分析.xlsx](draft/可重入分析/二维分析.xlsx) | analysis 素材 | 时间 × 空间矩阵 | draft | 结论抽到 `analysis/reentrancy/matrix.md`；xlsx 作附件 |
| [0.开题版设计/](archive/0.开题版设计/) | archive | 开题阶段思路：Rel4 异步 IPC、协程阻塞唤醒、io_uring 与协程整合 | archive | 只读；结论提炼到 `design/` 或 `decisions/` |
| [1.初版实现/](archive/1.初版实现/) | archive | 第一版实现：调度器设计、重构思路、WFI、trap handler、流程图 | archive | 只读；结论提炼到 `reference/` 或 `design/` |
| [2.清晰调度器边界修改/](archive/2.清晰调度器边界修改/) | archive | 第二轮：清晰边界计划、action/阻塞唤醒、状态变化流程图 | archive | 只读；结论提炼到 `design/blocking.md` 或 `decisions/` |

`doc/draft/新版设计/` 下还有设计图与流程图（任务切换模型、修改后的设计图、调度器复用线程栈问题等），迁移时随对应 design 文档走。

### 1.3 目标结构

下列文件默认是 `planned`，尚未创建；不要把它们当作已存在的链接。创建时按 [conventions.md](conventions.md) 的 front matter 规则加元数据，并在 1.2 登记。

```text
vsched2/
├── README.md                     # existing：项目介绍、快速开始、入口
├── AGENTS.md                     # existing：AI 路由与硬规则
├── GLOSSARY.md                   # existing：术语 ↔ 代码符号
└── doc/
    ├── README.md                 # existing：地图与生命周期
    ├── conventions.md            # existing：类型、状态机、格式、通用约定
    ├── maintenance.md            # existing：按场景的维护规则
    ├── status.md                 # planned：能力 × 设计/实现/验证/差距
    ├── questions.md              # existing：未决问题台账
    ├── questions-closed.md       # 可选：关闭项过多时拆出
    ├── design/
    │   ├── README.md             # existing：设计索引/状态
    │   ├── user-interrupt-context.md  # existing：用户态任务中断上下文
    │   ├── cross-process-wake.md # existing：跨进程唤醒
    │   ├── communication.md      # existing：事件源/接收队列通信方案
    │   ├── task-model.md         # 任务/线程/协程/任务树/上下文类型
    │   ├── os-boundary.md        # OS 与调度模块各自负责什么
    │   ├── scheduler.md          # 事件源/优先级/perCPU/共享
    │   ├── scheduling-loop.md    # existing：调度循环路径（Step 1）
    │   ├── blocking.md           # 阻塞队列/Waker/丢失唤醒
    │   ├── reentrancy.md         # existing：可重入总览
    │   └── kernel-shadow-task.md # existing：内核影子任务
    ├── reference/
    │   ├── README.md             # 覆盖矩阵 + 契约卡模板
    │   ├── module-map.md         # 模块契约卡（约 9 个模块）
    │   ├── interfaces.md         # 提供/依赖/内部接口 + 前置条件
    │   ├── shared-data.md        # VVAR/私有数据/perCPU/两份拷贝
    │   ├── state-machine.md      # TaskState × SchedAction × 上下文
    │   ├── concurrency.md        # 锁序/关中断/栈所有权/不变量
    │   └── build-and-test.md     # 构建矩阵/验证方式
    ├── decisions/
    │   ├── README.md             # existing：决策索引
    │   ├── 0001-user-task-interrupt-ownership.md  # existing：D-001
    │   ├── 0002-cross-process-wake.md           # existing：D-002
    │   ├── 0003-receive-queue-communication.md  # existing：D-003
    │   └── 0004-kernel-shadow-task.md           # existing：D-004
    ├── plans/
    │   ├── README.md             # existing：计划索引
    │   └── 0003-scheduler-refactor/      # existing：README + step-*
    ├── analysis/
    │   ├── new-design-planned-changes.md  # existing：新版设计 vs 实现
    │   └── reentrancy/           # planned：判定标准/矩阵/结论
    ├── assets/                   # 跨文档共享的图
    ├── draft/                    # existing：迁移中，内容逐步搬到 design/reference
    └── archive/                  # existing：历史，只读
        ├── README.md             # planned：每轮结论/仍有效/被谁取代
        ├── 0.开题版设计/
        ├── 1.初版实现/
        └── 2.清晰调度器边界修改/
```

### 1.4 阅读路径

#### 现在（迁移前，在迁移完成后删除）

1. **快速了解**：先读 [共享调度器模块设计文档](draft/新版设计/共享调度器模块设计文档.md) 的“任务与任务树”“调度路径”“可重入”三节。
2. **看实现约束**：再读 [共享调度器模块代码实现设计](draft/新版设计/共享调度器模块代码实现设计.md) 的“结构与功能”“接口”“数据结构”。
3. **可重入**：[调度器可重入设计文档](draft/可重入分析/调度器可重入设计文档.md) + [二维分析.xlsx](draft/可重入分析/二维分析.xlsx)。
4. **历史**：按 [0.开题版设计/](archive/0.开题版设计/) → [1.初版实现/](archive/1.初版实现/) → [2.清晰调度器边界修改/](archive/2.清晰调度器边界修改/) 的顺序读。
5. **编译**：[编译命令.md](编译命令.md)。

> 这些 draft 文档混合了“已实现/目标/待定”，读时注意文内标记；迁移后以 `design/`、`reference/` 为准。

#### 目标（文档建好后）

| 目标 | 路径 |
|---|---|
| 5 分钟了解 | 根 `README.md` → `doc/README.md` → `design/README.md` |
| 接手实现 | `design/scheduling-loop.md` → `reference/interfaces.md` → `reference/concurrency.md` → `plans/0003-scheduler-refactor/README.md` |
| 审查正确性 | `reference/state-machine.md` → `reference/concurrency.md` → `questions.md` |
| 写文档 | `conventions.md` |
| 维护文档 | `maintenance.md` |
| 考古 | `archive/README.md` |

### 1.5 代码 → 文档归属

| 代码 | 必须同步的文档 |
|---|---|
| `src/interface.rs` | `reference/interfaces.md`、`design/os-boundary.md` |
| `src/current.rs` | `reference/shared-data.md` |
| `src/schedule/**` | `reference/module-map.md`、`reference/state-machine.md`、`reference/concurrency.md`、`design/scheduler.md` |
| `src/main_loop.rs` | `design/scheduling-loop.md`、`reference/interfaces.md` |
| `src/stack/**` | `reference/module-map.md`、`reference/concurrency.md`、`design/scheduling-loop.md` |
| `src/block_and_wake/**` | `design/blocking.md`、`reference/concurrency.md` |
| `src/arch/**` | `reference/module-map.md`、`reference/build-and-test.md` |
| `src/api.rs` | `reference/interfaces.md` |
| `build.rs`、`Cargo.toml` | `reference/build-and-test.md` |

文档建好后生效；未建时先更新对应 draft 或登记 Q。

## 2. 约定与维护

- [conventions.md](conventions.md)：各文档类型的格式、状态机、必填栏目、front matter、ID、命名与链接规则。
- [maintenance.md](maintenance.md)：按场景说明要检查/更新哪些文档，以及过期、取代与归档规则。
