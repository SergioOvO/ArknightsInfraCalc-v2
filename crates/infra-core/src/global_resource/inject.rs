use std::collections::HashMap;

use crate::types::RecipeKind;

/// 灵知·精密计算：每名同贸易房谢拉格干员的订单效率/上限增量规则。
/// 控制域只携带系数；最终值在贸易域按房间内 `cc.g.karlan` 计数结算。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KarlanPrecision {
    pub eff_per_karlan: f64,
    pub limit_per_karlan: i32,
}

/// 中枢 `GlobalInject` 汇总：同 `inject_family` 取最高，不同族相加。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalInjectManifest {
    trade_by_family: HashMap<String, f64>,
    manu_all_by_family: HashMap<String, f64>,
    manu_gold_by_family: HashMap<String, f64>,
    manu_battle_record_by_family: HashMap<String, f64>,
    manu_originium_by_family: HashMap<String, f64>,
    trade_tagged: Vec<TaggedTradeInject>,
    manu_tagged: Vec<TaggedManuInject>,
    /// 灵知·精密计算规则（单一中枢，不与自身叠加）。
    karlan_precision: Option<KarlanPrecision>,
    /// 中枢八幡海铃 E2「家族认可」已进驻（叙拉古但书链段 producer；不含贸易站计数）。
    haru_e2_in_control: bool,
    /// 中枢戴菲恩 E2「运筹好手」已进驻（推王组链段 producer）。
    daifeen_e2_in_control: bool,
}

/// 中枢按贸易站实际标签人数结算的延迟注入规则。
///
/// 控制中枢先于贸易站搜索，因此不能在中枢求值时把标签人数冻结为 0。
/// `resolved_count` 只保存当前完整 layout 的展示快照；最终贸易房结算通过
/// [`GlobalInjectManifest::trade_eff_pct_with_scoped_tag_counts`] 分别提供全站与当前房计数。
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedTradeInject {
    pub source_buff_id: String,
    pub family: String,
    pub target_tag: String,
    pub value_per_operator: f64,
    pub resolved_count: u8,
    pub count_scope: TradeTaggedCountScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeTaggedCountScope {
    AllTradeRooms,
    CurrentTradeRoom,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaggedManuInject {
    pub source_buff_id: String,
    pub target_tag: String,
    pub recipe: Option<RecipeKind>,
    pub value: f64,
}

#[derive(Debug, Clone, Copy)]
enum ManuInjectSlot {
    All,
    Gold,
    BattleRecord,
    Originium,
}

impl GlobalInjectManifest {
    pub fn trade_eff_pct(&self) -> f64 {
        self.trade_eff_pct_by(|rule| rule.resolved_count)
    }

    /// 以调用方提供的贸易标签人数结算动态规则；同 `family` 仍取最高，
    /// 不同族相加，与静态 `record_trade` 语义一致。
    pub fn trade_eff_pct_with_tag_counts(&self, counts: &HashMap<String, u8>) -> f64 {
        self.trade_eff_pct_with_scoped_tag_counts(counts, &HashMap::new())
    }

    pub fn trade_eff_pct_with_scoped_tag_counts(
        &self,
        all_trade_counts: &HashMap<String, u8>,
        current_room_counts: &HashMap<String, u8>,
    ) -> f64 {
        self.trade_eff_pct_by(|rule| {
            let counts = match rule.count_scope {
                TradeTaggedCountScope::AllTradeRooms => all_trade_counts,
                TradeTaggedCountScope::CurrentTradeRoom => current_room_counts,
            };
            counts.get(&rule.target_tag).copied().unwrap_or(0)
        })
    }

    fn trade_eff_pct_by(&self, count_of: impl Fn(&TaggedTradeInject) -> u8) -> f64 {
        if self.trade_tagged.is_empty() {
            return self.trade_by_family.values().sum();
        }
        let mut by_family = self.trade_by_family.clone();
        for rule in &self.trade_tagged {
            let value = f64::from(count_of(rule)) * rule.value_per_operator;
            let entry = by_family.entry(rule.family.clone()).or_insert(0.0);
            *entry = entry.max(value);
        }
        by_family.values().sum()
    }

    pub fn trade_global_flat_eff_pct(&self) -> f64 {
        self.trade_by_family
            .get(INJECT_FAMILY_TRADE_GLOBAL_FLAT)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn manu_global_flat_eff_pct(&self) -> f64 {
        let value = self
            .manu_all_by_family
            .get(INJECT_FAMILY_MANU_GLOBAL_ALL)
            .copied()
            .unwrap_or(0.0);
        // The generic control-center effect is +2%. Hoshiguma the
        // Breacher's conditional +3% occupies the same max-of-family slot,
        // but is a cross-operator skill and must remain in displayed output.
        if (value - 2.0).abs() < f64::EPSILON {
            value
        } else {
            0.0
        }
    }

    pub fn manu_eff_for(&self, recipe: RecipeKind) -> f64 {
        let all: f64 = self.manu_all_by_family.values().sum();
        all + match recipe {
            RecipeKind::All => 0.0,
            RecipeKind::Gold => self.manu_gold_by_family.values().sum(),
            RecipeKind::BattleRecord => self.manu_battle_record_by_family.values().sum(),
            RecipeKind::Originium => self.manu_originium_by_family.values().sum(),
        }
    }

    pub fn record_trade(&mut self, family: &str, value: f64) {
        let entry = self
            .trade_by_family
            .entry(family.to_string())
            .or_insert(0.0);
        *entry = entry.max(value);
    }

    pub fn record_trade_tagged(
        &mut self,
        source_buff_id: &str,
        family: &str,
        target_tag: &str,
        value_per_operator: f64,
        resolved_count: u8,
        count_scope: TradeTaggedCountScope,
    ) {
        self.trade_tagged.push(TaggedTradeInject {
            source_buff_id: source_buff_id.to_string(),
            family: family.to_string(),
            target_tag: target_tag.to_string(),
            value_per_operator,
            resolved_count,
            count_scope,
        });
    }

    pub fn trade_tagged(&self) -> &[TaggedTradeInject] {
        &self.trade_tagged
    }

    pub fn has_dynamic_trade_inject(&self) -> bool {
        !self.trade_tagged.is_empty()
    }

    pub(crate) fn same_trade_effects_as(&self, other: &Self) -> bool {
        self.trade_by_family == other.trade_by_family
            && self.trade_tagged == other.trade_tagged
            && self.karlan_precision == other.karlan_precision
            && self.haru_e2_in_control == other.haru_e2_in_control
            && self.daifeen_e2_in_control == other.daifeen_e2_in_control
    }

    pub fn record_manu(&mut self, family: &str, recipe: Option<RecipeKind>, value: f64) {
        let slot = match recipe {
            None | Some(RecipeKind::All) => ManuInjectSlot::All,
            Some(RecipeKind::Gold) => ManuInjectSlot::Gold,
            Some(RecipeKind::BattleRecord) => ManuInjectSlot::BattleRecord,
            Some(RecipeKind::Originium) => ManuInjectSlot::Originium,
        };
        let map = match slot {
            ManuInjectSlot::All => &mut self.manu_all_by_family,
            ManuInjectSlot::Gold => &mut self.manu_gold_by_family,
            ManuInjectSlot::BattleRecord => &mut self.manu_battle_record_by_family,
            ManuInjectSlot::Originium => &mut self.manu_originium_by_family,
        };
        let entry = map.entry(family.to_string()).or_insert(0.0);
        if value >= 0.0 {
            *entry = entry.max(value);
        } else {
            *entry += value;
        }
    }

    pub fn record_manu_tagged(
        &mut self,
        source_buff_id: &str,
        target_tag: &str,
        recipe: Option<RecipeKind>,
        value: f64,
    ) {
        self.manu_tagged.push(TaggedManuInject {
            source_buff_id: source_buff_id.to_string(),
            target_tag: target_tag.to_string(),
            recipe,
            value,
        });
    }

    pub fn manu_tagged(&self) -> &[TaggedManuInject] {
        &self.manu_tagged
    }

    pub(crate) fn same_manufacture_effects_as(&self, other: &Self) -> bool {
        self.manu_all_by_family == other.manu_all_by_family
            && self.manu_gold_by_family == other.manu_gold_by_family
            && self.manu_battle_record_by_family == other.manu_battle_record_by_family
            && self.manu_originium_by_family == other.manu_originium_by_family
            && self.manu_tagged == other.manu_tagged
    }

    /// 记录灵知·精密计算；单一中枢不叠加，取订单上限增益更大的一条。
    pub fn record_karlan_precision(&mut self, eff_per_karlan: f64, limit_per_karlan: i32) {
        let cand = KarlanPrecision {
            eff_per_karlan,
            limit_per_karlan,
        };
        self.karlan_precision = Some(match self.karlan_precision {
            Some(cur) if cur.limit_per_karlan >= limit_per_karlan => cur,
            _ => cand,
        });
    }

    pub fn karlan_precision(&self) -> Option<KarlanPrecision> {
        self.karlan_precision
    }

    /// 叙拉古但书链段：中枢有八幡海铃 E2 时激活；贸易站三人组不全则仍走 `gsl_docus_solo` fallback。
    pub fn record_haru_e2_in_control(&mut self) {
        self.haru_e2_in_control = true;
    }

    pub fn haru_e2_in_control(&self) -> bool {
        self.haru_e2_in_control
    }

    /// 推王组链段：中枢有戴菲恩 E2 时激活。
    pub fn record_daifeen_e2_in_control(&mut self) {
        self.daifeen_e2_in_control = true;
    }

    pub fn daifeen_e2_in_control(&self) -> bool {
        self.daifeen_e2_in_control
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displayed_manu_flat_excludes_generic_two_but_keeps_hoshiguma_three() {
        let mut inject = GlobalInjectManifest::default();
        inject.record_manu(INJECT_FAMILY_MANU_GLOBAL_ALL, None, 2.0);
        assert_eq!(inject.manu_global_flat_eff_pct(), 2.0);

        inject.record_manu(INJECT_FAMILY_MANU_GLOBAL_ALL, None, 3.0);
        assert_eq!(inject.manu_global_flat_eff_pct(), 0.0);
        assert_eq!(inject.manu_eff_for(RecipeKind::Gold), 3.0);
    }

    #[test]
    fn tagged_trade_inject_uses_family_max_and_candidate_counts_without_mutating_snapshot() {
        let mut inject = GlobalInjectManifest::default();
        inject.record_trade("shared", 4.0);
        inject.record_trade_tagged(
            "source_a",
            "shared",
            "tag_a",
            5.0,
            1,
            TradeTaggedCountScope::AllTradeRooms,
        );
        inject.record_trade_tagged(
            "source_b",
            "shared",
            "tag_b",
            4.0,
            2,
            TradeTaggedCountScope::AllTradeRooms,
        );
        inject.record_trade_tagged(
            "source_c",
            "other",
            "tag_c",
            3.0,
            1,
            TradeTaggedCountScope::AllTradeRooms,
        );

        assert_eq!(inject.trade_eff_pct(), 11.0);
        let candidate_counts = HashMap::from([
            ("tag_a".to_string(), 3),
            ("tag_b".to_string(), 0),
            ("tag_c".to_string(), 2),
        ]);
        assert_eq!(
            inject.trade_eff_pct_with_tag_counts(&candidate_counts),
            21.0
        );
        assert_eq!(
            inject.trade_eff_pct(),
            11.0,
            "候选计数只能覆盖本次求值，不能改写 resolved_count 展示快照"
        );
    }

    #[test]
    fn tagged_trade_inject_keeps_all_trade_and_current_room_counts_distinct() {
        let mut inject = GlobalInjectManifest::default();
        inject.record_trade_tagged(
            "haru",
            "siracusa",
            "cc.g.siracusa",
            5.0,
            0,
            TradeTaggedCountScope::AllTradeRooms,
        );
        inject.record_trade_tagged(
            "daifeen",
            "glasgow",
            "cc.g.glasgow",
            10.0,
            0,
            TradeTaggedCountScope::CurrentTradeRoom,
        );
        let totals = HashMap::from([
            ("cc.g.siracusa".to_string(), 2),
            ("cc.g.glasgow".to_string(), 3),
        ]);

        for (glasgow_in_room, expected) in [(3, 40.0), (0, 10.0), (2, 30.0), (1, 20.0)] {
            let room = if glasgow_in_room == 0 {
                HashMap::new()
            } else {
                HashMap::from([("cc.g.glasgow".to_string(), glasgow_in_room)])
            };
            assert_eq!(
                inject.trade_eff_pct_with_scoped_tag_counts(&totals, &room),
                expected
            );
        }
    }
}

/// 默认注入族：全贸易站 flat %（阿米娅/诗怀雅/明椒/阿斯卡纶等）。
pub const INJECT_FAMILY_TRADE_GLOBAL_FLAT: &str = "trade_global_flat";

/// 默认注入族：全制造站 flat %（凯尔希/Mon3tr 等）。
pub const INJECT_FAMILY_MANU_GLOBAL_ALL: &str = "manu_global_all";

/// Per-faction scaling 注入族：每叙拉古干员在贸易站提供 +5%。
pub const INJECT_FAMILY_TRADE_SIRACUSA_SCALING: &str = "trade_siracusa_scaling";

/// Per-faction scaling 注入族：每 3 谢拉格贸易站提供 +10%。
pub const INJECT_FAMILY_TRADE_KARLAN_STATION: &str = "trade_karlan_station";

/// Per-faction scaling 注入族：每格拉斯哥帮干员在贸易站提供 +10%。
pub const INJECT_FAMILY_TRADE_GLASGOW_SCALING: &str = "trade_glasgow_scaling";

/// Per-faction scaling 注入族：每黑钢国际干员在制造站提供 +5%。
pub const INJECT_FAMILY_MANU_BLACKSTEEL_SCALING: &str = "manu_blacksteel_scaling";

/// Per-faction scaling 注入族：每骑士干员在制造站提供 +7%。
pub const INJECT_FAMILY_MANU_KNIGHT_SCALING: &str = "manu_knight_scaling";
