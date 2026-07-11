use serde::Serialize;

use crate::efficiency::Efficiency;

/// 三级普通赤金贸易站在 100% 效率、24h 下的社区基准龙门币产出。
pub const GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY: f64 = 10_265.0;

/// 纸面效率。内部统一使用无量纲小数，`1.0` 表示基础 100%。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PaperTradeEfficiency {
    pub base_efficiency: Efficiency,
    pub control_efficiency: Efficiency,
    pub occupancy_efficiency: Efficiency,
    pub skill_efficiency: Efficiency,
    pub paper_efficiency: Efficiency,
}

impl PaperTradeEfficiency {
    pub(crate) fn from_percent_points(
        occupancy_bonus_pct: f64,
        operator_skill_bonus_pct: f64,
        control_bonus_pct: f64,
    ) -> Self {
        let base_efficiency = Efficiency::ONE;
        let occupancy_efficiency = Efficiency::from_percent_points(occupancy_bonus_pct);
        let skill_efficiency = Efficiency::from_percent_points(operator_skill_bonus_pct);
        let control_efficiency = Efficiency::from_percent_points(control_bonus_pct);
        let paper_efficiency =
            base_efficiency + control_efficiency + occupancy_efficiency + skill_efficiency;
        Self {
            base_efficiency,
            control_efficiency,
            occupancy_efficiency,
            skill_efficiency,
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
    pub unit_output_multiplier: Efficiency,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TradeEfficiency {
    pub paper: PaperTradeEfficiency,
    pub production_basis: TradeProductionBasis,
    /// 可直接用于产出预估和贸易搜索排序的最终效率。
    pub final_efficiency: Efficiency,
    /// 报表展示用：扣除基础、中枢与人头后的等效技能加成（无量纲）。
    pub equivalent_skill_efficiency: Efficiency,
    pub applies_paper_efficiency: bool,
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
            Efficiency::from_decimal(enhanced_unit_output_per_day / reference_unit_output_per_day)
        } else {
            Efficiency::ONE
        };
        let paper_factor = if apply_paper_efficiency {
            paper.paper_efficiency
        } else {
            Efficiency::ONE
        };
        let final_efficiency = paper_factor * unit_output_multiplier;
        let non_skill_efficiency =
            paper.base_efficiency + paper.control_efficiency + paper.occupancy_efficiency;
        let equivalent_skill_efficiency = final_efficiency - non_skill_efficiency;
        Self {
            paper,
            production_basis: TradeProductionBasis {
                rule_id,
                reference_unit_output_per_day,
                enhanced_unit_output_per_day,
                unit_output_multiplier,
            },
            final_efficiency,
            equivalent_skill_efficiency,
            applies_paper_efficiency: apply_paper_efficiency,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_efficiency_includes_base_control_occupancy_and_skill() {
        let paper = PaperTradeEfficiency::from_percent_points(3.0, 13.0, 7.0);
        assert_eq!(paper.paper_efficiency, Efficiency::from_decimal(1.23));
    }

    #[test]
    fn docus_total_efficiency_multiplies_the_whole_room() {
        let paper = PaperTradeEfficiency::from_percent_points(3.0, 13.0, 7.0);
        let efficiency = TradeEfficiency::new(
            paper,
            GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY,
            GOLD_TRADE_REFERENCE_OUTPUT_PER_DAY * 1.55,
            Some("gsl_docus_solo".to_string()),
            true,
        );
        assert_eq!(efficiency.final_efficiency, Efficiency::from_decimal(1.907));
    }
}
