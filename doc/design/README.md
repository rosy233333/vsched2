# 设计索引

> 当前设计目标。`decided` 表示已批准、可以开始实现，不等于代码已实现；现状见 `reference/`（待建）和 `analysis/`。

| 设计 | 状态 | 回答 |
|---|---|---|
| [user-interrupt-context.md](user-interrupt-context.md) | draft | 过渡稿：用户态任务被内核级中断打断后的任务归属、上下文归属和保存/恢复；当前决策 [D-001](../decisions/0001-user-task-interrupt-ownership.md)，D-004（proposed）拟取代 |
| [scheduling-loop.md](scheduling-loop.md) | draft | 调度循环目标路径：内含路径表、新增中断恢复路径和步骤 1 差异 |
| [cross-process-wake.md](cross-process-wake.md) | draft | 跨进程唤醒的问题、VVAR 接收队列方案和未决项；选型见 [D-002](../decisions/0002-cross-process-wake.md) |
| [communication.md](communication.md) | draft | 事件源 → 接收队列的新旧通信方案、接入位置和接收队列汇聚；选型见 [D-003](../decisions/0003-receive-queue-communication.md) |
| [reentrancy.md](reentrancy.md) | draft | 调度模块可重入总览：三个子问题、共同不变量和设计入口 |
| [kernel-shadow-task.md](kernel-shadow-task.md) | draft | 内核影子任务：上下文归属、`krun_utask`、直接恢复/正常调度；核心选择见 [D-004](../decisions/0004-kernel-shadow-task.md)（proposed） |
