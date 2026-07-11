use serde::Serialize;

use super::PolicyEvaluation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ScoringPolicyId {
    /// Current central-control fill policy: sort by the raw sum of trade and
    /// manufacture inject percentages.
    ///
    /// This is a local sorting heuristic for central-control candidates, not a
    /// trade/manufacture balance formula.
    ControlInjectRawSumV0,
}

impl Default for ScoringPolicyId {
    fn default() -> Self {
        Self::ControlInjectRawSumV0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
pub struct TradeManuEfficiencyComponents {
    pub trade_eff_pct: f64,
    pub gold_manu_eff_pct: f64,
    pub battle_record_manu_eff_pct: f64,
    pub trade_station_count: u8,
    pub gold_line_count: u8,
    pub battle_record_line_count: u8,
}

/// Current central-control inject sorting policy.
///
/// The three efficiency components remain separately meaningful. The returned
/// sort key preserves historical behavior only for ranking control-room fill
/// candidates.
pub fn evaluate_control_inject_policy(input: TradeManuEfficiencyComponents) -> PolicyEvaluation {
    PolicyEvaluation::new(
        ScoringPolicyId::ControlInjectRawSumV0,
        input.trade_eff_pct + input.gold_manu_eff_pct + input.battle_record_manu_eff_pct,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_inject_raw_sum_reports_policy_and_current_sort_key() {
        let score = evaluate_control_inject_policy(TradeManuEfficiencyComponents {
            trade_eff_pct: 7.0,
            gold_manu_eff_pct: 2.0,
            battle_record_manu_eff_pct: 3.0,
            trade_station_count: 2,
            gold_line_count: 2,
            battle_record_line_count: 2,
        });

        assert_eq!(score.policy, ScoringPolicyId::ControlInjectRawSumV0);
        assert_eq!(score.sort_key_pct, 12.0);
    }
}
