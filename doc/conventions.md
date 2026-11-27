# 文档约定

> 本文件按文档类型组织：每种类型的定位、状态机、格式、必填内容、front matter 和约定。地图与生命周期见 [README.md](README.md)；按场景的维护规则见 [maintenance.md](maintenance.md)。

## 1. 类型索引

| 类型 | 回答什么 | 目录 |
|---|---|---|
| design | 应该是什么，目标模型 | `design/` |
| reference | 现在是什么，已验证的契约 | `reference/` |
| analysis | 查到了什么，调查与结论 | `analysis/` |
| decision | 为什么选这个，替代方案与后果 | `decisions/` |
| plan | 这次怎么改，步骤与验证 | `plans/` |
| question | 还没定什么，台账与链接 | `questions.md` |

`archive/` 不是文档类型，也不是 `status` 值，是历史位置，见第 9 节。

> 状态机里的 `→ archive` 是归档动作，不是 `status` 值；状态只取各卡片列出的值。

## 2. design

**回答**：应该是什么。**不写**：逐步实现细节、当前实现。
**目录**：`design/`。

**状态机**：

```text
状态：draft → decided → implemented
      draft/decided/implemented → superseded
归档：superseded → archive/
```

- `draft`：目标未定，可改。
- `decided`：目标已批准，可以开始实现。
- `implemented`：代码已反映目标；仍是“应该是什么”的权威。
- `superseded`：被新设计取代；迭代结束后进 `archive/`。

**必填栏目**：

- 问题/目标/非目标（非目标同时写“本文不回答什么”）
- 模型
- 规则与不变量（`I-xx`）
- 路径/时序
- 接口影响（旧→新→理由）
- 未决 Q（`Q-xx`）
- 验证

**Front matter**：`status` 必填；`applies_to`、`supersedes` / `superseded_by` 可选。字段定义见 8.1。

**约定**：

- 写“应该”，不写“现在”；现状放“现状”小节，并链接 `reference/`。
- 每个不变量给 `I-xx`；接口影响写清旧→新→理由。
- 未决问题用 `Q-xx`，不写裸 TODO。
- 目标变了就改 design；不要用 plan 代替 design。

**相关场景**：[改代码/接口](maintenance.md#2-改代码接口)、[过期/取代/归档](maintenance.md#6-发现文档过期被取代)。

## 3. reference

**回答**：现在是什么。**不写**：应该是什么、算法流程。
**目录**：`reference/`。

**状态机**：

```text
状态：current ↔ stale
      current/stale → superseded
归档：superseded → archive/
```

- `current`：已对照当前代码验证。
- `stale`：代码变了但没重新验证；留在原目录，加 STALE 提示，安排重验。
- `superseded`：被新 reference 取代；更新引用后进 `archive/`。

**必填栏目**：

- 范围
- 事实表
- 前置/后置条件
- 易错点
- 代码锚点
- `verified_at`

**Front matter**：`status`、`depth`、`verified_at` 必填；`applies_to` 可选。字段定义见 8.1。

**`depth`**：

| depth | 回答 | 写什么 | 不写什么 |
|---|---|---|---|
| `card` | 这个模块是什么、边界在哪？ | 职责、公开接口、依赖、所有权、关键不变量、易错点 | 算法流程、逐行实现 |
| `contract` | 改这里必须遵守什么？ | 前置/后置、状态转移、锁/IRQ、共享数据、失败模式 | 逐行代码翻译 |
| `deep` | 为什么这里容易错、怎么证明？ | unsafe/汇编约定、并发协议、栈/任务生命周期、场景走查、不变量 | 与契约无关的历史 |

使用规则：

- 全仓先有 `card`；只在改代码需要时升级到 `contract` / `deep`。
- `deep` 只给高风险区：unsafe/汇编、跨特权级协议、并发/锁、栈/任务生命周期、可重入、反复出 bug 的地方。
- `depth` 与 `status` 正交：`current` 说已验证，`card` 说只写到模块级。

**约定**：

- 只写“现在”，不写“应该”。
- 每条事实给代码锚点；前置/后置写清楚。
- `verified_at` 是对照代码验证的 commit，不是最后编辑日期。
- `stale` 必须带动作：重验、取代或删除；只标不办会变成 sediment。
- 覆盖矩阵在 `reference/README.md` 维护；这里只定义词义。

**相关场景**：[改代码](maintenance.md#2-改代码接口)、[发现过期](maintenance.md#6-发现文档过期被取代)。

## 4. analysis

**回答**：查到了什么。**不写**：规范设计、最终决策。
**目录**：`analysis/`。

**状态机**：

```text
状态：draft → concluded
      draft/concluded → superseded
归档：superseded → archive/
```

- `draft`：调查进行中。
- `concluded`：结论已提取到 decision/design/reference；留在 `analysis/` 作证据。
- `superseded`：被新分析取代；进 `archive/`。

**必填栏目**：

- 问题
- 方法
- 数据
- 结论
- 产生的 Q

**Front matter**：`status` 必填；`applies_to` 可选。字段定义见 8.1。

**约定**：

- 可以长，可以有数据/图/矩阵；但结论必须明确。
- 不写规范设计；结论提升到 `design/`、`decisions/` 或 `reference/`。
- 结束时标 `concluded`；产生的 Q 进 `questions.md`。
- 不复制最终设计；analysis 是证据，不是第二份 design。

**相关场景**：[开始一次代码改动](maintenance.md#1-开始一次代码改动)、[关闭 Q](maintenance.md#4-关闭-q)、[归档迭代](maintenance.md#8-归档一轮迭代)。

## 5. decision

**回答**：为什么选这个。**不写**：实现步骤、完整设计。
**目录**：`decisions/`。

**状态机**：

```text
状态：proposed → accepted → superseded
归档：不归档；留在 decisions/
```

- `proposed`：待评审。
- `accepted`：已生效，约束后续设计/计划。
- `superseded`：被新 ADR 取代；保留旧 ADR，更新 `status` / `superseded_by`，不归档。

**必填栏目**：

- 背景
- 决策
- 替代方案
- 后果
- 相关 Q/设计

**Front matter**：`status` 必填；`supersedes` / `superseded_by` 可选。字段定义见 8.1。

**约定**：

- 只收难回退、反直觉、真有权衡的决定；日常小决定留 `design/`。
- 一篇一个决定；append-only：不改写旧 ADR 的正文，只更新 `status` / `superseded_by` 并追加新 ADR。
- 不写实现步骤；plan 负责步骤。

**相关场景**：[做一个决定](maintenance.md#3-做一个决定)、[被新 ADR 取代](maintenance.md#6-发现文档过期被取代)。

## 6. plan

**回答**：这次怎么改。**不写**：完整论证、现状手册。
**目录**：`plans/`。

**状态机**：

```text
状态：draft → active ↔ blocked
      active/blocked → done
      draft/active/blocked → dropped
归档：done/dropped → archive/
```

- `draft`：未就绪，不能执行。
- `active`：执行中。
- `blocked`：等待 Q/依赖。
- `done`：活文档已反映变化、测试通过；归档。
- `dropped`：放弃；先回到稳定状态（可编译/可测试、接口不半迁移、reference 一致或标 `stale`），再归档并注明未反映/未回滚的内容。

**变更入口页**：`plans/README.md` 是计划索引；`plans/<id>/README.md` 是单次变更的入口页，至少包含：

- 背景与目标/前置假设：为什么改、改成什么、不做什么、哪些条件变了计划要改。
- 方案概述：为什么这样做、关键约束、备选方案与取舍。
- 相关文档：design/decision/analysis/Q 的链接；统一 topic slug。
- 步骤表：步骤、产出文档、测试、完成标准、依赖。
- 接口变更：旧→新、理由、受影响的 OS 代码。
- 风险与验证：风险、回滚、怎么验证。

**Front matter**：`status` 必填；`applies_to` 可选。字段定义见 8.1。

**约定**：

- plan 是 diff，不是 snapshot；不重复 design/reference 的完整内容。
- 单一事实源：plan 写摘要 + 链接，不复制完整论证。
- 小改动例外：改动小、可逆、无接口/架构影响时，背景/方案/取舍可内联在 plan，不单独建 design/decision/analysis；有接口/架构变化、难回退或真正的替代方案时，才拆出去。
- 完成标准不是代码合入，而是活文档已更新（见 [maintenance.md](maintenance.md#5-完成放弃-plan)）。

**相关场景**：[开始一次代码改动](maintenance.md#1-开始一次代码改动)、[完成/放弃 plan](maintenance.md#5-完成放弃-plan)。

## 7. question

**回答**：还没定什么。**不写**：长篇分析。
**位置**：`questions.md`。

**状态机**：

```text
状态：open ↔ leaning
      open/leaning → decided/dropped
归档：不归档；留在 questions.md
```

- `open`：未决，没有倾向。
- `leaning`：有倾向，未拍板。
- `decided`：已决定；`详见` 指向 decision/design。
- `dropped`：考虑过并放弃；保留，记录负结论。

**行格式**：

```text
| ID | 优先级 | 问题 | 类型 | 影响 | 现状/倾向 | 阻塞谁 | 详见 | 状态 |
```

**优先级与排序**：当前切片是当前正在设计/实现、可独立计划与验证的改动单元。`P0` 不解决就无法写当前切片的 design/plan；`P1` 当前切片可绕过、但下一片前必须解决；`P2` 重构完成前；`P3` 清理/以后。`## Open`（open/leaning）按主题分节，每个主题内先 `P0 → P3`、同优先级按 `Q-ID` 升序；`## Closed`（decided/dropped）是单表，不按主题分节，整表先 `P0 → P3`、再按 `Q-ID` 升序；顶部放 P0 速查。

**Front matter**：`questions.md` 是台账，不用 front matter；状态写在行内。

**约定**：

- `questions.md` 是索引，不是分析文档；详细分析放主题所在处。
- `详见` 必须指向具体标题/锚点；没有分析时写“待分析”。
- 分析超过 5 行就搬出去；台账保持可扫读。
- 一个问题一个 `Q-ID`；一个分析文档可以承载多个 Q。
- Q 关闭时标 `decided`，`详见` 改指 ADR/design；**不要删行**。关闭项是引用锚点，也会丢掉 `dropped` 的负结论。
- 关闭项多了就分节：`## Open` 在上、`## Closed` 在下；超过约 300 行或 100 条时，把较早的关闭项移到 `questions-closed.md`，保留列和 ID，不重新编号。

**相关场景**：[关闭 Q](maintenance.md#4-关闭-q)、[新增文档](maintenance.md#7-新增移动删除文档)。

## 8. 通用约定

### 8.1 Front matter

设计、事实、分析、计划、决策文档在文首加：

```yaml
---
status: current
depth: contract
verified_at: <commit>
applies_to:
  - src/schedule
  - src/current.rs
supersedes: []
---
```

- `status`：必填；取值见各类型卡片的状态机。
- `depth`：仅 reference；`card` / `contract` / `deep`，见第 3 节。
- `verified_at`：仅 reference；验证时的 commit。
- `applies_to`：可选；YAML 列表，代码路径或 glob。
- `supersedes` / `superseded_by`：可选；YAML 列表，`D-xxx` 或文档路径。

约定：

- 键和值统一英文；状态/深度用各类型卡片的闭集，不写同义词，不混中文。
- 只写元数据，不写正文摘要、不写变更日志、不写多句说明。
- 正文与 front matter 不一致时，以 front matter 为准，并在同一个 commit 修正。
- `verified_at` 是对照代码验证时的 commit，不是最后编辑日期；代码变了但没重新验证，标 `stale`，不要改日期蒙混。
- 一篇文档只写一个主 `depth`；需要“概览 + 深挖”时拆成两篇并互相链接。
- 导航层（README、AGENTS）、约定/维护总表（本文件、maintenance.md）、`questions.md`、`archive/` 不加 front matter。
- 过期文档的处理见 [maintenance.md](maintenance.md#6-发现文档过期被取代)。

### 8.2 稳定 ID

| 前缀 | 用途 | 权威位置 |
|---|---|---|
| `Q-001` | 未决问题 | `questions.md`（详细分析在主题文档，台账只放一行 + `详见` 链接） |
| `D-001` | 决定/ADR | `decisions/` |
| `I-001` | 不变量 | `concurrency.md` 或 design 的“规则与不变量” |
| `PATH-01` | 调度路径 | `design/scheduling-loop.md` |

规则：只增不改号；关闭后保留；提交信息和代码注释可以引用；不要给每个标题编号。

### 8.3 文件、链接与图

- 新文件用 ASCII kebab-case，H1 用中文；`archive/` 保留原名。
- 一个 H1；H2 是检索单元；单篇 150–300 行，导航/约定总表可到 360 行，超过 400 行按分支/序列拆。
- 图放在文档同目录，跨文档共享放 `doc/assets/`；保留 `.drawio` 源和 `.png` 导出；图旁写“读图说明”。
- 链接用仓库相对路径；禁止 `../26.x` 和“必读”外部链接；外部链接只作出处。迁移时清理现有的 `../26.x`、`assets/` 旧路径。
- 含空格的文件路径用 `<...>` 或 `%20`；标题锚点用 slug（空格→`-`、去标点），不要写原始标题。
- 不复制大段代码；给 `src/...:line`。
- 迁移前的 draft 可以有旧 TODO；迁移时逐条转 `Q-xxx`。新文档不允许裸 TODO。

### 8.4 语言与术语

- 正文中文；类型、字段、函数用代码原名，包括拼写（如 `hightest_priority`、`EVENT_SORCE_NUM`）。
- 术语首次出现用 `GLOSSARY.md` 的规范名；不要混用同义词。
- 整体调度用“调度模块”；`Scheduler` 类/实例用“调度器”；选择任务/抢占判断用“调度决策”。
- 一段只说一件事；枚举用表，不用嵌套 bullet。

### 8.5 禁止项

- 把“目标设计”写成“现状”。
- 为即将删除的代码写完整契约。
- 用空文件、占位步骤填目录。
- 把 `archive/` 当现状引用。
- 在 `reference/` 里写算法流程。

### 8.6 目录 README

目录 README 是该目录的索引（一句话职责 + 表），不是第二份文档；不加 front matter。至少包含：

- 一张表：文档 | 回答什么 | 状态 | 备注。
- `reference/` 额外列 `depth` / `verified_at`。

按目录：

| 目录 | 表格内容 | 归档后 |
|---|---|---|
| `reference/` | 文档、depth、status、verified_at | `stale` 保留；`superseded` 移出表格 |
| `design/` | 文档、status、相关 plan | `superseded` 移出表格 |
| `analysis/` | 文档、status、结论链接 | `concluded` 保留；`superseded` 移出 |
| `plans/` | 活跃/阻塞计划、入口页 | `done`/`dropped` 移出，由 `archive/README.md` 记录 |
| `decisions/` | ADR、status、superseded_by | `superseded` 保留，不归档 |
| `archive/` | 迭代目录，不列单篇文档 | — |

- 活跃目录 README 只索引“当前目录内容”；文件移入 `archive/` 后，从表格移除，不留指向旧路径的行。
- 需要保留引用时，在表格下方单独放“已归档”小节，链接到 `archive/`；不要混在正常表格里。
- 完整历史由 `archive/README.md` 承担；它按迭代记录，不列单篇文档。

## 9. archive（历史位置）

`archive/` 不是文档类型，也不是 `status` 值；它是只读历史位置：

- 每轮迭代一个子目录；`archive/README.md` 记录目标、结论、仍有效、被谁取代。
- 进入 archive 前先把结论提炼到 `reference/`、`design/`、`decisions/`。
- archive 文档不加 front matter；不参与状态词表。
- 不把 archive 当现状引用；需要历史背景时从这里开始。
