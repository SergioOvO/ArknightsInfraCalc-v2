# 评分口径收敛与公孙公式接入计划

> 状态：执行中 v1（2026-06-23）  
> 已完成：Phase 0 首轮审计、Phase 1 注释修正、Phase 2 排序 key 显式化、Phase 3 `scoring` / `balance` 公式接口、Phase 4 当前行为锁定测试；安全 warning hygiene 已完成。  
> 待做：Phase 5 等公孙长乐公式到位后接入真实平衡公式。  
> 目标：在不重写现有求解器、不破坏百毫秒级求解性能的前提下，把历史遗留的 `score` / `composite_score` / `total_score` 口径收敛为**效率量纲**，并为公孙长乐提供的贸易-制造平衡公式预留唯一接入口。

---

## 1. 背景

本项目是基于 Cursor + DeepSeek 快速迭代出的明日方舟基建排班引擎：

- 机制层：EffectAtom、L1/L2/L3、shortcut、global resource；
- 理论层：公孙长乐提供体系理论、排班理论、贸易-制造平衡公式；
- 求解层：通过体系编排、剪枝、局部搜索、`top_k` 回退实现百毫秒内排班。

早期实现中，AI 曾引入过若干自创评分字段和综合分。后来项目方向已约束为：

> **站内搜索使用效率贪心；跨贸易/制造的比较必须使用公孙长乐提供的严谨平衡公式。**

当前要做的不是重写 solver，而是把评分口径显式化、可审计、可替换、可测试。

---

## 2. 总目标

### 2.1 必须达成

1. 所有参与排序的字段都明确单位：贸易效率%、制造效率%、充能效率%、复合等效效率等。
2. 所有跨贸易/制造/全局注入的换算都收敛到一个公式入口。
3. 在公孙公式未落地前，不猜测权重、不发明综合分。
4. 现有排班结果尽量保持稳定，先文档化和接口化，再接真实公式。
5. 用测试锁定关键口径，防止后续 AI 或人误改。

### 2.2 非目标

- 不引入通用约束求解器。
- 不做全局 `trade * x + manu * y + power * z` 的任意综合分。
- 不重写 EffectAtom / L1-L3 求解语义。
- 不因为公式接入而牺牲百毫秒级主路径性能。
- 不在公式未确认前改变大量排序结果。

---

## 3. 核心原则

### 3.1 效率量纲优先

项目中所有排序口径都应落入以下明确类型之一：

| 口径 | 含义 | 例子 | 是否可直接排序 |
|------|------|------|----------------|
| Facility efficiency | 设施内纸面效率 | 贸易 `order_eff_total`、制造 `prod_total`、发电 `charge_speed_pct` | 同设施内可以 |
| Mechanic-adjusted metric | 机制解释值 | 贸易 `mechanic_equiv_eff_pct`、`effective_eff_multiplier` | `mechanic_equiv_eff_pct` 作为赤金效率展示；`effective_eff_multiplier` 仅作内部调试乘积，不作为用户侧“最终倍率” |
| Balanced equivalent efficiency | 经公孙公式换算后的复合效率 | 黑键/但书/中枢注入的贸易-制造复合贡献 | 可以用于体系竞争 / 跨设施比较 |
| Schedule weighted efficiency | 按班次时长折算后的效率 | `eff * shift_hours / 24` | 用于日汇总 / 展示 |

### 3.2 站内搜索与体系竞争分层

| 层级 | 推荐口径 |
|------|----------|
| 贸易站 C(n,3) 搜索 | 贸易效率 `trade_pct`；L3 shortcut 命中时为公孙 vault 等效贸易效率，赤金效率 `gold_pct` 单独展示；不合成用户侧最终倍率 |
| 制造站 C(n,3) 搜索 | 制造产出效率 `prod_total` |
| 发电站 O(n) 搜索 | 充能效率 `charge_speed_pct`；虚拟电站是否折算由公式决定 |
| 控制中枢补位 | 全局注入的平衡等效效率；公式未接入前先保留现状并标记 |
| 体系选择 / 中枢竞争 | 公孙平衡公式后的复合效率 + 体系硬约束 / priority |
| 排班日汇总 | 保留 trade/manu/power 分量；可新增 balanced summary，不替代原始分量 |

### 3.3 公式入口唯一化

严禁散落写法：

```rust
trade_inject + manu_gold + manu_br
trade_eff * 1.5 + manu_eff
virtual_power * arbitrary_weight
```

跨设施换算必须进入统一模块，例如：

```text
crates/infra-core/src/scoring/balance.rs
```

或等价路径。

---

## 4. 推荐模块设计

### 4.1 新增模块

建议新增：

```text
crates/infra-core/src/scoring/
  mod.rs
  balance.rs
  metric.rs
```

职责：

| 文件 | 职责 |
|------|------|
| `metric.rs` | 定义效率口径结构和单位说明，如 `EffPct`、`BalancedEff`、`ScoreUnit` |
| `balance.rs` | 公孙贸易-制造平衡公式唯一入口；公式未到时仅放 placeholder / trait |
| `mod.rs` | 对外导出评分口径和公式入口 |

### 4.2 初始接口草案

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceFormulaId {
    Placeholder,
    GongsunTradeManuV1,
}

#[derive(Debug, Clone, Copy)]
pub struct TradeManuBalanceInput {
    pub trade_eff_pct: f64,
    pub gold_manu_eff_pct: f64,
    pub battle_record_manu_eff_pct: f64,
    pub trade_station_count: u8,
    pub gold_line_count: u8,
    pub battle_record_line_count: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct BalancedEff {
    pub formula: BalanceFormulaId,
    pub composite_eff_pct: f64,
}
```

公式未到前，允许 placeholder 存在，但必须明确：

- placeholder 不代表最终理论；
- 不能作为回归锚点；
- 只用于打通接口和标记调用点。

---

## 5. 分阶段实施

### Phase 0：评分口径审计（已完成首轮）

产出：[SCORING_MODEL.md](SCORING_MODEL.md)

任务：

- 列出所有 `score` / `composite_score` / `total_score` / `weighted_*` 字段；
- 标记当前单位、用途、是否参与排序；
- 标记是否需要公孙公式替换；
- 标记注释与代码不一致处。

建议审计文件：

- `crates/infra-core/src/search/trade.rs`
- `crates/infra-core/src/search/manufacture.rs`
- `crates/infra-core/src/search/power.rs`
- `crates/infra-core/src/search/control.rs`
- `crates/infra-core/src/schedule/base_rotation.rs`
- `crates/infra-core/src/schedule/team_rotation.rs`
- `crates/infra-core/src/schedule/trade_rotation.rs`
- `crates/infra-core/src/layout/assign.rs`

预计工期：0.5 天。

### Phase 1：修正文档和注释，不改行为（已完成）

任务：

- 修正 trade 注释：`score` 当前是 `order_eff_total`，不是 `effective_eff_multiplier`；
- 修正 power 注释：当前排序只看 `charge_speed_pct`，虚拟发电字段仅解释 / 后续公式；
- 修正 control 注释：`inject_subtotal` 是临时注入口径，待公孙公式替换；
- 在 PROJECT_MAP / 相关文档中链接评分口径文档。

预计工期：0.5 天。

### Phase 2：抽排序 key 函数，不改排序结果（已完成）

任务：

- 贸易：`trade_efficiency_sort_key(hit)`；
- 制造：`manufacture_efficiency_sort_key(hit)`；
- 发电：`power_efficiency_sort_key(hit)`；
- 中枢：`control_inject_sort_key(hit)`；
- 排班层保留分量汇总，不混量纲。

原则：函数初始返回值必须与现有行为一致。当前已完成并由测试锁定。

预计工期：0.5–1 天。

### Phase 3：建立公式接口，不接真实公式（已完成）

已完成：

- 新增 `scoring` 或 `economy` 模块；
- 定义公式 ID、输入、输出；
- 在中枢搜索裸加注入口径处接入 placeholder 调用点；
- 不改变主路径排序结果。

预计工期：0.5–1 天。

### Phase 4：加当前行为锁定测试（已完成）

任务：

- 贸易：锁定 `score == order_eff_total` 或明确 sort key == `trade_pct`；
- 制造：锁定 `composite_score == prod_total`；
- 发电：锁定 `score == charge_speed_pct`；
- 中枢：锁定当前 `total_score` 来源，并标记待公式替换；
- 全精2 243 fixture 烟测仍通过。

已完成测试锚点：

- `search::trade::tests::trade_search_score_is_paper_efficiency_sort_key`
- `search::manufacture::tests::manufacture_search_score_is_prod_total_sort_key`
- `search::power::tests::power_search_score_is_charge_speed_sort_key`
- `search::control::tests::control_search_score_uses_total_score_sort_key`

预计工期：0.5–1 天。

### Phase 5：接入公孙公式（等待公式）

公式到位后：

- 在 `balance.rs` 实现 `GongsunTradeManuV1`；
- 增加公孙提供的锚点测试；
- 替换中枢注入评分；
- 评估是否替换体系竞争 / meta priority 的部分逻辑；
- 输出中显示公式 ID 和复合效率。

预计工期：2–3 天，取决于公式复杂度和锚点数量。

---

## 6. 当前审计结论

> 该节会随着代码审计持续更新。2026-06-24 已完成首轮审计、注释修正、排序 key 显式化、行为锁定测试和 Phase 3 placeholder 公式入口。

### 6.1 贸易搜索

文件：`crates/infra-core/src/search/trade.rs`

当前观察：

- `TradeSearchHit.score` 当前赋值为 `result.order_eff_total`；
- `trade_pct` 也是 `result.order_eff_total`；
- `effective_eff_multiplier` 在 breakdown 中，仅作为内部调试乘积；CLI / 文档优先拆开展示贸易效率与赤金效率；
- 原“score = effective_eff_multiplier”的历史注释已修正；
- 排序已显式走 `trade_efficiency_sort_key(hit)`，行为保持不变。

### 6.2 制造搜索

文件：`crates/infra-core/src/search/manufacture.rs`

当前观察：

- 单配方搜索按 `composite_score` 排序；
- 单配方 `composite_score` 实际为 `prod_total`；
- 多产线时按赤金线数 / 经验线数对各自 `prod_total` 加权；
- 排序已显式走 `manufacture_efficiency_sort_key(hit)`，行为保持不变；
- 该口径基本是制造效率量纲，但字段名“composite”仍需在后续公式接入时评估是否改名。

### 6.3 发电搜索

文件：`crates/infra-core/src/search/power.rs`

当前观察：

- `power_station_score(charge_speed_pct, _virtual_power_produced)` 当前只返回 `charge_speed_pct`；
- `virtual_power_equiv` / `VIRTUAL_POWER_MANU_EQUIV` 字段残留解释价值，但不参与当前排序；
- 注释已明确当前不预支虚拟发电价值，是否折算等待公孙公式；
- 排序已显式走 `power_efficiency_sort_key(hit)`，行为保持不变。

### 6.4 中枢搜索

文件：`crates/infra-core/src/search/control.rs`

当前观察：

- 普通中枢补位 `total_score = trade_inject + manu_gold + manu_br`；
- `HrAndMood` 策略下使用 `ancillary_score`；
- `matatabi` / `virtual_power` / `mood` 等字段在 breakdown 中存在，但当前普通排序主键不是它们；
- 裸加口径已通过 `placeholder_trade_manu_balance` 进入 `scoring` 公式入口，等待公孙平衡公式替换；
- 排序已显式走 `control_inject_sort_key(hit)`，行为保持不变；
- 中枢是最优先等待公孙平衡公式替换的评分点。

### 6.5 排班汇总

文件：`crates/infra-core/src/schedule/base_rotation.rs`、`team_rotation.rs`

当前观察：

- `ShiftScores` 分开存 `trade_score` / `manu_prod_sum` / `power_charge_sum`；
- `weighted_*` 只做时长折算；
- 当前没有把三类硬加成一个全局分，这一点应保留；
- 后续可新增 `balanced_eff` 字段，但不应替代原始分量。

---

## 7. Warning hygiene 记录

2026-06-23 已完成一轮安全 warning 清理，仅处理：

- unused import；
- unused variable；
- unused mut；
- 测试专用 import 下沉；
- 当前确认无用的局部变量。

未处理且暂不建议处理：

- `private_interfaces`：涉及内部数据结构可见性，可能影响 API / serde 边界；
- `dead_code`：多为未来机制、公式接入或 schema 预留；
- 预留字段未读：如 system schema、导出输入结构等；
- 未来机制常量未使用：等待 Phase 4 global effect / 公孙公式接入。

验证：`cargo test -p infra-core` 通过，289 passed；剩余 warning 为上述保留类别。

---

## 8. 风险和防护

| 风险 | 防护 |
|------|------|
| 误把展示字段用于排序 | 抽 sort key 函数 + 测试 |
| 公式未到前又发明权重 | placeholder 明确禁止作为最终口径 |
| 改评分导致排班大范围变化 | Phase 0–4 不改行为，公式接入后用锚点测试 |
| 体系 priority 与公式冲突 | 先并存：priority 是硬先验，公式用于解释 / 局部替换 |
| 性能退化 | 公式应为 O(1) 纯函数；不引入全局笛卡尔积 |

---

## 9. 待公孙公式确认的问题

1. 贸易效率与制造效率的平衡公式具体形式是什么？
2. 赤金线和经验线是否同权？是否依赖玩家目标？
3. 贸易订单机制等效效率对用户侧拆分为贸易效率 / 赤金效率展示；不要合成用户侧最终倍率。
4. 黑键、但书、巫恋、孑等机制锚点以 `arknights-base-vault/docs/2-体系/` 的公孙长乐体系文档为权威，代码和 shortcut 表需向 vault 对齐。
5. 中枢全局贸易 +7%、制造 +2% 如何在 243 / 252 / 333 下折算？
6. 虚拟发电站是否应折算成制造等效效率？如果是，公式是否依赖布局/无人机策略？
7. 复合效率用于哪些层级：体系选择、中枢竞争、日报告，还是站内搜索也使用？

---

## 10. 建议验收标准

Phase 0–4 完成后：

- [x] `docs/SCORING_MODEL.md` 完成首轮审计表；
- [x] 主要 score 字段注释与实际代码一致；
- [x] 所有搜索模块有明确 sort key 入口或明确注释；
- [x] 公孙公式模块接口存在；
- [x] 当前核心测试通过；
- [x] 全精2 243 `layout team-rotation` 仍可在百毫秒级完成。

公式接入后：

- [ ] 公孙锚点测试通过；
- [ ] 中枢注入评分不再直接裸加 trade/manu；
- [ ] 输出能说明使用的公式 ID；
- [ ] 关键体系选择结果符合理论预期。 
