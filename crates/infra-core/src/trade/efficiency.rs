use serde::Serialize;

/// 三级普通赤金贸易站在 100% 效率、24h 下的社区基准龙门币产出。
pub const GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY: f64 = 10_265.0;

/// 纸面效率。内部统一使用无量纲小数，`1.0` 表示基础 100%。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PaperTradeEfficiency {
    pub base_efficiency: f64,
    pub control_bonus: f64,
    pub occupancy_bonus: f64,
    pub operator_skill_bonus: f64,
    pub paper_efficiency: f64,
}

impl PaperTradeEfficiency {
    pub fn from_bonus_pct(
        occupancy_bonus_pct: f64,
        operator_skill_bonus_pct: f64,
        control_bonus_pct: f64,
    ) -> Self {
        let base_efficiency = 1.0;
        let occupancy_bonus = occupancy_bonus_pct / 100.0;
        let operator_skill_bonus = operator_skill_bonus_pct / 100.0;
        let control_bonus = control_bonus_pct / 100.0;
        let paper_efficiency =
            base_efficiency + control_bonus + occupancy_bonus + operator_skill_bonus;
        Self {
            base_efficiency,
            control_bonus,
            occupancy_bonus,
            operator_skill_bonus,
            paper_efficiency,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TradeProductionBasis {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    pub reference_unit_output_per_day: f64,
    pub enhanced_unit_output_per_day: f64,
    pub unit_output_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TradeEfficiency {
    pub paper: PaperTradeEfficiency,
    pub production_basis: TradeProductionBasis,
    /// 可直接用于产出预估和贸易搜索排序的最终效率。
    pub final_efficiency: f64,
    /// 报表展示用：扣除基础、中枢与人头后的等效技能加成（无量纲）。
    pub equivalent_operator_skill_bonus: f64,
}

impl TradeEfficiency {
    pub fn new(
        paper: PaperTradeEfficiency,
        reference_unit_output_per_day: f64,
        enhanced_unit_output_per_day: f64,
        rule_id: Option<String>,
        apply_paper_efficiency: bool,
    ) -> Self {
        let unit_output_multiplier = if reference_unit_output_per_day > 0.0 {
            enhanced_unit_output_per_day / reference_unit_output_per_day
        } else {
            1.0
        };
        let paper_factor = if apply_paper_efficiency {
            paper.paper_efficiency
        } else {
            1.0
        };
        let final_efficiency = paper_factor * unit_output_multiplier;
        let non_skill_efficiency =
            paper.base_efficiency + paper.control_bonus + paper.occupancy_bonus;
        let equivalent_operator_skill_bonus = final_efficiency - non_skill_efficiency;
        Self {
            paper,
            production_basis: TradeProductionBasis {
                rule_id,
                reference_unit_output_per_day,
                enhanced_unit_output_per_day,
                unit_output_multiplier,
            },
            final_efficiency,
            equivalent_operator_skill_bonus,
        }
    }

    pub fn final_efficiency_pct(&self) -> f64 {
        self.final_efficiency * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_efficiency_includes_base_control_occupancy_and_skill() {
        let paper = PaperTradeEfficiency::from_bonus_pct(3.0, 13.0, 7.0);
        assert!((paper.paper_efficiency - 1.23).abs() < f64::EPSILON);
    }

    #[test]
    fn docus_total_efficiency_multiplies_the_whole_room() {
        let paper = PaperTradeEfficiency::from_bonus_pct(3.0, 13.0, 7.0);
        let efficiency = TradeEfficiency::new(
            paper,
            GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY,
            GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY * 1.55,
            Some("gsl_docus_solo".to_string()),
            true,
        );
        assert!((efficiency.final_efficiency - 1.9065).abs() < 1e-9);
        assert!((efficiency.final_efficiency_pct() - 190.65).abs() < 1e-9);
    }
}
