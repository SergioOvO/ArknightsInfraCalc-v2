use serde::Serialize;

use super::components::ScoringPolicyId;

/// Efficiency percentage (`%`) with an explicit type at scoring boundaries.
///
/// Existing solver fields are still plain `f64`; this wrapper is for new
/// scoring-boundary APIs where a value's unit must be obvious.
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

/// Sort-key output produced by a named scoring policy.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct PolicyEvaluation {
    pub policy: ScoringPolicyId,
    pub sort_key_pct: f64,
}

impl PolicyEvaluation {
    pub const fn new(policy: ScoringPolicyId, sort_key_pct: f64) -> Self {
        Self {
            policy,
            sort_key_pct,
        }
    }
}
