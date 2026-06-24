use std::collections::HashSet;
use std::time::Instant;

use rayon::prelude::*;
use serde::Serialize;

use crate::control::{solve_control, ControlOperator, ControlRoomInput};
use crate::error::Result;
use crate::global_resource::GlobalResourceKey;
use crate::layout::LayoutContext;
use crate::pool::{combinations_indices, ControlPool};
use crate::scoring::{placeholder_trade_manu_balance, TradeManuBalanceInput};
use crate::skill_table::SkillTable;
use crate::types::RecipeKind;

/// 木天蓼 consumer：贸易/制造侧的泰拉大陆调查团。
pub const MATATABI_CONSUMER_NAME: &str = "泰拉大陆调查团";

/// 中枢评分的完整分解。
///
/// 当前中枢评分公式（`Efficiency` 策略，待公孙贸易-制造平衡公式替换）：
/// ```text
/// score = trade_inject + manu_gold_inject + manu_br_inject
/// ```
/// 这是历史注入口径：三项均为效率百分比，但跨贸易/制造的最终平衡口径
/// 等待公孙公式确认。vpower、木天蓼、心情扣分不在中枢评分里预支——
/// 它们分别在制造 resolve、调查团 consumer、轮换层体现。
#[derive(Debug, Clone, Serialize, Default)]
pub struct ControlScoreBreakdown {
    /// 贸易效率注入%
    pub trade_inject_pct: f64,
    /// 赤金制造效率注入%
    pub manu_gold_inject_pct: f64,
    /// 经验书制造效率注入%
    pub manu_br_inject_pct: f64,
    /// 历史注入子总分 = trade + gold + br；待公孙平衡公式替换。
    pub inject_subtotal: f64,
    /// 虚拟发电站数
    pub virtual_power: f64,
    /// vpower × 2.0
    pub virtual_power_score: f64,
    /// 木天蓼点数
    pub matatabi: f64,
    /// 5.0 + matatabi × 3.0（仅调查团在岗时生效）
    pub matatabi_score: f64,
    /// 本班是否有调查团 consumer
    pub matatabi_consumer_active: bool,
    /// 心情消耗总和 (>0 的部分)
    pub mood_penalty: f64,
    /// -mood × 2.0（HrAndMood 策略时为 ×3.0）
    pub mood_penalty_score: f64,
    /// HrAndMood 策略固定加分（公招/心情技能）
    pub ancillary_score: f64,
    /// 搜索排序分；`Efficiency` 为 `inject_subtotal`，`HrAndMood` 为 `ancillary_score`。
    pub total_score: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ControlSearchHit {
    pub names: Vec<String>,
    pub score: f64,
    pub trade_inject_pct: f64,
    pub manu_gold_inject_pct: f64,
    /// 评分明细分解
    pub breakdown: ControlScoreBreakdown,
}

/// 中枢补位策略：`base_systems` 钉死后剩余席位按公孙「公招 + 心情」填，而非热情贸易链。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlFillPolicy {
    #[default]
    Efficiency,
    HrAndMood,
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
        || matches!(
            buff_id,
            "control_token_tra_spd[000]"
                | "control_prod_tra_spd[000]"
                | "control_tra_limit&spd[010]"
        )
}

fn control_mood_cost_buff(buff_id: &str) -> bool {
    buff_id == "control_mp_psk[000]"
        || (buff_id.starts_with("control_mp_cost[") && !buff_id.contains('&'))
        || buff_id.starts_with("control_mp_cost&faction")
}

fn control_hr_mood_buff(buff_id: &str) -> bool {
    matches!(
        buff_id,
        "control_hire_spd&bd[000]"
            | "control_dorm_rec2[000]"
            | "control_mp_cost[007]"
            | "control_mp_cost[010]"
            | "control_mp_cost[012]"
            | "control_mp_psk[000]"
    ) || (buff_id.starts_with("control_mp_cost[") && !buff_id.contains('&'))
        || buff_id.starts_with("control_mp_cost&faction")
}

fn control_inject_sort_key(hit: &ControlSearchHit) -> f64 {
    hit.score
}

pub fn control_entry_core_inject_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    entry
        .buff_ids
        .iter()
        .any(|b| control_efficiency_inject_buff(b))
}

pub fn control_entry_mood_cost_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry
        .buff_ids
        .iter()
        .any(|b| control_standalone_producer_buff(b))
    {
        return false;
    }
    entry.buff_ids.iter().any(|b| control_mood_cost_buff(b))
}

pub fn control_entry_hr_mood_fill(entry: &crate::pool::ControlPoolEntry) -> bool {
    if entry
        .buff_ids
        .iter()
        .any(|b| control_standalone_producer_buff(b) || control_efficiency_inject_buff(b))
    {
        return false;
    }
    entry.buff_ids.iter().any(|b| control_hr_mood_buff(b))
}

/// 公招 / 心情类中枢技能（`atoms: []` 挡池条目）的补位加分。
fn control_hr_mood_ancillary(operators: &[ControlOperator], table: &SkillTable) -> f64 {
    let mut score = 0.0;
    for op in operators {
        for bid in &op.buff_ids {
            score += match bid.as_str() {
                // 八幡海铃·可靠伙伴：人脉联络 +10%（skill_table 仅建模热情，补位单独计分）
                "control_hire_spd&bd[000]" => 10.0,
                // 中枢内全员心情 +0.05/h
                "control_mp_cost[007]"
                | "control_mp_cost[010]"
                | "control_mp_cost[012]"
                | "control_mp_psk[000]" => 5.0,
                // 宿舍全员心情 +0.05/h（低于中枢内恢复）
                "control_dorm_rec2[000]" => 2.0,
                _ => {
                    let Some(skill) = table.get(bid) else {
                        continue;
                    };
                    if skill.facility != "control" || !skill.atoms.is_empty() {
                        continue;
                    }
                    if bid.starts_with("control_mp_cost[") && !bid.contains('&') {
                        5.0
                    } else if bid.starts_with("control_mp_cost&faction") {
                        5.0
                    } else {
                        0.0
                    }
                }
            };
        }
    }
    score
}

/// 中枢搜索评分：当前为贸易/制造注入%之和。
///
/// TODO(scoring): 用公孙长乐贸易-制造平衡公式替换裸加的
/// `trade_inject + manu_gold + manu_br`。在公式落地前保持现有行为，
/// 只显式化排序口径。
///
/// vpower、木天蓼、心情扣分不在此评分——它们分别在制造站 resolve、调查团 consumer、
/// 轮换层心情管理里体现。
///
/// 返回完整的评分明细分解；`.total_score` 即搜索排序分。
fn score_control_result(
    result: &crate::control::ControlCenterResult,
    operators: &[ControlOperator],
    table: &SkillTable,
    options: &ControlSearchOptions,
) -> ControlScoreBreakdown {
    let mood_penalty: f64 = result
        .operator_mood_drains
        .values()
        .filter(|v| **v > 0.0)
        .sum();

    if options.fill_policy == ControlFillPolicy::HrAndMood {
        let ancillary = control_hr_mood_ancillary(operators, table);
        return ControlScoreBreakdown {
            ancillary_score: ancillary,
            mood_penalty,
            mood_penalty_score: 0.0,
            total_score: ancillary,
            ..ControlScoreBreakdown::default()
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
    let balanced = placeholder_trade_manu_balance(TradeManuBalanceInput {
        trade_eff_pct: trade_inject,
        gold_manu_eff_pct: manu_gold,
        battle_record_manu_eff_pct: manu_br,
        trade_station_count: options.layout.trade_station_count,
        gold_line_count,
        battle_record_line_count,
    });
    let inject_subtotal = balanced.composite_eff_pct;

    let matatabi = result.global.get(GlobalResourceKey::Matatabi);
    let virtual_power = result.global.get(GlobalResourceKey::VirtualPower);

    ControlScoreBreakdown {
        trade_inject_pct: trade_inject,
        manu_gold_inject_pct: manu_gold,
        manu_br_inject_pct: manu_br,
        inject_subtotal,
        virtual_power,
        matatabi,
        matatabi_consumer_active: options.matatabi_consumer_active,
        mood_penalty,
        total_score: inject_subtotal,
        ..ControlScoreBreakdown::default()
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
            let breakdown = score_control_result(&result, &operators, table, options);
            Some(ControlSearchHit {
                score: breakdown.total_score,
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
    fn control_search_score_uses_total_score_sort_key() {
        let hit = ControlSearchHit {
            names: vec!["a".into()],
            score: 12.0,
            trade_inject_pct: 7.0,
            manu_gold_inject_pct: 5.0,
            breakdown: ControlScoreBreakdown {
                total_score: 12.0,
                inject_subtotal: 12.0,
                ..Default::default()
            },
        };
        assert_eq!(control_inject_sort_key(&hit), hit.score);
        assert_eq!(hit.score, hit.breakdown.total_score);
    }

    #[test]
    fn control_score_is_pure_inject_sum() {
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let (result, ops) = monhun_control_ops(&table);
        assert!(result.global.get(GlobalResourceKey::Matatabi) > 0.0);

        let without = score_control_result(
            &result,
            &ops,
            &table,
            &ControlSearchOptions {
                matatabi_consumer_active: false,
                ..Default::default()
            },
        );
        let with = score_control_result(
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
            (with.total_score - without.total_score).abs() < 0.001,
            "matatabi 不应影响中枢注入评分: with={} without={}",
            with.total_score,
            without.total_score,
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
}
