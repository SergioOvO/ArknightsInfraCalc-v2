# 文档入口

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/文档生命周期.md
> 复核触发：scripts/codex/docs_inventory.py；AGENTS.md；.agents/skills/**
> 摘要：提供文档路由和生成的 canonical 入口
> 源摘要：981f5a744d0679db91bd551eacac55c5b1af4e1f20ebf0ffc72859899946dd8c
> 文档摘要：c99c36a8cb70179d54ecd0adf7b8cc126f0e1a0abaafb4a47fcae425a23f5ae3
> 复核原因：user-ruling
> 复核结论：updated
> 稳定事实：提供文档路由和生成的 canonical 入口
> 证据引用：tracked:docs/INDEX.md

> 实现快照：Current
> 读者：玩家、策略作者、开发者、维护者、AI Agent
> 本文负责：在文档位置未知时提供路由；不定义业务公式，也不是每个任务的无条件首读
> 业务真源：用户当前裁决与对应领域 canonical Markdown

ArknightsInfraCalc 的文档分为展示、当前参考、流程和历史材料。不要按目录顺序通读，也不要用历史 TODO 解释当前行为。

## 渐进式读取

Agent 先由根 `AGENTS.md` 选择任务 Skill：

```text
debug -> arknights-maintenance
feature -> arknights-feature
quality-refactor -> arknights-quality
system / conformance / formal audit -> arknights-system-audit
命令或产物结论 -> arknights-evidence
```

- 已知领域文档时直接完整读取，不需要先读本文。
- 不知道领域真源位置时，用本文定位。
- 不知道代码 owner、命令或调用链时，定向读取 [PROJECT_MAP.md](PROJECT_MAP.md)。
- 只有 `formal-audit` 才完整读取 [SYSTEM_AUDIT_WORKFLOW.md](SYSTEM_AUDIT_WORKFLOW.md)。
- 验证半径、求解保证或交付证据不清时，读取 [QUALITY_AND_AUDIT.md](QUALITY_AND_AUDIT.md) 的相关段。

## 第一次了解项目

这是面向人类读者的建议顺序，不是 Agent 每任务首读清单：

1. [项目首页](../README.md)
2. [项目总览](OVERVIEW.md)
3. [架构导览](ARCHITECTURE_TOUR.md)
4. [243 全精二案例](EXAMPLES/243_FULL_E2.md)
5. [质量与审计](QUALITY_AND_AUDIT.md)
6. [性能工程](PERFORMANCE_ENGINEERING.md)

术语查 [GLOSSARY.md](GLOSSARY.md)，函数和文件位置查 [PROJECT_MAP.md](PROJECT_MAP.md)。

## 按任务意图进入

| 任务 | Skill / 入口 | 后续按需读取 |
|---|---|---|
| bug、结果不对、CLI、数据、solver、局部排班 | [Debug Skill](../.agents/skills/arknights-maintenance/SKILL.md) | [Debug 指南](MAINTENANCE_MODE.md) 对应章节 + 领域文档 |
| 新能力、恢复 TODO、新模式或接口 | [Feature Skill](../.agents/skills/arknights-feature/SKILL.md) | 用户场景对应领域文档、扩展点和调用方 |
| 架构、性能、工作流、技术债或 solver assurance | [Quality Skill](../.agents/skills/arknights-quality/SKILL.md) | [质量与审计](QUALITY_AND_AUDIT.md)、相关架构/性能文档 |
| 体系、跨设施、required admission、scope、Team/Shift bind | [System Audit Skill](../.agents/skills/arknights-system-audit/SKILL.md) | 对应体系 canonical 文档；formal 时再读审计工作流 |
| build、test、CLI、性能或产物证据 | [Evidence Skill](../.agents/skills/arknights-evidence/SKILL.md) | [工具协议](../scripts/codex/README.md)；高风险搜索改动读质量规范 |
| 只读调查或知识提取 | `terra-explorer` / `luna-extractor` | Codex：`.codex/agents/`；OpenCode：`.opencode/agents/`；按独立调查轴读取原始材料 |

具名 subagent 必须由当前运行时的 profile 选择能力实际加载，不能用 `task_name`、昵称或提示词模拟模型路由。结构化提取优先 `luna-extractor`，代码和文档 owner 调查优先 `terra-explorer`，高风险最终反方审阅才使用 `sol-reviewer`；运行时不暴露目标 profile 时，主 Agent 应缩小委派或明确说明默认模型成本。

## 当前 canonical 参考

<!-- BEGIN GENERATED CANONICAL -->
| 领域键 | 权威入口 |
|---|---|
| `architecture.cli` | [infra-cli 模块职责](INFRA_CLI.md) |
| `architecture.orchestration` | [编排层重构路线图（Orchestration Layer）](ORCHESTRATION_LAYER.md) |
| `docs.lifecycle` | [文档生命周期](文档生命周期.md) |
| `facility.control-assignment` | [控制中枢排班规则](CONTROL_CENTER_ASSIGNMENT.md) |
| `facility.manufacture` | [制造站域状态](MANUFACTURE_STATUS.md) |
| `interface.frontend-cli` | [infra-cli + Layout 生成器 — 前端对接说明](FRONTEND_CLI.md) |
| `layout.assignment` | [全基建进驻编制（宏观排班）](BASE_ASSIGNMENT.md) |
| `mechanics.effect-atom` | [EffectAtom 设计文档](EFFECT_ATOM_DESIGN.md) |
| `schedule.fiammetta` | [菲亚梅塔换心情规则](Fiammetta.md) |
| `schedule.mode` | [排班模式](排班模式.md) |
| `schedule.timed` | [定时换班（公孙长乐 × InfraCalc）](定时换班.md) |
| `scoring.efficiency` | [直接效率与整数结算架构](EFFICIENCY_MODEL.md) |
| `scoring.policy` | [评分口径审计](SCORING_MODEL.md) |
| `system.automation-group` | [自动化组体系论证（公孙长乐 × InfraCalc）](公孙长乐的体系分析文档/AUTOMATION_GROUP_CHAIN.md) |
| `system.fireworks` | [人间烟火排班规则](FIREWORKS.md) |
| `system.red-pine-forest` | [红松林体系论证（公孙长乐 × InfraCalc）](公孙长乐的体系分析文档/RED_PINE_FOREST_CHAIN.md) |
| `system.rhine-lab` | [莱茵生命体系论证（公孙长乐 × InfraCalc）](公孙长乐的体系分析文档/RHINE_LAB_CHAIN.md) |
| `system.rosemary-perception` | [迷迭香感知链体系论证（公孙长乐 × InfraCalc）](公孙长乐的体系分析文档/ROSEMARY_PERCEPTION_CHAIN.md) |
| `workflow.maintenance` | [Debug 与一致性修复指南](MAINTENANCE_MODE.md) |
| `workflow.quality` | [质量、求解保证与验证证据总则](QUALITY_AND_AUDIT.md) |
| `workflow.system-audit` | [公孙长乐体系逐项审计与修复工作流](SYSTEM_AUDIT_WORKFLOW.md) |
<!-- END GENERATED CANONICAL -->

实现地图、汇总和示例不在 canonical 表重复登记。当前 ABC 实现见 [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md)，体系导航见 [SYSTEM_CHAINS.md](SYSTEM_CHAINS.md)，干员建模覆盖见 [MODELLED_OPERATORS.md](MODELLED_OPERATORS.md)。

业务裁决顺序为：用户当前明确裁决 → canonical 领域 Markdown → 实现数据和代码。CSV、JSON、注释、测试、fixture 和历史输出不能推翻 Markdown。

## 按领域定位代码或数据

| 任务 | 先读 | 主要入口 |
|---|---|---|
| 修改效率或排序 | [效率模型](EFFICIENCY_MODEL.md)、[评分模型](SCORING_MODEL.md) | `scoring/`、`search/` |
| 修改贸易机制 | [EffectAtom](EFFECT_ATOM_DESIGN.md)、[贸易解释器地图](INTERNAL/TRADE_INTERPRETER.md) | `trade/`、L2、L3 |
| 修改 shortcut | [Shortcut 地图](INTERNAL/SHORTCUT_MATCHING.md) | `trade_shortcuts.json`、`trade/shortcut.rs` |
| 修改制造站 | [制造状态](MANUFACTURE_STATUS.md) | `manufacture/`、`search/manufacture.rs` |
| 修改中枢或全局资源 | [中枢编排](CONTROL_CENTER_ASSIGNMENT.md)、[跨设施地图](INTERNAL/CROSS_FACILITY.md) | `control/`、`global_resource/`、`layout/resolve.rs` |
| 修改完整编排 | [单班编制](BASE_ASSIGNMENT.md)、[编排层](ORCHESTRATION_LAYER.md) | `layout/orchestrate/`、`layout/assign/` |
| 修改轮换或 MAA | [轮换](SCHEDULE_ROTATION.md)、[CLI](INFRA_CLI.md) | `schedule/team_rotation.rs`、`export/maa.rs` |
| 性能、工具人池、Bake | [性能工程](PERFORMANCE_ENGINEERING.md) | `pool/standalone.rs`、`bake.rs`、各设施 search |

## 内部、TODO 与历史材料

- `docs/INTERNAL/`：大模块导航和实现口径，不替代领域真源。
- `docs/ADR/`：已接受架构决策及其原因。
- `docs/TODO/`：未实施提案或未来工作；只有当前 feature/quality 任务明确恢复后才执行。
- `docs/ARCHIVE/`：完成、废弃或仅供追溯的材料。
- 根 `plans/`：不再承载 Markdown；历史设计统一迁入 `docs/ARCHIVE/plans/`。

未来计划必须标注尚未实现并链接 current facts；实施完成后更新当前文档，并归档或更新计划状态，不能让 TODO 冒充运行时能力。
TODO 的创建、恢复、实施同步和自动归档遵循 [文档生命周期](文档生命周期.md)；[TODO/README.md](TODO/README.md) 只承载生成的活动入口。新增人类可读文档优先采用清晰的中文文件名；协议固定名、工具约定和外部兼容路径除外。

## 文档维护

当前文档尽量声明状态、读者、职责、真源、代码入口和验证入口。一项业务规则只保留一个 canonical 定义，其他文档链接摘要。新增命令、测试或性能数字必须有可复现入口；易变化数字标注日期或生成来源。
