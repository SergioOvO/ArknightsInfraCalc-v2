mod control;
mod manufacture;
mod power;
mod role_pick;
mod trade;

pub(crate) use control::control_inject_policy_sort_key_for_layout;
pub use control::{
    control_efficiency_fill_sort_weight, control_entry_core_inject_fill,
    control_entry_dynamic_trade_tags, control_entry_layered_fill, control_entry_mood_cost_fill,
    control_entry_optional_dynamic_trade_tags, control_entry_plugin_fill, search_control_combos,
    ControlFillPolicy, ControlPolicyBreakdown, ControlSearchHit, ControlSearchOptions,
    MATATABI_CONSUMER_NAME,
};
pub use manufacture::{
    search_manufacture_triples, ManuEfficiencyBreakdown, ManuSearchHit, ManuSearchOptions,
    ManuSearchReport,
};
pub use power::{
    search_power_assignment, search_power_top, PowerEfficiencyBreakdown, PowerSearchHit,
    PowerSearchOptions, PowerSearchReport, PowerStationAssignment, VIRTUAL_POWER_MANU_EQUIV,
};
pub use role_pick::{hit_docus_syracusa_shortcut, pick_docus_trade_hit, pick_trade_role_hit};
pub use trade::{
    hit_blackkey_closure_shortcut, hit_closure_shortcut, hit_docus_solo_shortcut,
    hit_witch_shortcut, search_trade_triples, search_trade_triples_filtered, SearchTripleFilter,
    TradeEfficiencyBreakdown, TradeSearchHit, TradeSearchOptions, TradeSearchReport,
};
