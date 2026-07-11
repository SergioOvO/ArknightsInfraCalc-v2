//! Shared scoring units and component-based scoring policy entry points.
//!
//! Trade and manufacture efficiency components stay separate unless a local
//! ranking policy explicitly needs a sort key. No cross-domain balance formula
//! is assumed.

mod components;
mod metric;

pub use components::{
    evaluate_control_inject_policy, ScoringPolicyId, TradeManuEfficiencyComponents,
};
pub use metric::{EffPct, PolicyEvaluation};
