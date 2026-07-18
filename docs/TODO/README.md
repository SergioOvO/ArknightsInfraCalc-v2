# TODO 任务目录

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/文档生命周期.md
> 复核触发：scripts/codex/docs_inventory.py
> 摘要：说明 TODO 目录并承载生成的活动变更表
> 源摘要：327f6c544e35afb38569076ed48dd729d5afd5fe2afa3cd0f71a7e3e565c0e63
> 文档摘要：7992d506db5b6b505e5a48f13dda133512f4d3803cf4b1a9b7fc3da2bcff5157
> 复核原因：user-ruling
> 复核结论：updated
> 稳定事实：说明 TODO 目录并承载生成的活动变更表
> 证据引用：tracked:docs/TODO/README.md

本目录只容纳 `active-change`。它不是默认工作队列；只有用户当前指令明确选择某项任务时，Agent 才获得实施授权。状态、恢复、交接、关闭和自动归档规则统一见 [文档生命周期](../文档生命周期.md)。

`in-progress` 只用于当前有 writer 的工作树，不能进入默认分支。未完成的会话在交接前改为 `ready-on-request` 或有明确原因的 `blocked`。

## 当前活动变更

<!-- BEGIN GENERATED ACTIVE CHANGES -->
| 文件 | 状态 | 用途 |
|---|---|---|
| [设施无关条件化响应 Bake 实施计划](CONDITIONAL_ROOM_RESPONSE_BAKE_PLAN.md) | `ready-on-request` | 设施无关条件化响应 Bake 的后续实施任务 |
| [动态 Producer A+：设施无关候选列 + 精确索引 Join](DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md) | `ready-on-request` | 动态 producer 候选列、精确索引 Join 和 winner dependency 任务 |
| [练度比对与练卡推荐 RAG 企划](TRAINING_RECOMMENDER_RAG_PLAN.md) | `proposal` | 练度比对、练卡推荐和 RAG 解释层提案 |
| [体系注册表后续缺口](体系注册表后续缺口.md) | `ready-on-request` | 承接旧组合体系规范化报告中仍有效但未获当前授权的开放项 |
| [罗德岛基建管家 — 网站页面结构设计](网站页面结构设计.md) | `proposal` | 网站页面结构和前端线框图提案，尚未获得实施授权 |
| [罗德岛基建管家](罗德岛基建管家.md) | `proposal` | 一站式基建管家的产品愿景提案，尚未获得实施授权 |
| [近期已知缺口修复清单](近期已知缺口修复清单.md) | `ready-on-request` | 按独立授权逐项恢复的近期已知缺口清单 |
<!-- END GENERATED ACTIVE CHANGES -->

新建任务时优先使用清晰中文名，并声明 current owner、目标、非目标、复核触发和成功标准。完成后先吸收 current facts、拆开放项，再移动到对应归档目录。
