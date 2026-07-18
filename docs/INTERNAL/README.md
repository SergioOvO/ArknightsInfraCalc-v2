# 大文件内部地图

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/文档生命周期.md；docs/PROJECT_MAP.md
> 复核触发：crates/infra-core/src/trade/**；crates/infra-core/src/cross_facility/**
> 摘要：路由大模块内部实现地图和对应 current owner
> 源摘要：5c8954b03bd80d49780d0fda95e9e79c45aae2530e436c2223982890bf218e69
> 文档摘要：c83ace131b8904d0338e76645446158708c7ea356700643e123c8e4f0e901942
> 复核原因：user-ruling
> 复核结论：updated
> 稳定事实：路由大模块内部实现地图和对应 current owner
> 证据引用：tracked:docs/INTERNAL/README.md

> 代码不拆分；用本文档族把千行级文件「切片」给 Agent / 开发者。**改前先查表，再打开对应函数段。**

| 文档 | 对应源码 | 何时读 |
|------|----------|--------|
| [TRADE_INTERPRETER.md](TRADE_INTERPRETER.md) | `crates/infra-core/src/trade/interpreter.rs` | 改 L1 Phase、Condition、Selector、效率/上限叠加 |
| [SHORTCUT_MATCHING.md](SHORTCUT_MATCHING.md) | `crates/infra-core/src/trade/shortcut.rs` | 改 L3 匹配、同房互斥、shortcut 回归 |
| [CROSS_FACILITY.md](CROSS_FACILITY.md) | `crates/infra-core/src/cross_facility/` | 改跨房 buff、scope=Global atom、resolve.rs 编排集成 |

编排层当前主线见 [ORCHESTRATION_LAYER.md](../ORCHESTRATION_LAYER.md)（`layout/orchestrate/` 已落地；System → Plan → Execute）。

制造站 L1 与贸易站对称，见 [MANUFACTURE_STATUS.md](../MANUFACTURE_STATUS.md)（入口 `manufacture/interpreter.rs`，结构类似 TRADE_INTERPRETER）。

CLI 输出层：`infra-cli/src/output.rs` 按 `emit_pool` / `emit_trade_search` / `emit_bench` / `emit_trade_yield` / `emit_team_rotation` 等函数名定位，无需通读全文。
