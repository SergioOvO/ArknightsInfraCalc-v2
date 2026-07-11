# trade/shortcut L3 匹配地图

> 文件：`crates/infra-core/src/trade/shortcut.rs`  
> 数据：`data/trade_shortcuts.json`  
> 设计背景：`EFFECT_ATOM_DESIGN.md` §8.7

社区单位产出及视觉表解析来源见 [TRADE_COMMUNITY_UNIT_OUTPUT.md](TRADE_COMMUNITY_UNIT_OUTPUT.md)。特殊订单 shortcut 必须携带带来源和精度标记的 `unit_output`；加载失败时直接报错，不再静默使用旧等效百分比。

## 求解器中的位置

`solver.rs` 在 L1（`apply_trade_phases`）结束后：

1. 取 `order_eff_pre = ctx.order_eff_total()`
2. 若赤金订单 → `resolve_trade_shortcut(..., &layout.global_inject)`
3. 命中则走 L3：`sc.build_mechanic_result` + 社区 `unit_output`，并输出小数 `mechanic_equivalent_efficiency`
4. 未命中则走 L2 `order_mechanic::resolve_order_mechanic`

互斥违规在 solver 入口直接 `Err`（`trade_station_exclusive_violation`）。

## 链段注册表（`trade_segments.json`）

数据：`data/trade_segments.json`；代码：`trade/segment.rs`（求解命中）、`search/role_pick.rs`（meta 落位 fallback 链）。

| 字段 | 含义 |
|------|------|
| `segments[]` | producer 条件 + `consumer` 种类 + `shortcut_id` + `priority` |
| `roles[].pick_steps` | meta 站落位顺序：`segment` / `shortcut` / `filtered` / `unfiltered`；可带 `must_include_name` 或 `must_include_names` |

**Producer**（`GlobalInjectManifest`）：`haru_e2_in_control`、`daifeen_e2_in_control` 等；`karlan_precision` 仍是全局注入，但喀兰市井孑已改走 L1 自然计算，不再注册 active L3 segment。

**Consumer**（Rust 匹配器）：`docus_syracusa`、`blackkey_closure`、`vina_lungmen`、`penguin_*`。

**贸易 core role fallback 链**：

- `docus`：`segment/docus_syracusa`（仅 `haru_e2_in_control` 时尝试）→ `gsl_docus_syracusa`，再 `gsl_docus_solo`，最后 `unfiltered + must_include_name=但书`。无但书时 role 失败，由调用方进入下一个 role 或 plain；不会把无但书 plain 误报为 docus。
- `closure`：`gsl_blackkey_closure` 优先，再 `closure` 分档，最后 `unfiltered + must_include_name=可露希尔`。黑键缺失不影响可露希尔核心上站。
- `witch`：`filtered hit_filter=witch + must_include_names=[巫恋, 龙舌兰]`，只表示高优先级龙巫；由 `classify_witch_room` 覆盖龙舌兰精二 + 裁缝 β / α / 空白第三人等 fallback。
- `meta_vina`：仅 `segment/vina_lungmen`，且必须由 `daifeen_e2_in_control` producer 激活；无 producer 时失败，不 fallback 成 plain。推进之王以 0% 贸易触发器入池，用于触发摩根/格拉斯哥同站技能；优先级高于无龙舌兰巫恋兜底。
- `witch_fallback`：`filtered hit_filter=witch + must_include_name=巫恋`，只做无龙舌兰时的低优先兜底。

`resolve_trade_shortcut` 在巫恋/可露之前调用 `match_registered_trade_segment`（按 `priority`）。

## 成套方案认领（`base_systems.json`）

数据：`data/base_systems.json`（每 System 含 `"tier"` 字段：`cross_station` / `same_station`）；主路径代码：`layout/orchestrate::{build_plan, execute_plan}`，其中 `build_plan` 调用 `select_registry_systems`。`layout/system.rs::claim_base_systems` 仅作兼容 / 测试辅助入口。

在 `assign_shift` **开头**（高峰班）由 `build_plan` 按 **tier 两阶段**贪心认领真正的跨站体系 / fixed bond：先 `CrossStation`（跨站体系）、后 `SameStation`（同站组合），各阶段内按 `priority` 排序，`exclusive_group` 互斥态跨阶段共享。随后 `execute_plan` 先占 `control` / `trade_post` 等空房并写入 `used`；后续设施贪心跳过已占房间。中枢若只钉了体系内 1 人（如海铃），`assign_control` 会**补满剩余席位**而非整房重搜。

来源：公孙长乐工具人表（`scripts/build_base_systems_from_gongsun_xlsx.py` 维护小目录）。`exclusive_group` 互斥（如 `meta_chain`：叙拉古/喀兰/推王/怪猎四选一）；`pick_one` 在认领时按顺序取盒内第一个可用干员。贸易核心优先（但书、可露希尔、巫恋）不再依赖 `base_systems` fixed 认领；`assign_shift` 会跳过 `witch_long_beta`、`blackkey_closure`、企鹅、推王等旧抢站条目，改由 `roles` 搜索。贸易 L3 锚点仍在 `trade_shortcuts.json`，但 `gsl_ling_jie_yaxin` 仅作参考锚点，不参与 active 匹配。

## 匹配优先级（`resolve_trade_shortcut`）

```
互斥检查 → None（solver 层已 Err，此处双保险）
    ↓
链段表 match_registered_trade_segment（docus_syracusa / blackkey_closure / vina_lungmen / penguin_*）
    ↓
但书单走 match_docus_solo_shortcut
    ↓
巫恋核 match_witch_group_shortcut
    ↓
可露希尔分档 match_closure_shortcut
    ↓
None → 走 L2
```

## 同房互斥（`trade_station_exclusive_violation`）

以下组合**禁止同站**（搜索 / 轮换 / 求解均拒绝）：

| 规则 | 函数 | 说明 |
|------|------|------|
| 但书 × 巫恋侧 | `docus_tailor_exclusive_violation` | 但书（合同法/违约）与巫恋低语 / 龙舌兰投资 / 裁缝 αβ 不得同房 |
| 佩佩 × 效率人 | `pepe_station_trade_eff_violation` | 佩佩独占站时，他人不得有 `constant` 阶段 `AddFlatEff` |
| 但书 × 可露希尔 | `trade_station_exclusive_violation` 内 | 违约链与特别订单 |
| 巫恋低语 × 可露希尔 | 同上 | 低语清零与特别订单 |

**巫恋侧判定**（`room_has_witch_side_group`）：精二巫恋且 `PeerEffAbsorb rate>0`；或龙舌兰投资；或同房有裁缝 α/β。

## 但书单走（`gsl_docus_solo`）

- 条件：`is_docus_solo_station` — ≥3 人、有但书机制 buff、无巫恋侧。
- `unit_output` 按贸易站等级提供社区加强日产出；三级站相对 `10265` 为 `×1.55`。
- 最终得分为完整纸面效率（基础 100% + 人头 + 技能 + 中枢）乘单位产出倍率。
- `mechanic_equivalent_efficiency=0.550` 仅作解释，不再参与排序或产出乘法。

## 巫恋核（`gsl_witch_*`）

前提：`has_witch_e2`（精二巫恋 + `rate_per_peer > 0`），且同房**无**但书、**无**可露希尔。

`classify_witch_room` → shortcut id：

| 条件 | shortcut id |
|------|-------------|
| 龙舌兰精二 + 裁缝 β（第三人） | `gsl_witch_long_beta` |
| 龙舌兰精二 + 裁缝 α（无 β） | `gsl_witch_long_alpha` |
| 龙舌兰精二 + 空白第三人 | `gsl_witch_long_blank` |
| 龙舌兰精0 + 空白第三人 | `gsl_witch_long0_blank` |
| 无龙舌兰精二 + β + 空白第三人 | `gsl_witch_beta_blank` |

表内 `unit_output` 是社区单位产出真源，`mechanic_equivalent_efficiency` 是解释锚点；L1 仍计算纸面效率。

## 灵知市井孑（L1 自然计算）

- 当前不走 active L3：`trade_segments.json` 无 `ling_jie`，`shortcut.rs` 无 `match_ling_jie_shortcut`。
- `base_systems.json` 的 `ling_jie_karlan` 只认领灵知 E2 中枢；贸易站由 L1 搜索在 `karlan_precision` 激活时注入精1+ 市井孑，再自然选择银灰、琳琅诗怀雅、崖心、讯使等第三人。
- 回归：`reg_ling_jie_yaxin_natural` 断言中枢灵知 E2 + 精1+孑 / 银灰 / 琳琅诗怀雅的最终直接效率为 **2.290**，且 `rule_id=None`。
- 129 拆法：银灰受精密计算后 5% + 琳琅 20% = 25%；孑按 18 单上限给 72%；琳琅按超出 10 单的 8 单给 32%；合计 129%。
- `gsl_ling_jie_yaxin` 保留在 `trade_shortcuts.json` 作为参考锚点，不应出现在 solver 输出的 `trade_shortcut`。

## 可露希尔分档（`match.kind == "closure"`）

- 条件：`has_closure`、无巫恋低语、无但书。
- 在 `trade_shortcuts.json` 中筛 `match.kind == "closure"` 的条目。
- 选 `station_bonus_efficiency_anchor` 与纸面加成效率**距离最小**的档。
- 若最小距离 **> 25** → 不匹配（回退 L2）。
- 典型 case_id：`reg_gsl_closure_tier90` / `tier80` / `tier60`。

## 数据文件字段

| 字段 | 含义 |
|------|------|
| `id` | 回归 `rule_id`、solver 输出 `rule_id` |
| `mechanic_equivalent_efficiency` | 小数机制解释值，不参与第二次乘法 |
| `tailor_tier` | 裁缝档 → `GoldDistribution` |
| `match.kind` | `closure` 等匹配器类型 |
| `match.station_bonus_efficiency_anchor` | 可露希尔分档纸面加成效率锚点 |
| `unit_gsl_gold_anchor` | 独立赤金消耗锚点 |
| `unit_output` | 正式社区单位产出规则：倍率、固定日产出或分等级日产出 |

## 回归夹具映射

`verify_cmd` 按 `rule_id` 前缀选夹具（**不是** CSV `operators` 列）：

| `rule_id` 前缀 | 夹具函数 |
|------------------------|----------|
| `gsl_witch_*` | `witch_fixture(shortcut_id, level)` |
| `gsl_docus_*` | `docus_fixture(case_id, level)` |
| `case_id contains ling_jie` + `rule_id=none` | `ling_jie_fixture(level)` |
| 其他已接线 closure | `closure_fixture(case_id, level)` |

未接线 case 打印 `skip ... (fixture not wired)`。夹具定义：`infra-cli/src/verify/fixtures.rs`。

## 改 L3 时检查清单

1. `trade_shortcuts.json` 条目与 `id` 一致
2. `shortcut.rs` 匹配条件（尤其互斥与 `classify_witch_room`）
3. `verify/fixtures.rs` 若新族需要新夹具
4. `REGRESSION_CASES.csv` 的 `expect_final_efficiency` / `expect_mechanic_equivalent_efficiency` / `rule_id`
5. `cargo run -p infra-cli -- verify --case <id>`
