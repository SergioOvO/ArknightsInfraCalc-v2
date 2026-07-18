# 基建体系链入口

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/公孙长乐的体系分析文档/ROSEMARY_PERCEPTION_CHAIN.md；docs/公孙长乐的体系分析文档/AUTOMATION_GROUP_CHAIN.md；docs/公孙长乐的体系分析文档/RED_PINE_FOREST_CHAIN.md；docs/公孙长乐的体系分析文档/RHINE_LAB_CHAIN.md
> 复核触发：data/orchestration_rules.json；crates/infra-core/src/layout/orchestrate/**
> 摘要：导航已确认的具名体系 canonical 文档
> 源摘要：1f9ba6e72b5253d2ca32c37e08f0145b05245f8ae5d2a278b92124ceef6cd3a0
> 文档摘要：4f7bdb0526cb51f70c55548f6f9b4fa9584af1b4d877ddda830fde7266d97e0a
> 复核原因：lifecycle-migration
> 复核结论：updated
> 稳定事实：导航已确认的具名体系 canonical 文档
> 证据引用：tracked:docs/SYSTEM_CHAINS.md

本文只提供体系入口，不复制激活条件、成员规则、效率锚点、priority、降级或当前实现状态。所有规范性事实由对应体系 canonical 文档裁决。

| 体系 | Canonical owner | 当前实现入口 |
|---|---|---|
| 迷迭香感知体系 | [迷迭香感知链](公孙长乐的体系分析文档/ROSEMARY_PERCEPTION_CHAIN.md) | `data/orchestration_rules.json`、`layout/orchestrate/`、`global_resource/` |
| 自动化组 | [自动化组](公孙长乐的体系分析文档/AUTOMATION_GROUP_CHAIN.md) | `data/orchestration_rules.json`、`layout/orchestrate/`、`manufacture/`、`power/` |
| 红松林 | [红松林](公孙长乐的体系分析文档/RED_PINE_FOREST_CHAIN.md) | `data/orchestration_rules.json`、`layout/orchestrate/`、`manufacture/` |
| 莱茵 | [莱茵体系](公孙长乐的体系分析文档/RHINE_LAB_CHAIN.md) | `data/orchestration_rules.json`、`layout/orchestrate/`、`manufacture/`、`power/` |

人间烟火是独立全局资源链，见 [FIREWORKS.md](FIREWORKS.md)。贸易 required core 和完整进驻编排分别见 [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md) 与 [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md)。

2026-07-12 至 2026-07-14 的旧实现审计已移入 [ARCHIVE/audits/](ARCHIVE/audits/)，只用于追溯，不能覆盖上述 current owner。
