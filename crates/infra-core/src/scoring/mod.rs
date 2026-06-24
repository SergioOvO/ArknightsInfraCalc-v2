//! Shared scoring units and balance-formula entry points.
//!
//! This module is intentionally small for now: real trade/manufacture balance
//! formulas belong here once the theory and anchors are available.

mod balance;
mod metric;

pub use balance::{placeholder_trade_manu_balance, BalanceFormulaId, TradeManuBalanceInput};
pub use metric::{BalancedEff, EffPct};
