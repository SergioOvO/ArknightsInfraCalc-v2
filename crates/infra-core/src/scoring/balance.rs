use serde::Serialize;

use super::BalancedEff;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BalanceFormulaId {
    Placeholder,
    GongsunTradeManuV1,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Default)]
pub struct TradeManuBalanceInput {
    pub trade_eff_pct: f64,
    pub gold_manu_eff_pct: f64,
    pub battle_record_manu_eff_pct: f64,
    pub trade_station_count: u8,
    pub gold_line_count: u8,
    pub battle_record_line_count: u8,
}

/// Temporary formula entry point for trade/manufacture balancing.
///
/// This preserves the historical raw-sum behavior only to mark the call site.
/// It is not a theoretical formula and must not be used as a final anchor.
pub fn placeholder_trade_manu_balance(input: TradeManuBalanceInput) -> BalancedEff {
    BalancedEff::new(
        BalanceFormulaId::Placeholder,
        input.trade_eff_pct + input.gold_manu_eff_pct + input.battle_record_manu_eff_pct,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholder_reports_formula_id_and_current_passthrough_sum() {
        let balanced = placeholder_trade_manu_balance(TradeManuBalanceInput {
            trade_eff_pct: 7.0,
            gold_manu_eff_pct: 2.0,
            battle_record_manu_eff_pct: 3.0,
            trade_station_count: 2,
            gold_line_count: 2,
            battle_record_line_count: 2,
        });

        assert_eq!(balanced.formula, BalanceFormulaId::Placeholder);
        assert_eq!(balanced.composite_eff_pct, 12.0);
    }
}
