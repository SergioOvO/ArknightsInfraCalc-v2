# 贸易站社区等效效率架构设计

> 状态：proposal
> 日期：2026-07-11
> 范围：贸易站纸面效率、社区等效换算、搜索排序与产出预估
> 关联文档：[../SCORING_MODEL.md](../SCORING_MODEL.md)、[../SCORING_REFACTOR_PLAN.md](../SCORING_REFACTOR_PLAN.md)、[../EFFECT_ATOM_DESIGN.md](../EFFECT_ATOM_DESIGN.md)、[../INTERNAL/SHORTCUT_MATCHING.md](../INTERNAL/SHORTCUT_MATCHING.md)

## 1. 背景

当前贸易站实现同时使用以下字段：

- `order_eff_base`：每名进驻干员提供的 `1%` 人头效率；
- `order_eff_skill`：技能产生的订单效率加成；
- `order_eff_total` / `trade_pct`：纸面效率或 shortcut 覆盖后的等效效率；
- `mechanic_equiv_eff_pct` / `gold_pct`：订单机制解释分量；
- `effective_eff_multiplier`：部分路径将前两个百分比分别转为倍率后相乘；
- `unit_trade_anchor`：社区工具人表提供的单位产出锚点。

这些字段的量纲没有完全统一。以但书为例，当前实现保留纸面 `trade_pct`，另设固定 `gold_pct=55`，再由调试倍率执行 `(1 + trade_pct / 100) × 1.55`。这会造成：

1. 搜索和展示只读取 `trade_pct` 时低估但书；
2. 产出路径可能再次组合 `trade_pct` 与 `gold_pct`；
3. 固定 shortcut 会覆盖实时的人头效率、中枢注入或挂件技能；
4. 字段名无法说明数值是“加成百分比”“总效率百分比”还是“倍率”。

社区已经为但书、可露希尔、龙舌兰、巫恋等特殊贸易机制提供等效转换。本项目不需要还原随机订单、逐笔交付或游戏内部概率，只需要把社区等效转换作为贸易站的正式结算层。

## 2. 目标与非目标

### 2.1 目标

1. 统一贸易效率量纲，任何字段都能从名称判断其单位。
2. 纸面效率只计算一次，并完整包含基础 `100%`、技能、人头效率和中枢注入。
3. 社区等效规则只应用一次，产出、搜索、排班和展示共享同一个结果。
4. 但书按“整个房间总效率 × 1.55”动态结算，不再使用固定 `+55%` 分量或固定叙拉古总值。
5. 可露希尔、龙舌兰、巫恋等继续使用社区现成的固定值或分档值，不要求还原游戏订单逻辑。
6. 预估产出直接消费最终等效倍率，不再自行拼接多个百分比字段。
7. 迁移期间保持 CLI / JSON 可兼容，并给旧字段明确映射和废弃路径。

### 2.2 非目标

- 不模拟订单生成概率、订单队列或逐笔订单。
- 不重新推导社区已经给出的等效效率。
- 不把贸易与制造、发电合成一个综合分。
- 不改 L1 `EffectAtom` 的技能解释职责。
- 不把 `trade_shortcuts.json` 扩展成通用表达式语言。
- 不因本次重构改变体系认领、落位或三队轮换规则。

## 3. 统一量纲

所有贸易效率只允许使用以下四个核心量：

| 字段 | 单位 | 示例 | 含义 |
|------|------|------|------|
| `paper_bonus_pct` | 加成百分比 | `23.0` | 技能 + 人头 + 中枢，共计 `+23%` |
| `paper_multiplier` | 无量纲倍率 | `1.23` | `1 + paper_bonus_pct / 100` |
| `equivalent_bonus_pct` | 加成百分比 | `90.65` | 社区等效后的 `+90.65%` |
| `equivalent_multiplier` | 无量纲倍率 | `1.9065` | 社区等效后的完整倍率 |

需要展示“总效率百分比”时，由倍率临时生成：

```text
paper_total_pct      = paper_multiplier × 100
equivalent_total_pct = equivalent_multiplier × 100
```

不得把总效率百分比写入带 `_bonus_pct` 后缀的字段。

但书示例：

```text
技能 + 人头 + 中枢 = 23%
paper_bonus_pct      = 23.0
paper_multiplier     = 1.23
equivalent_multiplier = 1.23 × 1.55 = 1.9065
equivalent_bonus_pct  = (1.9065 - 1) × 100 = 90.65
equivalent_total_pct  = 190.65
```

## 4. 结算流水线

```text
TradeRoomInput
  -> TradeContext + apply_trade_phases
  -> PaperTradeEfficiency
       headcount_bonus_pct
       skill_bonus_pct
       global_bonus_pct
       paper_bonus_pct
       paper_multiplier
  -> shortcut / segment 只识别社区规则 ID
  -> apply_community_conversion
  -> EquivalentTradeEfficiency
       rule_id
       equivalent_bonus_pct
       equivalent_multiplier
  -> TradeResult
  -> search / layout / schedule / CLI / production
```

职责边界：

| 层 | 职责 | 不再负责 |
|----|------|----------|
| L1 interpreter | 计算技能、人头、中枢形成的纸面加成 | 社区等效、产出倍率 |
| L2 order mechanic | 保留需要展示的机制说明和兼容数据 | 生成最终排序分、再次乘倍率 |
| L3 shortcut / segment | 识别组合和选择社区规则 ID | 同时覆盖多个可相乘的效率字段 |
| equivalent conversion | 将纸面倍率转换成唯一最终倍率 | 识别干员、执行 EffectAtom |
| production | `基准产出 × equivalent_multiplier × 时长` | 推测 shortcut 该如何组合 |

## 5. 核心类型

建议新增 `crates/infra-core/src/trade/efficiency.rs`：

```rust
pub struct PaperTradeEfficiency {
    pub headcount_bonus_pct: f64,
    pub skill_bonus_pct: f64,
    pub global_bonus_pct: f64,
    pub bonus_pct: f64,
    pub multiplier: f64,
}

pub struct EquivalentTradeEfficiency {
    pub rule_id: Option<String>,
    pub bonus_pct: f64,
    pub multiplier: f64,
}

pub struct TradeEfficiency {
    pub paper: PaperTradeEfficiency,
    pub equivalent: EquivalentTradeEfficiency,
}
```

构造函数维护不变式，调用方不得独立填写同一组的百分比和倍率：

```text
paper.multiplier      == 1 + paper.bonus_pct / 100
equivalent.multiplier == 1 + equivalent.bonus_pct / 100
```

建议新增 `crates/infra-core/src/trade/equivalent.rs`，只包含社区规则的数据类型和换算函数：

```rust
pub enum CommunityEquivalentRule {
    Identity,
    ScaleTotalMultiplier { factor: f64 },
    FixedEquivalentBonusPct { value: f64 },
}
```

当前已知需求只需要三种规则：

| 规则 | 用途 |
|------|------|
| `Identity` | 普通贸易站，等效倍率等于纸面倍率 |
| `ScaleTotalMultiplier` | 但书，对包括基础 `100%` 在内的房间总倍率乘 `1.55` |
| `FixedEquivalentBonusPct` | 可露希尔、龙舌兰、巫恋等社区已给定固定值或分档值 |

不提供四则运算表达式、脚本或任意公式 DSL。出现新的社区换算形态时，再根据真实需求新增具名变体。

## 6. 数据结构

`trade_shortcuts.json` 继续承担社区规则与组合锚点的数据真源，但将计算字段收敛为单一 `conversion`：

```json
{
  "id": "gsl_docus_solo",
  "label": "但书单走",
  "conversion": {
    "kind": "scale_total_multiplier",
    "factor": 1.55
  },
  "match": {
    "kind": "docus"
  }
}
```

固定社区等效值使用加成口径：

```json
{
  "id": "gsl_witch_long_beta",
  "label": "巫恋+龙舌兰+裁缝β",
  "conversion": {
    "kind": "fixed_equivalent_bonus_pct",
    "value": 138.0
  }
}
```

数据约束：

1. `value=138` 表示最终 `+138%`，对应 `2.38` 倍率，不表示总效率 `138%`。
2. `factor=1.55` 必须作用于 `paper_multiplier`，不能作用于 `paper_bonus_pct`。
3. 每个 active shortcut 必须且只能有一个 `conversion`。
4. `trade_pct` / `gold_pct` 不再作为计算输入；迁移期只用于旧数据加载或兼容输出。
5. `unit_trade_anchor` / `unit_gsl_gold_anchor` 降级为回归参考，不得覆盖正式等效倍率。
6. 文件级 `source` 保留；若同一文件混入不同来源，条目增加 `source`，但不引入额外置信度系统。

## 7. 但书与叙拉古链

`gsl_docus_solo` 和 `gsl_docus_syracusa` 使用同一条 `ScaleTotalMultiplier { factor: 1.55 }`。二者可以保留不同 ID，用于体系识别、调试与回归，但不得拥有不同的效率公式。

叙拉古链仍由以下条件决定是否命中：

- 贸易站为但书 + 伺夜 + 贝洛内；
- 中枢有精二八幡海铃；
- 不违反但书同房互斥规则。

八幡海铃、阿米娅或其他中枢产生的实际贸易注入先进入 `paper_bonus_pct`，随后整体乘 `1.55`。`gsl_docus_syracusa` 中现有固定 `trade_pct=200` 只作为旧锚点迁移参考，不能继续覆盖运行时结果。

## 8. 搜索、排班与产出

### 8.1 搜索排序

贸易搜索统一按 `equivalent_bonus_pct` 排序：

```text
TradeSearchHit.score     = equivalent_bonus_pct
TradeSearchHit.trade_pct = equivalent_bonus_pct  // 兼容字段
```

breakdown 同时提供纸面与等效两组值。这样既能解释“原本多少”，也能可靠比较社区等效后的组合。

### 8.2 排班快照

`RoomEfficiencySnapshot` 最终应保存：

- `trade_paper_bonus_pct`；
- `trade_equivalent_bonus_pct`；
- `trade_equivalent_multiplier`；
- `trade_rule_id`。

旧 `trade_score` / `trade_pct` / `trade_gold_pct` 在兼容期继续序列化，但只从新结构派生，不再作为真源。

### 8.3 产出预估

产出入口只接收最终倍率：

```text
estimated_output
  = baseline_output_per_day
  × equivalent_multiplier
  × shift_hours / 24
```

`daily_yield` 不再接收名称含糊的 `order_eff_total_pct`。建议参数改为 `equivalent_multiplier`，从类型和调用点阻止 `/100` 与 `1 + pct/100` 混用。

社区贸易等效效率只能直接推导其定义覆盖的产出分量。如果 `gold_pct` 表示赤金需求解释而不是贸易产出倍率，则继续单独展示，不能乘进 `estimated_output`。赤金消耗预估只有在社区给出独立可靠换算时才接入。

## 9. 兼容迁移

### Phase 1：建立新类型，不改结果

- 新增 `PaperTradeEfficiency` / `EquivalentTradeEfficiency`。
- 普通站先使用 `Identity`。
- `TradeResult` 增加 `efficiency`，旧字段从新结构派生。
- 给百分比与倍率不变式补单元测试。

### Phase 2：迁移社区规则

- `trade_shortcuts.json` 增加 `conversion`。
- 但书 solo 与叙拉古链改为动态 `×1.55`。
- 可露希尔、龙舌兰、巫恋逐条把现有社区锚点迁为固定等效加成。
- shortcut 匹配逻辑保持不变，只改变匹配结果携带的数据。

### Phase 3：切换消费者

- `search/trade.rs` 按 `equivalent_bonus_pct` 排序。
- `box_profile`、`bake`、layout 快照、排班汇总读取新结构。
- CLI / JSON 同时输出纸面值、等效值和规则 ID。
- `daily_yield` 改为消费 `equivalent_multiplier`。

### Phase 4：删除双重口径

- 删除 `TradeShortcutEntry.trade_pct` / `gold_pct` 的计算职责。
- 删除 `TradeShortcutMatch::effective_multiplier()` 的双百分比相乘实现。
- `effective_eff_multiplier`、`mechanic_equiv_eff_pct`、`trade_gold_pct` 完成废弃或降级为纯解释字段。
- 重新 bake 贸易候选数据，禁止混用旧缓存。

## 10. 回归与验收

### 10.1 数学不变式

- `bonus_pct=0` 时倍率为 `1.0`。
- `bonus_pct=23` 时倍率为 `1.23`。
- 任意结果满足 `multiplier == 1 + bonus_pct / 100`。
- `ScaleTotalMultiplier` 只乘纸面总倍率。

### 10.2 但书回归

| 纸面加成 | 纸面倍率 | 等效倍率 | 等效加成 |
|----------|----------|----------|----------|
| `23%` | `1.23` | `1.9065` | `90.65%` |
| `30%` | `1.30` | `2.015` | `101.5%` |
| `83%` | `1.83` | `2.8365` | `183.65%` |

另需断言：

- 人头从 2 人变 3 人时，增加的 `1%` 也进入 `×1.55`；
- 中枢注入 `+7%` 位于乘法之前；
- solo 与叙拉古链在相同纸面值下得到相同等效倍率；
- 叙拉古链不再固定返回 `200%`；
- `gold_pct=55` 不得再次参与产出或排序。

### 10.3 固定社区锚点回归

- 可露希尔、龙舌兰、巫恋当前各分档的 `equivalent_bonus_pct` 与社区表一致；
- 固定规则不受未定义的额外乘法影响；
- shortcut 识别结果和同房互斥保持不变。

### 10.4 集成验收

```bash
mkdir -p target/codex-logs
cargo test -p infra-core --no-run > target/codex-logs/infra-core-test-build.log 2>&1
tail -80 target/codex-logs/infra-core-test-build.log
cargo test -p infra-core --quiet
cargo run -q -p infra-cli -- verify --all
cargo run -q -p infra-cli -- layout test \
  --layout data/fixtures/243/layout.json \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --text
```

验收结果必须同时检查：

1. 但书站的纸面值、规则 ID、等效值；
2. 贸易搜索排序是否使用等效值；
3. `plan` / `team-rotation` 是否仍选择预期贸易核心；
4. JSON 中旧字段与新字段是否满足映射关系；
5. 产出是否只应用一次等效倍率。

## 11. 最终决策摘要

1. L1 继续计算技能、人头和中枢形成的纸面加成。
2. L3 只识别社区规则，不再同时提供多个参与计算的百分比。
3. 新等效层把纸面倍率转换成唯一最终倍率。
4. 但书规则固定为 `paper_multiplier × 1.55`。
5. 可露希尔、龙舌兰、巫恋使用社区给定固定值或分档值。
6. 搜索、排班、展示和产出全部消费同一个等效结果。
7. `gold_pct` 等旧解释分量不得参与隐式二次乘法。
8. 本设计不引入真实订单模拟。
