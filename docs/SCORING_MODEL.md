# 评分口径审计（Scoring Model Audit）

> 状态：首轮审计草案（2026-06-23）  
> 配套计划：[SCORING_REFACTOR_PLAN.md](SCORING_REFACTOR_PLAN.md)  
> 目标：记录项目中所有参与搜索、排序、排班汇总、CLI 展示的评分字段，明确其单位、用途和后续是否需要公孙长乐贸易-制造平衡公式替换。

---

## 1. 口径分类

| 分类 | 单位 | 说明 |
|------|------|------|
| 贸易效率 `%` | percent | 如 `order_eff_total`，贸易站纸面效率，含人头、技能、全局注入 |
| 贸易调试乘积 `multiplier` | multiplier | 如 `effective_eff_multiplier`，由贸易效率与赤金效率组合出的内部调试值；不是用户侧结论，不替代拆开的贸易效率% / 赤金效率% |
| 制造效率 `%` | percent | 如 `prod_total`，制造站纸面产出效率，含人头、技能、全局注入 |
| 发电效率 `%` | percent | 如 `charge_speed_pct`，无人机充能速度 |
| 全局注入 `%` | percent | 中枢或跨设施向贸易/制造写入的效率注入 |
| 平衡复合效率 `%` | percent | 经公孙公式把贸易/制造等贡献折算后的统一效率；待接入 |
| 时长折算效率 | percent × hours/24 | 日排班汇总用，不改变原量纲 |

---

## 2. 总览表

> 本表会随着审计继续更新。`待公式` 表示需要公孙公式确认后再决定最终排序/解释口径。

| 模块 | 字段 / 函数 | 当前值来源 | 单位 | 当前用途 | 是否排序 | 后续处理 |
|------|-------------|------------|------|----------|----------|----------|
| trade search | `TradeSearchHit.score` | `result.order_eff_total` | 贸易效率 % | 贸易三人组排序主键 | 是 | 保留或改名为 `sort_eff_pct`；修注释 |
| trade search | `TradeSearchHit.trade_pct` | `result.order_eff_total` | 贸易效率 % | 展示/解释 | 否 | 保留 |
| trade search | `TradeScoreBreakdown.effective_eff_multiplier` | `result.effective_eff_multiplier` | 调试乘积 | 内部排查单位产出/机制用 | 否 | 不作为用户侧“最终倍率”展示；CLI 优先展示 trade_pct / gold_pct |
| trade search | `unit_trade_per_day` | `result.production.unit.unit_trade_per_day` | 产量/天 | 展示 | 否 | 保留 |
| trade search | `mechanic_equiv_eff_pct` | `result.order_mechanic.mechanic_equiv_eff_pct` | 贸易机制等效效率 % | 解释订单机制 | 否 | 待公式确认是否进入复合效率 |
| manufacture search | `ManuSearchHit.composite_score` | 单配方 `prod_total`；多线按线数加权 | 制造效率 % | 制造三人组排序主键 | 是 | 注释明确；可能改名 |
| manufacture search | `ManuScoreBreakdown.prod_total` | `prod_base + prod_skill + prod_global` | 制造效率 % | 展示/解释 | 否 | 保留 |
| power search | `PowerSearchHit.score` | `charge_speed_pct` | 发电效率 % | 发电站排序主键 | 是 | 修注释，虚拟电站不参与当前排序 |
| power search | `virtual_power_equiv` | `virtual_power * VIRTUAL_POWER_MANU_EQUIV` | 临时解释值 | breakdown 展示 | 否 | 待公式确认是否保留/替换 |
| control search | `ControlScoreBreakdown.total_score` | 普通：`placeholder_trade_manu_balance(...)` 维持 `trade_inject + manu_gold + manu_br`；补位：`ancillary_score` | 混合效率 % | 中枢组合排序 | 是 | **待公式优先替换 placeholder** |
| schedule scoring | `ShiftScores.trade_score` | sum of `effective_eff_multiplier` per trade room | 贸易倍率和 | 排班评估/展示 | 否（不与其他量纲混合） | 需解释搜索口径和评估口径差异 |
| schedule scoring | `ShiftScores.manu_prod_sum` | sum of `prod_total` | 制造效率 % 汇总 | 排班评估/展示 | 否 | 保留 |
| schedule scoring | `ShiftScores.power_charge_sum` | sum of `charge_speed_pct` | 发电效率 % 汇总 | 排班评估/展示 | 否 | 保留 |
| schedule scoring | `weighted_*` | `score * shift_hours / 24` | 时长折算 | 日汇总展示 | 否 | 保留 |

---

## 3. 贸易搜索审计

文件：`crates/infra-core/src/search/trade.rs`

### 当前排序

`eval_combo_hit(...)` 中：

```rust
score: result.order_eff_total,
trade_pct: result.order_eff_total,
gold_pct: result.order_mechanic.mechanic_equiv_eff_pct,
```

`search_trade_single_order(...)` 中按：

```rust
b.score.partial_cmp(&a.score)
```

排序。

### 当前含义

- `score` 当前等于贸易纸面效率 `order_eff_total`；
- `trade_pct` 与 `score` 同值；
- `effective_eff_multiplier` 存在于 breakdown，但当前不作为排序主键，也不作为用户侧结论；
- `unit_trade_per_day` / `output_multiplier` 用于调试产出，不参与当前排序；
- L3 shortcut 命中时，`trade_pct` / `gold_pct` 以公孙长乐 vault 体系文档为权威等效值。

### 发现的问题

文件顶部 `TradeScoreBreakdown` 注释仍描述：

```text
score = effective_eff_multiplier
```

但代码实际排序是 `order_eff_total`。这是历史注释漂移，需要修正。

### 当前约定

- 贸易 meta 复杂组合上不上班走 L3 shortcut / 编排认领；
- shortcut 的 `trade_pct` / `gold_pct` 是公孙长乐 vault 体系文档给出的等效贸易效率 / 赤金效率；
- CLI 与文档应拆开展示贸易效率% 与赤金效率%，不要创造一个用户侧“最终倍率”；
- `effective_eff_multiplier` 仅保留为内部调试乘积，用于核对 solver / unit output。

---

## 4. 制造搜索审计

文件：`crates/infra-core/src/search/manufacture.rs`

### 当前排序

单配方搜索按：

```rust
b.composite_score.partial_cmp(&a.composite_score)
```

排序。

单配方下 `composite_score` 来自 `eval_single_recipe_hit`，当前语义为该配方 `prod_total`。

多产线模式下：

```rust
composite_score = gold_lines * gold_report.best.composite_score
                + battle_record_lines * br_report.best.composite_score
```

### 当前含义

- 单配方：制造效率 `%`；
- 多产线：按产线数加权后的制造效率 `%`；
- `storage` 仅作为 tie-break 或展示，不是主排序口径。

### 待确认

- 赤金线和经验线是否在所有玩家目标下等权？
- 公孙公式是否会改变多线模式下 gold / battle_record 的权重？

---

## 5. 发电搜索审计

文件：`crates/infra-core/src/search/power.rs`

### 当前排序

```rust
pub fn power_station_score(charge_speed_pct: f64, _virtual_power_produced: f64) -> f64 {
    charge_speed_pct
}
```

### 当前含义

- 发电站排序只看 `charge_speed_pct`；
- `virtual_power_produced` 当前不影响排序；
- `virtual_power_equiv` 在 breakdown 中保留解释，但不是当前排序依据。

### 发现的问题

`VIRTUAL_POWER_MANU_EQUIV` 和相关注释仍容易让人误解为虚拟发电已参与排序。需要在 Phase 1 明确：当前不预支虚拟发电价值。

### 待确认

- 公孙公式是否提供虚拟发电 → 制造效率的稳定折算？
- 如提供，折算应进入发电站搜索，还是只进入排班总评估？

---

## 6. 中枢搜索审计

文件：`crates/infra-core/src/search/control.rs`

### 当前排序

`score_control_result(...)` 中：

- `ControlFillPolicy::HrAndMood`：
  - `total_score = ancillary_score`
- 默认效率策略：
  - `inject_subtotal = trade_inject + manu_gold + manu_br`
  - `total_score = inject_subtotal`

### 当前含义

中枢普通排序是在裸加贸易注入和制造注入，属于当前最需要公式化的点。

### 风险

`trade_inject`、`manu_gold`、`manu_br` 虽然都是 `%`，但不是同一经济语境下的同质贡献。直接相加可能是历史 AI 简化口径。

### 后续处理

当前已通过 `scoring::placeholder_trade_manu_balance(...)` 收敛到公式入口；公孙公式到位后优先替换该入口的真实实现 / 调用方式：

```rust
GongsunTradeManuBalance::control_inject_eff(...)
```

或等价公式入口。

---

## 7. 排班层评分审计

文件：

- `crates/infra-core/src/schedule/base_rotation.rs`
- `crates/infra-core/src/schedule/team_rotation.rs`

### 当前评分

`score_base_assignment(...)`：

- 贸易：`trade_score += result.effective_eff_multiplier`（历史内部调试/汇总字段；用户侧输出应优先展示逐房 `trade_pct` / `gold_pct`）
- 制造：`manu_prod_sum += result.prod_total`
- 发电：`power_charge_sum += charge_speed_pct`

`ShiftScores::weighted_*`：

```rust
score * shift_hours / 24.0
```

### 当前含义

排班层保留三类分量，不硬合成一个全局分。该设计应保留。

### 需要解释的差异

贸易搜索主键是 `order_eff_total` / `trade_pct`。L3 shortcut 命中时该值就是 vault 等效贸易效率，`gold_pct` 另列赤金效率。排班层仍有历史 `effective_eff_multiplier` 汇总字段，但用户侧不应把它当成更高优先级的“最终倍率”：

- 搜索 / 编排：复杂贸易 meta 先走 shortcut / priority，普通散件按贸易效率；
- 展示：拆开展示贸易效率% 与赤金效率%；
- 调试：`effective_eff_multiplier` / `output_multiplier` 可用于核对产出，但不作为用户口径。

---

## 8. 后续审计 TODO

- [ ] 审计 `schedule/trade_rotation.rs` 的 `total_score` 是否只是 trade hit score 求和；
- [ ] 审计 `layout/assign.rs` 中各设施落位是否使用了正确排序字段；
- [ ] 审计 `box_profile/eval.rs` 是否混用了产量和效率；
- [ ] 审计 CLI 输出字段命名是否会误导用户；
- [ ] 确认 `data/trade_shortcuts.json` 中 shortcut 的效率口径；
- [ ] 等公孙公式后补充公式锚点和实例。
