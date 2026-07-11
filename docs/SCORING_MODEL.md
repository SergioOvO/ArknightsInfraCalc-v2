# 评分口径审计

> 状态：直接效率硬切版（2026-07-11）
> 完整数值架构见 [EFFICIENCY_MODEL.md](EFFICIENCY_MODEL.md)。

## 1. 生产域排序

| 模块 | 排序字段 | 单位 | 说明 |
|---|---|---|---|
| 贸易搜索 | `TradeSearchHit.final_efficiency` | 直接效率 | 可直接乘三级贸易基准日产出与时长 |
| 制造搜索 | `ManuSearchHit.final_efficiency` | 直接效率 | 单线为房间效率，多线为各产线效率和 |
| 发电搜索 | `PowerSearchHit.final_efficiency` | 直接效率 | 基础、技能与爬升后的充能效率 |

三者都使用 `Efficiency` 千分整数排序。不存在生产域匿名 `score`，也不存在
`trade_pct`、`prod_total`、`charge_speed_pct` 的输出别名。

## 2. 排班汇总

`ShiftEfficiencies` 分别保存：

- `trade_efficiency`；
- `manufacture_efficiency`；
- `power_efficiency`。

每项是同类房间直接效率之和。`weighted_*` 按分钟执行整数时长折算，
`DailyTotals` 仍分开保存贸易、制造和发电，不做跨域相加。

`RoomEfficiencySnapshot` 保存 solver / search 的直接效率结果，避免 CLI 或排班层再次
解释公式。手写 assignment 没有快照时才调用相应 solver 重算。

## 3. 非生产 heuristic

中枢普通排序仍使用具名 policy `ControlInjectRawSumV0`：

```text
trade_inject + manu_gold + manu_br
```

它是局部补位 heuristic，不是生产效率，也不进入三类每日总计。虚拟发电同样作为独立
资源注入制造 resolve，不预支成匿名综合分。中枢搜索结果通过
`breakdown.policy = ControlInjectRawSumV0` 与 `breakdown.policy_sort_key` 暴露排序依据，
不提供 `score` / `total_score` 或 `final_efficiency`。

## 4. 输出约束

- CLI / CSV：效率固定三位小数；
- JSON：效率是量化到三位小数的数值；
- 产量、赤金消耗、心情等非效率物理量保留各自单位；
- `rule_id` 只用于社区换算审计；
- `mechanic_equivalent_efficiency` 只作解释，不二次乘入最终效率。
- 排班 JSON 的班次结算字段为 `efficiencies`，不使用泛型 `scores`。
