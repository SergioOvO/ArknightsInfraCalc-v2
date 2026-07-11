use std::collections::HashSet;
use std::time::Instant;

use rayon::prelude::*;
use serde::Serialize;

use crate::control::{solve_control, ControlOperator, ControlRoomInput};
use crate::error::Result;
use crate::global_resource::GlobalResourceKey;
use crate::layout::LayoutContext;
use crate::pool::{combinations_indices, ControlPool};
use crate::scoring::{
    evaluate_control_inject_policy, ScoringPolicyId, TradeManuEfficiencyComponents,
};
use crate::skill_table::SkillTable;
use crate::types::RecipeKind;

/// 木天蓼 consumer：贸易/制造侧的泰拉大陆调查团。
pub const MATATABI_CONSUMER_NAME: &str = "泰拉大陆调查团";

/// 中枢具名排序 policy 的完整分解。
///
/// 当前中枢排序策略（`ControlInjectRawSumV0`）：
/// ```text
/// sort_key = trade_inject + manu_gold_inject + manu_br_inject
/// ```
/// 这是局部注入排序策略：三项均为可解释效率百分比分量。它不是
/// 贸易/制造平衡公式；vpower、木天蓼、心情扣分也不在中枢 policy 里预支，
/// 而是分别在制造 resolve、调查团 consumer、轮换层体现。
#[derive(Debug, Clone, Serialize, Default)]
pub struct ControlPolicyBreakdown {
    /// 具名局部排序策略；不是生产效率。
    pub policy: ScoringPolicyId,
    /// 贸易效率注入%
    pub trade_inject_pct: f64,
    /// 赤金制造效率注入%
    pub manu_gold_inject_pct: f64,
    /// 经验书制造效率注入%
    pub manu_br_inject_pct: f64,
    /// 当前注入排序 key = trade + gold + br；仅用于中枢候选排序。
    pub inject_subtotal: f64,
    /// 虚拟发电站数
    pub virtual_power: f64,
    /// 木天蓼点数
    pub matatabi: f64,
    /// 本班是否有调查团 consumer
    pub matatabi_consumer_active: bool,
    /// 心情消耗总和 (>0 的部分)
    pub mood_penalty: f64,
    /// 体系外散件排序分量（单走八幡海铃等；与 +7/+2 同层比较）。
    pub loose_piece_sort_component: f64,
    /// 心情补位排序分量（EW / 玛恩纳等；低于效率散件）。
    pub mood_fill_sort_component: f64,
    /// 线索补位排序分量（低于心情）。
    pub clue_fill_sort_component: f64,
    /// policy 排序键；中枢补位按 组合体系(pinned) → 散件 → 心情 → 线索。
    pub policy_sort_key: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlSearchHit {
    pub names: Vec<String>,
    pub trade_inject_pct: f64,
    pub manu_gold_inject_pct: f64,
    /// 具名 policy 明细
    pub breakdown: ControlPolicyBreakdown,
}

/// 中枢补位策略：`base_systems` 钉死组合体系后，剩余席位按
/// 散件（单走八幡海铃 / +7 / +2）→ 心情 → 线索填，而非热情贸易链。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlFillPolicy {
    #[default]
    InjectOnly,
    /// 公孙分层补位：注入散件 → 心情 → 线索。
    LayeredFill,
}

#[derive(Debug, Clone)]
pub struct ControlSearchOptions {
    pub max_operators: u8,
    pub top_k: usize,
    pub mood: f64,
    pub layout: LayoutContext,
    /// 本班编制是否已有调查团在贸易/制造上岗；无 consumer 时木天蓼不计正分。
    pub matatabi_consumer_active: bool,
    /// 组合必须包含这些干员（如 `base_systems` 已钉死的中枢位）。
    pub must_include: HashSet<String>,
    pub fill_policy: ControlFillPolicy,
}

impl Default for ControlSearchOptions {
    fn default() -> Self {
        Self {
            max_operators: 5,
            top_k: 20,
            mood: 24.0,
            layout: LayoutContext::default(),
            matatabi_consumer_active: false,
            must_include: HashSet::new(),
            fill_policy: ControlFillPolicy::default(),
        }
    }
}

/// 热情/MyGO 经济链 buff：补位阶段排除，避免占满剩余中枢位。
pub fn control_passion_chain_buff(buff_id: &str) -> bool {
    matches!(
        buff_id,
        "control_dorm_bd[000]"
            | "control_mp_bd&trade[000]"
            | "control_prod_bd_spd[000]"
            | "control_prod_bd_spd[010]"
    )
}

fn control_resource_producer_buff(buff_id: &str) -> bool {
    matches!(
        buff_id,
        "control_mp_bd[000]"
            | "control_mp_bd[010]"
            | "control_mp_bd2[000]"
            | "control_mp_cost&bd2[010]"
    )
}

fn control_standalone_producer_buff(buff_id: &str) -> bool {
    control_passion_chain_buff(buff_id) || control_resource_producer_buff(buff_id)
}

fn control_efficiency_inject_buff(buff_id: &str) -> bool {
    buff_id.starts_with("control_prod_spd")
        || buff_id.starts_with("control_tra_spd")
        || buff_id.starts_with("control_token_prod_spd")
        || buff_id.starts_with("control_tra_limit&spd")
        || buff_id.starts_with("control_bd_spd")
        || matches!(
            buff_id,
            "control_token_tra_spd[000]" | "control_prod_tra_spd[000]"
        )
}

pub fn control_efficiency_fill_sort_weight(entry: &crate::pool::ControlPoolEntry) -> f64 {
    let mut weight = 0.0;
    for bid in &entry.buff_ids {
        if bid.starts_with("control_tra_spd") || bid == "control_token_tra_spd[000]" {
            weight += 100.0;
        } else if bid.starts_with("control_prod_spd")
            || bid.starts_with("control_token_prod_spd")
            || bid == "control_bd_spd[000]"
        {
            weight += 80.0;
        } else if bid.starts_with("control_tra_limit&spd") || bid == "control_prod_tra_spd[000]" {
            weight += 60.0;
        } else if bid == "control_hire_spd&bd[000]" {
            weight += 10.0;
        }
    }
    weight
}

fn control_mood_cost_buff(buff_id: &str) -> bool {
    buff_id == "control_mp_psk[000]"
        || buff_id == "control_mp_lonely[000]"
        || buff_id == "control_mp_expand_double[000]"
        || (buff_id.starts_with("control_mp_cost[") && !buff_id.contains('&'))
        || buff_id.starts_with("control_mp_cost&faction")
}

fn control_loose_piece_buff(buff_id: &str) -> bool {
    matches!(
        buff_id,
        "control_hire_spd&bd[000]" | "control_tra_limit&spd[000]"
    )
}

fn control_clue_buff(_buff_id: &str) -> bool {
    false
}

fn control_layered_fill_buff(buff_id: &str) -> bool {
    control_loose_piece_buff(buff_id)
        || control_mood_cost_buff(buff_id)
        || control_clue_buff(buff_id)
}

fn control_inject_sort_key(hit: &ControlSearchHit) -> f64 {
    hit.breakdown.policy_sort_key
}

pub fn control_entry_core_inject_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry.name == "琴柳" {
        return false;
    }
    entry
        .buff_ids
        .iter()
        .any(|b| control_efficiency_inject_buff(b))
}

pub fn control_entry_mood_cost_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry.name == "琴柳" {
        return false;
    }
    if entry
        .buff_ids
        .iter()
        .any(|b| control_standalone_producer_buff(b))
    {
        return false;
    }
    entry.buff_ids.iter().any(|b| control_mood_cost_buff(b))
}

pub fn control_entry_plugin_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry.name == "琴柳" {
        return false;
    }
    if entry
        .buff_ids
        .iter()
        .any(|b| control_standalone_producer_buff(b))
    {
        return false;
    }
    control_entry_core_inject_fill(entry) || control_entry_layered_fill(entry)
}

pub fn control_entry_layered_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry.name == "琴柳" {
        return false;
    }
    if entry
        .buff_ids
        .iter()
        .any(|b| control_standalone_producer_buff(b) || control_efficiency_inject_buff(b))
    {
        return false;
    }
    entry.buff_ids.iter().any(|b| control_layered_fill_buff(b))
}

#[derive(Debug, Clone, Copy, Default)]
struct ControlFillTierScores {
    loose_piece: f64,
    mood: f64,
    clue: f64,
}

impl ControlFillTierScores {
    fn layered_sort_bonus(self) -> f64 {
        // 散件与 +7/+2 同层按数值比较；心情和线索只在高层同分时补空。
        self.loose_piece + self.mood * 0.01 + self.clue * 0.0001
    }
}

/// 体系外中枢补位分层：散件（单走八幡海铃）→ 心情（EW / 玛恩纳）→ 线索。
fn control_layered_fill_scores(
    operators: &[ControlOperator],
    table: &SkillTable,
) -> ControlFillTierScores {
    let mut scores = ControlFillTierScores::default();
    for op in operators {
        for bid in &op.buff_ids {
            match bid.as_str() {
                // 喀兰贸易路线入口：单独放中枢时本身不产数值，但会解锁孑/银灰路线。
                "control_tra_limit&spd[000]" => scores.loose_piece += 8.0,
                // 八幡海铃·可靠伙伴：单走时按散件层处理，但低于直接 +2/+7 产能。
                "control_hire_spd&bd[000]" => scores.loose_piece += 1.0,
                // 中枢内全员心情 +0.05/h；EW / 玛恩纳等也归心情层。
                "control_mp_cost[007]"
                | "control_mp_cost[010]"
                | "control_mp_cost[012]"
                | "control_mp_psk[000]"
                | "control_mp_lonely[000]"
                | "control_mp_expand_double[000]" => scores.mood += 5.0,
                // 宿舍全员心情 +0.05/h（低于中枢内恢复）
                "control_dorm_rec2[000]" => scores.mood += 2.0,
                _ => {
                    let Some(skill) = table.get(bid) else {
                        continue;
                    };
                    if skill.facility != "control" || !skill.atoms.is_empty() {
                        continue;
                    }
                    if bid.starts_with("control_mp_cost[") && !bid.contains('&') {
                        scores.mood += 5.0;
                    } else if control_clue_buff(bid) {
                        scores.clue += 1.0;
                    }
                }
            };
        }
    }
    scores
}

/// 中枢搜索排序：当前为贸易/制造注入%之和。
///
/// 公孙长乐口径下不需要跨贸易/制造平衡公式；贸易站赤金订单总效率、
/// 制造赤金效率、制造经验效率作为分量分别解释。这里保留裸加只作为
/// 当前中枢候选的局部排序策略，并通过 `scoring` 命名入口显式标记。
///
/// vpower、木天蓼、心情扣分不在此评分——它们分别在制造站 resolve、调查团 consumer、
/// 轮换层心情管理里体现。
///
/// 返回完整的 policy 分解；`.policy_sort_key` 只用于中枢候选排序。
fn evaluate_control_policy(
    result: &crate::control::ControlCenterResult,
    operators: &[ControlOperator],
    table: &SkillTable,
    options: &ControlSearchOptions,
) -> ControlPolicyBreakdown {
    let mood_penalty: f64 = result
        .operator_mood_drains
        .values()
        .filter(|v| **v > 0.0)
        .sum();

    if options.fill_policy == ControlFillPolicy::LayeredFill {
        let fill_scores = control_layered_fill_scores(operators, table);
        let trade_inject = result.inject.trade_eff_pct();
        let manu_gold = result.inject.manu_eff_for(RecipeKind::Gold);
        let manu_br = result.inject.manu_eff_for(RecipeKind::BattleRecord);
        let component_score = evaluate_control_inject_policy(TradeManuEfficiencyComponents {
            trade_eff_pct: trade_inject,
            gold_manu_eff_pct: manu_gold,
            battle_record_manu_eff_pct: manu_br,
            trade_station_count: options.layout.trade_station_count,
            gold_line_count: options.layout.gold_manu_line_count.min(u32::from(u8::MAX)) as u8,
            battle_record_line_count: options
                .layout
                .manufacture_station_count
                .saturating_sub(options.layout.gold_manu_line_count.min(u32::from(u8::MAX)) as u8),
        });
        return ControlPolicyBreakdown {
            policy: component_score.policy,
            trade_inject_pct: trade_inject,
            manu_gold_inject_pct: manu_gold,
            manu_br_inject_pct: manu_br,
            inject_subtotal: component_score.sort_key_pct,
            loose_piece_sort_component: fill_scores.loose_piece,
            mood_fill_sort_component: fill_scores.mood,
            clue_fill_sort_component: fill_scores.clue,
            mood_penalty,
            policy_sort_key: component_score.sort_key_pct + fill_scores.layered_sort_bonus(),
            ..ControlPolicyBreakdown::default()
        };
    }

    let trade_inject = result.inject.trade_eff_pct();
    let manu_gold = result.inject.manu_eff_for(RecipeKind::Gold);
    let manu_br = result.inject.manu_eff_for(RecipeKind::BattleRecord);
    let gold_line_count = options.layout.gold_manu_line_count.min(u32::from(u8::MAX)) as u8;
    let battle_record_line_count = options
        .layout
        .manufacture_station_count
        .saturating_sub(gold_line_count);
    let component_score = evaluate_control_inject_policy(TradeManuEfficiencyComponents {
        trade_eff_pct: trade_inject,
        gold_manu_eff_pct: manu_gold,
        battle_record_manu_eff_pct: manu_br,
        trade_station_count: options.layout.trade_station_count,
        gold_line_count,
        battle_record_line_count,
    });
    let inject_subtotal = component_score.sort_key_pct;

    let matatabi = result.global.get(GlobalResourceKey::Matatabi);
    let virtual_power = result.global.get(GlobalResourceKey::VirtualPower);

    ControlPolicyBreakdown {
        policy: component_score.policy,
        trade_inject_pct: trade_inject,
        manu_gold_inject_pct: manu_gold,
        manu_br_inject_pct: manu_br,
        inject_subtotal,
        virtual_power,
        matatabi,
        matatabi_consumer_active: options.matatabi_consumer_active,
        mood_penalty,
        policy_sort_key: inject_subtotal,
        ..ControlPolicyBreakdown::default()
    }
}

/// 中枢 C(n,k)，k ∈ [1, max_operators]；按全局注入与资源池评分。
pub fn search_control_combos(
    pool: &ControlPool,
    table: &SkillTable,
    options: &ControlSearchOptions,
) -> Result<Vec<ControlSearchHit>> {
    let n = pool.entries.len();
    if n == 0 {
        return Ok(Vec::new());
    }

    let start = Instant::now();
    let max_k = options.max_operators.min(5).min(n as u8) as usize;
    let mut combos: Vec<Vec<usize>> = Vec::new();
    for k in 1..=max_k {
        combos.extend(combinations_indices(n, k));
    }

    let layout = options.layout.clone();
    let mood = options.mood;

    let mut hits: Vec<ControlSearchHit> = combos
        .par_iter()
        .filter_map(|idxs| {
            let operators: Vec<_> = idxs
                .iter()
                .map(|i| pool.entries[*i].to_control_operator())
                .collect();
            let mut names: Vec<String> = operators.iter().map(|o| o.name.clone()).collect();
            names.sort();
            let input = ControlRoomInput {
                operators: operators.clone(),
                mood,
                layout: layout.clone(),
            };
            let result = solve_control(&input, table);
            let breakdown = evaluate_control_policy(&result, &operators, table, options);
            Some(ControlSearchHit {
                trade_inject_pct: result.inject.trade_eff_pct(),
                manu_gold_inject_pct: result.inject.manu_eff_for(RecipeKind::Gold),
                names,
                breakdown,
            })
        })
        .collect();

    if !options.must_include.is_empty() {
        hits.retain(|h| {
            options
                .must_include
                .iter()
                .all(|name| h.names.contains(name))
        });
    }

    hits.sort_by(|a, b| {
        control_inject_sort_key(b)
            .partial_cmp(&control_inject_sort_key(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.names.cmp(&b.names))
    });
    hits.truncate(options.top_k);
    let _elapsed = start.elapsed();
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::{ControlOperator, ControlRoomInput};
    use crate::instances::{default_instances_path, OperatorInstances};
    use crate::pool::build_control_pool;
    use crate::roster::Roster;
    use crate::skill_table::{default_skill_table_path, SkillTable};

    fn monhun_control_ops(
        table: &SkillTable,
    ) -> (crate::control::ControlCenterResult, Vec<ControlOperator>) {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let roster = Roster::from_elite_map(
            [("火龙S黑角", 2), ("麒麟R夜刀", 2)]
                .into_iter()
                .map(|(n, e)| (n.to_string(), e))
                .collect(),
        );
        let pool = build_control_pool(&roster, &instances, table).unwrap();
        let ops: Vec<ControlOperator> = ["火龙S黑角", "麒麟R夜刀"]
            .iter()
            .map(|n| pool.entry(n).unwrap().to_control_operator())
            .collect();
        let result = solve_control(
            &ControlRoomInput {
                operators: ops.clone(),
                mood: 24.0,
                layout: LayoutContext::default(),
            },
            table,
        );
        (result, ops)
    }

    #[test]
    fn control_search_uses_named_policy_sort_key() {
        let hit = ControlSearchHit {
            names: vec!["a".into()],
            trade_inject_pct: 7.0,
            manu_gold_inject_pct: 5.0,
            breakdown: ControlPolicyBreakdown {
                policy: ScoringPolicyId::ControlInjectRawSumV0,
                policy_sort_key: 12.0,
                inject_subtotal: 12.0,
                ..Default::default()
            },
        };
        assert_eq!(control_inject_sort_key(&hit), 12.0);
        assert_eq!(hit.breakdown.policy, ScoringPolicyId::ControlInjectRawSumV0);
        let json = serde_json::to_value(&hit).unwrap();
        assert_eq!(
            json["breakdown"]["policy"],
            serde_json::json!("ControlInjectRawSumV0")
        );
        assert_eq!(json["breakdown"]["policy_sort_key"], 12.0);
        assert!(json.get("score").is_none());
        assert!(json["breakdown"].get("total_score").is_none());
    }

    #[test]
    fn control_score_is_pure_inject_sum() {
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let (result, ops) = monhun_control_ops(&table);
        assert!(result.global.get(GlobalResourceKey::Matatabi) > 0.0);

        let without = evaluate_control_policy(
            &result,
            &ops,
            &table,
            &ControlSearchOptions {
                matatabi_consumer_active: false,
                ..Default::default()
            },
        );
        let with = evaluate_control_policy(
            &result,
            &ops,
            &table,
            &ControlSearchOptions {
                matatabi_consumer_active: true,
                ..Default::default()
            },
        );
        // 纯注入评分：matatabi consumer 不影响 score（木天蓼不在中枢评分里预支）
        assert!(
            (with.policy_sort_key - without.policy_sort_key).abs() < 0.001,
            "matatabi 不应影响中枢注入评分: with={} without={}",
            with.policy_sort_key,
            without.policy_sort_key,
        );
        assert!(with.matatabi > 0.0, "matatabi 仍应在 breakdown 中记录");
        assert!(
            result
                .operator_mood_drains
                .get("麒麟R夜刀")
                .copied()
                .unwrap_or(0.0)
                > 0.0,
            "夜刀应记账心情消耗"
        );
    }

    #[test]
    fn control_plugin_fill_excludes_unclaimed_system_producers() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let roster = Roster::from_elite_map(
            [
                ("阿米娅", 2),
                ("诗怀雅", 2),
                ("八幡海铃", 2),
                ("焰尾", 2),
                ("薇薇安娜", 2),
                ("三角初华", 2),
                ("若叶睦", 2),
                ("丰川祥子", 2),
            ]
            .into_iter()
            .map(|(n, e)| (n.to_string(), e))
            .collect(),
        );
        let pool = build_control_pool(&roster, &instances, &table).unwrap();

        for name in ["阿米娅", "诗怀雅", "八幡海铃", "焰尾", "薇薇安娜"] {
            let entry = pool.entry(name).unwrap();
            assert!(
                control_entry_plugin_fill(entry),
                "{name} should be allowed as control plugin fill"
            );
        }

        for name in ["三角初华", "若叶睦", "丰川祥子"] {
            let entry = pool.entry(name).unwrap();
            assert!(
                !control_entry_plugin_fill(entry),
                "{name} is a system producer and should not be inserted as standalone control fill"
            );
        }
    }

    #[test]
    fn control_layered_fill_prefers_efficiency_piece_over_mood() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let roster = Roster::from_elite_map(
            [("Mon3tr", 2), ("玛恩纳", 2)]
                .into_iter()
                .map(|(n, e)| (n.to_string(), e))
                .collect(),
        );
        let pool = build_control_pool(&roster, &instances, &table).unwrap();
        assert!(control_entry_plugin_fill(pool.entry("Mon3tr").unwrap()));
        assert!(control_entry_plugin_fill(pool.entry("玛恩纳").unwrap()));

        let hits = search_control_combos(
            &pool,
            &table,
            &ControlSearchOptions {
                max_operators: 1,
                top_k: 1,
                fill_policy: ControlFillPolicy::LayeredFill,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(hits[0].names, vec!["Mon3tr".to_string()]);
        assert!(
            hits[0].breakdown.inject_subtotal > hits[0].breakdown.mood_fill_sort_component * 0.01,
            "efficiency pieces should outrank pure mood fill: {:?}",
            hits[0].breakdown
        );
    }

    #[test]
    fn control_layered_fill_allows_haru_as_loose_piece_but_not_resource_producer() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let roster = Roster::from_elite_map(
            [("八幡海铃", 0), ("三角初华", 2)]
                .into_iter()
                .map(|(n, e)| (n.to_string(), e))
                .collect(),
        );
        let pool = build_control_pool(&roster, &instances, &table).unwrap();

        assert!(control_entry_plugin_fill(pool.entry("八幡海铃").unwrap()));
        assert!(!control_entry_plugin_fill(pool.entry("三角初华").unwrap()));
    }
}
