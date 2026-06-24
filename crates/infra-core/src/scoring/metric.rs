use serde::Serialize;

use super::balance::BalanceFormulaId;

/// Efficiency percentage (`%`) with an explicit type at scoring boundaries.
///
/// Existing solver fields are still plain `f64`; this wrapper is for new
/// formula-facing APIs where a value's unit must be obvious.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Default)]
pub struct EffPct(pub f64);

impl EffPct {
    pub const fn new(value: f64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> f64 {
        self.0
    }
}

impl From<f64> for EffPct {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

/// Formula output in a balanced efficiency percentage unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BalancedEff {
    pub formula: BalanceFormulaId,
    pub composite_eff_pct: f64,
}

impl BalancedEff {
    pub const fn new(formula: BalanceFormulaId, composite_eff_pct: f64) -> Self {
        Self {
            formula,
            composite_eff_pct,
        }
    }
}
