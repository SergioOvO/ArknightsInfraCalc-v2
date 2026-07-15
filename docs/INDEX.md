# 文档入口

> 状态：Current
> 读者：玩家、策略作者、开发者、维护者、AI Agent
> 本文负责：文档分层与任务路由；不定义业务公式
> 业务真源：用户当前裁决与对应领域 Markdown

ArknightsInfraCalc v2 的文档分为两层：展示层负责让读者快速理解项目为什么值得关注，参考层负责让实现、审计和维护拥有唯一事实源。不要从 `docs/` 目录顺序通读，也不要用历史 TODO 解释当前行为。

## 第一次了解项目

建议按以下顺序阅读：

1. [项目首页](../README.md)：三分钟了解问题、能力、结果与快速开始。
2. [项目总览](OVERVIEW.md)：从游戏机制到可执行排班的完整故事。
3. [架构导览](ARCHITECTURE_TOUR.md)：沿一次 `plan` 请求理解代码边界。
4. [243 全精二案例](EXAMPLES/243_FULL_E2.md)：查看真实输入、搜索、轮换和产物。
5. [质量与审计](QUALITY_AND_AUDIT.md)：了解结果为什么可以被复现和检查。
6. [性能工程](PERFORMANCE_ENGINEERING.md)：了解工具人池、Bake、bitset 与安全回退。

术语不熟悉时查 [术语表](GLOSSARY.md)。函数和文件位置查 [项目地图](PROJECT_MAP.md)。

## 按读者身份进入

| 我是谁 | 推荐入口 | 接下来阅读 |
|--------|----------|------------|
| 想直接运行排班的玩家 | [CLI 指南](INFRA_CLI.md) | [243 案例](EXAMPLES/243_FULL_E2.md)、[MAA / 前端调用](FRONTEND_CLI.md) |
| 懂基建机制的策略作者 | [公孙长乐运行时总览](GONGSUN_RUNTIME_OVERVIEW.md) | [体系分析文档](公孙长乐的体系分析文档/)、[评分模型](SCORING_MODEL.md) |
| 第一次进入代码的开发者 | [架构导览](ARCHITECTURE_TOUR.md) | [项目地图](PROJECT_MAP.md)、[EffectAtom 设计](EFFECT_ATOM_DESIGN.md) |
| 修复 bug 的维护者 | [维护期 Skill](../.agents/skills/arknights-maintenance/SKILL.md) | [维护期指南](MAINTENANCE_MODE.md)、对应领域文档 |
| 审计体系实现的 Agent | [体系审计 Skill](../.agents/skills/arknights-system-audit/SKILL.md) | [formal audit 工作流](SYSTEM_AUDIT_WORKFLOW.md)、[项目地图](PROJECT_MAP.md) |
| 运行验证的 Agent | [证据 Skill](../.agents/skills/arknights-evidence/SKILL.md) | [质量与证据](QUALITY_AND_AUDIT.md)、[工具协议](../scripts/codex/README.md) |
| 接手未来搜索改造的 Agent | [动态 producer / Bake 计划](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md) | [性能工程](PERFORMANCE_ENGINEERING.md)、[中枢编排](CONTROL_CENTER_ASSIGNMENT.md) |

## 当前权威参考

下列文档定义当前业务或实现边界。展示层若与它们冲突，应修正展示层，不得反向以 README 推翻领域真源。

| 领域 | 权威入口 |
|------|----------|
| 单班完整编制 | [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md) |
| 中枢候选与注入 | [CONTROL_CENTER_ASSIGNMENT.md](CONTROL_CENTER_ASSIGNMENT.md) |
| αβγ 三队轮换 | [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md) |
| 人间烟火 | [FIREWORKS.md](FIREWORKS.md) |
| EffectAtom / Phase / Selector / Action | [EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md) |
| 效率值结构 | [EFFICIENCY_MODEL.md](EFFICIENCY_MODEL.md) |
| 排序 policy | [SCORING_MODEL.md](SCORING_MODEL.md) |
| System → Plan → Execute | [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md)、[ADR 0001](ADR/0001-layout-assignment-decomposition.md) |
| 制造站 | [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md) |
| CLI 与前端 | [INFRA_CLI.md](INFRA_CLI.md)、[FRONTEND_CLI.md](FRONTEND_CLI.md)、[FRONTEND_SERVE_GUIDE.md](FRONTEND_SERVE_GUIDE.md) |
| 干员建模覆盖 | [MODELLED_OPERATORS.md](MODELLED_OPERATORS.md)、[需要完成的干员建模](需要完成的干员建模.md) |

业务规则的裁决顺序为：当前用户明确裁决 → 当前领域 Markdown → 实现数据和代码。CSV、JSON、代码注释、测试与历史输出只能用于实现和核对，不能推翻 Markdown。

## 按维护任务路由

| 任务 | 先读 | 主要代码或数据入口 |
|------|------|--------------------|
| 结果不对、回归失败 | [维护期指南](MAINTENANCE_MODE.md) | 按现象从 CLI → Layout → Search → Solver → Data 缩层 |
| 逐项严格审计体系 | [体系审计工作流](SYSTEM_AUDIT_WORKFLOW.md) | `docs/公孙长乐的体系分析文档/` + 对应生命周期 |
| 运行测试、CLI、性能或生成最终证据 | [质量与证据](QUALITY_AND_AUDIT.md) | `scripts/codex/` + 任务 manifest |
| 修改效率或排序 | [效率模型](EFFICIENCY_MODEL.md)、[评分模型](SCORING_MODEL.md) | `crates/infra-core/src/scoring/`、`search/` |
| 修改贸易机制 | [EffectAtom 设计](EFFECT_ATOM_DESIGN.md)、[贸易解释器地图](INTERNAL/TRADE_INTERPRETER.md) | `trade/interpreter.rs`、L2、L3 |
| 修改 shortcut | [Shortcut 内部地图](INTERNAL/SHORTCUT_MATCHING.md) | `data/trade_shortcuts.json`、`trade/shortcut.rs` |
| 修改制造站 | [制造状态](MANUFACTURE_STATUS.md) | `manufacture/`、`search/manufacture.rs` |
| 修改中枢或全局资源 | [中枢编排](CONTROL_CENTER_ASSIGNMENT.md)、[跨设施地图](INTERNAL/CROSS_FACILITY.md) | `control/`、`global_resource/`、`layout/resolve.rs` |
| 修改完整编排 | [单班编制](BASE_ASSIGNMENT.md)、[编排层](ORCHESTRATION_LAYER.md) | `layout/orchestrate/`、`layout/assign/` |
| 修改轮换或 MAA | [轮换](SCHEDULE_ROTATION.md)、[CLI](INFRA_CLI.md) | `schedule/team_rotation.rs`、`export/maa.rs` |
| 性能、工具人池、Bake | [性能工程](PERFORMANCE_ENGINEERING.md) | `pool/standalone.rs`、`bake.rs`、各设施 search |
| 接手动态 producer 联合搜索 | [实施计划](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md) | 先完成计划中的 Markdown 裁决与修改门禁 |

## 内部地图与架构决策

- `docs/INTERNAL/`：千行级模块的函数导航和实现口径，不替代业务真源。
- `docs/ADR/`：已经接受的架构决策，解释为什么这样拆分。
- [PROJECT_MAP.md](PROJECT_MAP.md)：当前代码、数据和命令地图。
- [QUALITY_AND_AUDIT.md](QUALITY_AND_AUDIT.md)：验证、失败基线、Bake 门禁和完成证据的唯一文字真源。

## 历史材料

- `docs/TODO/`：未实施计划或历史建设材料。维护期默认冻结，只有用户明确恢复时才执行。
- `docs/ARCHIVE/`：已经完成、废弃或仅供追溯的材料。
- [Agent 工作流优化实施计划](ARCHIVE/done/AGENT_WORKFLOW_OPTIMIZATION_PLAN.md)：A-E 已完成；仅供实施历史追溯。
- `plans/`：历史设计记录，不是当前任务入口。

未来计划必须明确标注“尚未实现”，并链接当前事实文档。实现完成后，应更新权威文档并将已完成计划移入归档，不能让 TODO 长期冒充运行时说明。

## 文档维护约定

当前有效文档应尽量在开头写明：状态、读者、本文负责、业务真源、代码入口和验证入口。一项业务规则只保留一个权威定义；其他文档使用链接和摘要，不复制整段公式。新增命令、测试或性能数字必须提供可复现入口，易变化的统计值应标注快照日期或生成来源。
