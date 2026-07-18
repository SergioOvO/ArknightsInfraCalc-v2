# 贸易社区单位产出锚点

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/EFFICIENCY_MODEL.md；docs/EFFECT_ATOM_DESIGN.md
> 复核触发：crates/infra-core/src/trade/unit_output.rs；data/UNIT_OUTPUT_ANCHORS.csv；data/trade_shortcuts.json
> 摘要：说明社区单位产出与贸易解释值边界
> 源摘要：984ae74e6e61578049631b21def4d0b2c2b31981b393c3fe4f84908a8f733308
> 文档摘要：483a883fa5a36d4560160e11fb344b7daccce07b4184dd813af95078ae29d3eb
> 复核原因：lifecycle-migration
> 复核结论：updated
> 稳定事实：说明社区单位产出与贸易解释值边界
> 证据引用：tracked:docs/INTERNAL/TRADE_COMMUNITY_UNIT_OUTPUT.md

> 真源：`工具人表26.5 (2).xlsx`（公孙长乐，2026 年 5 月）与 `排班表图片生成器.md`「三、需求模块二」。

工具人表是截图式视觉排版，不是逐行数据表。解析时只读取明确标注的视觉卡片，不根据相邻行列自动推断组合关系。运行时规范化数据位于 `data/trade_shortcuts.json`。

## 精确锚点

| 规则 | 加强后单位贸易产出 | 来源 |
|------|-------------------:|------|
| 可露希尔特别订单 | `12000` | `Sheet1 H5:H10`：固定 2 赤金 / 1200 龙门币、每小时单位贸易收益 500 |
| 巫恋Ⅱ + 龙舌兰Ⅱ + 裁缝β | `12739.73` | 需求文档三.3 的全精二示例 |
| 但书Ⅱ Lv.1 | `20000` | 用户确认的社区分级锚点，2026-07-11 |
| 但书Ⅱ Lv.2 | `18591.55` | 用户确认的社区分级锚点；`Sheet1 E27` 同时标注约 `1.81` 倍率 |
| 但书Ⅱ Lv.3 | `15910.75` | 用户确认的社区分级锚点；`Sheet1 E5` 同时标注 `1.55` 倍率 |

## 由截图取整等效值派生的锚点

工具人表对其余巫恋档只提供取整后的等效技能效率。按需求文档给出的等效反算关系，在三人站纸面效率 `1.93`、非技能部分 `1.03` 下转换为固定单位产出：

```text
unit_output
  = 10265 × (1.03 + equivalent_skill_bonus) / 1.93
```

| 规则 | 截图值 | 规范化单位产出 | 来源单元格 |
|------|-------:|-----------------:|------------|
| 巫恋Ⅱ + 龙舌兰Ⅱ + 裁缝α | `129%` | `12339.274611` | `Sheet1 B41` |
| 巫恋Ⅱ + 龙舌兰Ⅱ + 白板 | `124%` | `12073.341969` | `Sheet1 B42` |
| 巫恋Ⅱ + 龙舌兰0 + 白板 | `108%` | `11222.357513` | `Sheet1 B43` |
| 巫恋Ⅱ + 裁缝β + 白板 | `93%` | `10424.559585` | `Sheet1 B44` |

这些条目在 JSON 中标记为 `derived_from_rounded_equivalent`，表示计算过程确定，但输入本身只有整数百分比精度。精确原始单位产出一旦获得，应直接替换派生值并改为 `exact`。

## 运行时约束

- 但书、可露希尔、黑键可露希尔和巫恋 shortcut 必须配置 `unit_output`、来源与精度。
- 配置缺失、重复、非正数或非有限数时，solver 直接报错，不回退到旧 `trade_pct` 推算。
- 旧 `trade_pct` / `gold_pct` 已删除；解释值统一为小数 `mechanic_equivalent_efficiency`。
- 最终产出统一为 `10265 × final_efficiency × shift_hours / 24`。
