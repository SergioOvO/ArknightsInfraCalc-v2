# Trust Recovery Plan: Feedback, Repro, Trace, Regression

> 状态：active
> 启动日期：2026-07-03
> 替代范围：暂停 `QUALITY_90_TO_95_PLAN.md` 的大架构推进，先恢复结果可信度与排错速度。

## 0. 判断

当前主要风险不是推荐质量从 90 分到 95 分，而是：

> 结果错时，很难快速判断错在数据、机制、搜索、编排、排班、导出还是展示。

因此当前主线从“继续做候选架构”切换为“生产反馈闭环”。目标是让每个线上反馈都能被追踪、复现、定位、修复和回归保护。

## 1. 非目标

本阶段不做：

- 不引入统一 `TeamCandidate` / `TeamColumn` 主架构；
- 不推进全局 selector、SystemRule registry、materialized candidate view；
- 不为了单个反馈泛化体系编排；
- 不自动把中文反馈或机制原文转成规则；
- 不追求全仓库重构或零 warning；
- 不扩大非目标：心情排班、宿管恢复、全基建连续时间最优化仍不做。

`QUALITY_90_TO_95_PLAN.md` 中已有的 trace-only pilot、机制审计和反馈材料可以作为证据使用，但不再作为默认建设路线。

## 2. 当前资产

| 资产 | 用途 |
|------|------|
| [../../feedback/](../../feedback/) | 生产反馈原始证据 |
| [../../feedback/TRACKING.md](../../feedback/TRACKING.md) | 反馈 ledger，记录状态、疑似层、下一步 |
| [../MAINTENANCE_MODE.md](../MAINTENANCE_MODE.md) | 单个 bug 的复现、定位、修复流程 |
| [../PROJECT_MAP.md](../PROJECT_MAP.md) | 代码边界与入口 |

## 3. 工作流

每个反馈按同一条闭环处理：

1. `intake`：从 `issue.json` / `meta.json` / `debug-bundle.json` 记录反馈 id、房间、用户期望和线上命令。
2. `reproduced`：用本地 CLI 复现，优先复用 debug bundle 中的 layout 与 operbox。
3. `localized`：确认错层，只允许落在一个主层：CLI / layout / schedule / search / solver / mechanism / data / output。
4. `fixing`：只改确认错层，避免顺手重构。
5. `regressed`：补最小回归，优先 fixture / CSV / targeted test / debug bundle smoke。
6. `closed`：在 `feedback/TRACKING.md` 写清 commit、验证命令和确认层。

不能复现时标记 `blocked`，写明已跑命令和缺失材料，不猜公式。

## 4. Trace v0 原则

Trace v0 不是完整解释系统，只补关键边界的可观测点。

优先加这些小 trace：

| 层 | 需要看见什么 |
|----|--------------|
| layout/orchestrate | 哪个体系认领了哪些干员；谁因占用或缺条件被拒 |
| assign/fill | 房间为什么空置；候选池数量；最终 fallback 来源 |
| search | top candidate、被用户期望组合的 raw score、是否缺人 |
| control/global | 注入来源、同类效果是否取最高、重复项来源 |
| schedule/shift_bind | 固定组是否换站；换站原因 |
| output/serve | 前端看到的字段是否来自 core，还是展示层加工 |

Trace 默认应是 debug-only，不能改变推荐结果。

## 5. 优先级

先处理会破坏导出或结构完整性的反馈：

| Priority | Examples |
|----------|----------|
| P0 | 发电站数量不足导致 MAA/profile 不生成；会客室/办公室空置 |
| P1 | 制造/贸易/中枢/发电明确推荐错误 |
| P2 | 宿舍 filler、解释、展示质量 |

制造站体系建议很多，但不要从制造站开始重构。先复现 2 到 3 个反馈，确认它们到底是 raw score 错、候选缺失、占用冲突、room level 误判，还是体系偏好缺失。

## 6. 验收

本阶段完成的标志不是“推荐变聪明”，而是：

- 每条生产反馈都在 `feedback/TRACKING.md` 有状态；
- P0 反馈都有本地复现或明确 blocked 原因；
- 每修一个 bug 都有最小回归；
- 后续 agent 不需要全仓库通读，就能从 feedback id 进入对应层；
- `QUALITY_90_TO_95_PLAN.md` 不再被默认当作下一步实现路线。
