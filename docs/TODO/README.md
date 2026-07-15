# TODO 任务生命周期

> 状态：Current
> 本文负责：TODO 的创建、恢复、实施同步、关闭和归档
> 边界：TODO 记录待办和实施交接，不裁决领域语义，也不代表 Agent 可自动扩大当前任务

本目录同时容纳待确认提案、用户已授权但尚未实施的任务，以及需要跨会话追踪的问题。目录中的文件不是默认工作队列；只有用户当前指令明确选择某项任务时，Agent 才获得实施授权。

## 文件命名

- 新增面向人类阅读的 Markdown 优先使用简短、可搜索的中文文件名，例如 `近期已知缺口修复清单.md`。
- 协议固定名、工具读取的固定路径、代码生成物和外部兼容文件可以保留英文。
- 不为了统一外观批量重命名现有文件；只有文件进入本轮任务且全部引用可以安全更新时，才单独评估改名。

## 状态模型

每份 TODO 在开头至少声明状态、目标、真源和非目标。状态只使用以下值：

| 状态 | 含义 | Agent 行为 |
|---|---|---|
| `proposal` | 尚未完成语义或方案裁决 | 只读梳理，不能直接实施 |
| `ready-on-request` | 边界已足够清楚，等待用户明确恢复 | 不自动执行 |
| `in-progress` | 用户已授权，当前正在实施 | 持续同步范围、裁决和证据 |
| `blocked` | 缺少用户裁决、外部事实或前置条件 | 记录阻塞项并停止相关写入 |
| `completed` | 验收已完成，等待或正在归档 | 更新 current 文档后移入 `ARCHIVE/done/` |
| `superseded` | 已被新方案替代或明确不采用 | 写明替代项后移入 `ARCHIVE/superseded/` |

## Agent 编排流程

### 1. 创建

只有任务需要跨会话追踪、包含多个独立交付单元，或用户明确要求 TODO 时才新建文件。创建时：

1. 从 current canonical 文档链接到已确认事实，不在 TODO 复制第二份业务规则。
2. 区分已复现错误、文档明确记录的实现缺口、算法保证边界和待验证假设。
3. 写明成功标准、非目标、依赖关系和建议拆分；一个实施单元只保留一个主要意图。
4. 在本 README 的“当前状态”表登记文件、状态和用途。

### 2. 恢复

用户明确恢复某项 TODO 后，Agent 必须先将它与当前用户裁决、canonical 文档、代码和测试对账。旧 TODO 是历史提案，不得覆盖更新后的 current facts。对账完成后：

1. 将状态改为 `in-progress`，记录恢复日期和本轮范围。
2. 将不再成立的内容标为删除、改写或 `superseded`，不能静默照搬旧计划。
3. 按任务意图加载 maintenance、feature 或 quality Skill；体系和证据协议按触发条件附加。
4. 若 TODO 含多个可独立验证的 bug 或质量项，逐项实施和提交，不用一个大改同时关闭整张清单。

### 3. 实施同步

实施期间 TODO 必须保持可交接：

- 已完成项及时勾选，并链接 commit、证据报告或 current 文档；不能只在对话里宣称完成。
- 新的用户裁决先同步到 canonical 文档，TODO 只记录裁决结果和链接。
- 范围变化、阻塞、失败反例和 deferred finding 写入对应条目。
- 某条风险经调查不是 bug 时，记录结论和证据，不为“完成 TODO”强行修改代码。

### 4. 关闭与归档

最后一个实施项完成不等于可以直接归档。Agent 应自动完成以下收尾，不等待用户再次提醒：

1. 通过风险匹配的真实入口和回归证明验收，并保留 evidence。
2. 更新受影响的 current canonical、实现状态、限制说明和 `docs/INDEX.md` 链接。
3. 将未完成但仍有效的内容拆到新的中文名 TODO；不要把开放项一起归档为完成。
4. 将本 README 状态表中的活动条目删除或改为归档链接。
5. `completed` 移入 `docs/ARCHIVE/done/`；`superseded` 移入 `docs/ARCHIVE/superseded/`，并检查仓库内引用。
6. 最终回复声明归档路径、未完成项、文档影响和证据；归档文档不再作为 current 真源。

## 默认行为

默认行为：

1. 用户报告 bug、结果不对、回归失败时，按根 `AGENTS.md` 路由到 maintenance Skill。
2. 不主动继续本目录里的历史 Phase 计划。
3. 不用“顺手完成 TODO”解释与当前 bug 无关的大改。
4. 只有用户明确说“继续某个 TODO / 继续 Phase / 做历史计划”时，才读取对应文件并恢复功能建设模式。

## 当前状态

本目录没有默认主动工作队列。新问题按根 `AGENTS.md` 判断任务意图；已关闭反馈从 [../../feedback/TRACKING.md](../../feedback/TRACKING.md) 查证。

工作流优化实施计划已完成 A-E 批次，归档于 [../ARCHIVE/done/AGENT_WORKFLOW_OPTIMIZATION_PLAN.md](../ARCHIVE/done/AGENT_WORKFLOW_OPTIMIZATION_PLAN.md)；当前执行入口是根 `AGENTS.md`、维护/质量文档、项目 Skills 和 `scripts/codex/`。

| 文件 | 状态 | 用途 |
|------|------|------|
| [近期已知缺口修复清单.md](近期已知缺口修复清单.md) | ready-on-request | 汇总动态 producer、自动化第三人、迷迭香低心情、制造联合最优性、Bake 安全和 canonical 状态冲突；逐项授权、逐项验证 |
| [DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md) | ready-on-request | 用户已确认的下一位 Agent 交接：三类可选中枢 producer 的 live 精确索引 Join 与 A+ Bake；尚未实现，不自动执行 |
| [CONDITIONAL_ROOM_RESPONSE_BAKE_PLAN.md](CONDITIONAL_ROOM_RESPONSE_BAKE_PLAN.md) | ready-on-request | 动态 producer 的条件化单房响应 Bake：允许离线计算数十分钟，按中枢效果签名物化完整单房 solver response，运行时做互斥 Join 与 live 对账 |
| [TRAINING_RECOMMENDER_RAG_PLAN.md](TRAINING_RECOMMENDER_RAG_PLAN.md) | proposal | 练度比对 / 练卡推荐 / RAG 解释层企划，待 Claude 与用户确认 |
| [TRADE_EQUIVALENT_EFFICIENCY_ARCHITECTURE.md](TRADE_EQUIVALENT_EFFICIENCY_ARCHITECTURE.md) | proposal | 贸易纸面效率、社区等效换算、搜索排序与产出预估的统一量纲设计 |
| [TRUST_RECOVERY_PLAN.md](TRUST_RECOVERY_PLAN.md) | maintenance-reference | 已修复反馈的关闭审计、防回归矩阵、新反馈处理规则 |
| [QUALITY_90_TO_95_PLAN.md](QUALITY_90_TO_95_PLAN.md) | paused | 历史质量提升方案；只作为参考，不默认继续推进大架构 |

## 冻结的历史计划

| 文件 | 原状态 | 维护期处理 |
|------|--------|------------|
| [SYSTEM_REGISTRY_NORMALIZATION_REPORT.md](SYSTEM_REGISTRY_NORMALIZATION_REPORT.md) | doing | 冻结；只在相关 bug 需要理解 registry 语义时读取 |
| [SYSTEM_ANCHOR_ORCHESTRATION_PLAN.md](SYSTEM_ANCHOR_ORCHESTRATION_PLAN.md) | ready | 冻结；不要为单一 bug 泛化 anchor 三态 |
| [CODEIZED_SYSTEM_ORCHESTRATION_PLAN.md](CODEIZED_SYSTEM_ORCHESTRATION_PLAN.md) | ready | 冻结；迷迭香现状以 ADR 0001 和代码为准 |

这些文件仍保留为历史上下文，不删除、不自动归档。若用户明确恢复功能建设，再逐项确认哪些内容仍然符合当前代码状态。

## 新 bug 记录模板

不建议为每个局部 bug 新建 TODO。若 bug 需要跨会话跟踪，使用中文文件名并采用以下模板：

```markdown
# 简短标题

> 状态：proposal | ready-on-request | in-progress | blocked | completed | superseded
> 来源：用户消息 / issue / debug bundle
> 领域真源：相关 canonical 文档
> 非目标：本任务明确不处理的事项

## 复现

- 命令：
- layout：
- operbox：
- assignment：
- 实际：
- 期望：

## 定位

- 层级：CLI / data / trade / manufacture / search / layout / schedule / export
- 相关文件：

## 修复范围

| 文件/目录 | 动作 |
|-----------|------|
| `path` | 修改 / 新增回归 |

## 验收

- [ ] 命令或测试
- [ ] 回归已补
- [ ] 文档已更新（如需要）

## 完成后

更新 current 文档和索引，再按本文生命周期移动到对应归档目录。
```

## 归档规则

- bug 修复完成：移动到 `docs/ARCHIVE/done/`。
- 历史方案不再采用：移动到 `docs/ARCHIVE/superseded/`。
- 纯历史设计：优先留在 `plans/` 或移动到 `docs/ARCHIVE/plans/`。
- 移动前处理开放项和仓库内引用；归档后不得继续在原路径保留重复副本。
