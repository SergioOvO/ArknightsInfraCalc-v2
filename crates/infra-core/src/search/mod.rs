mod control;
mod manufacture;
mod power;
mod role_pick;
mod trade;

pub use control::{
    control_entry_core_inject_fill, control_entry_hr_mood_fill, control_entry_mood_cost_fill,
    search_control_combos, ControlFillPolicy, ControlScoreBreakdown, ControlSearchHit,
    ControlSearchOptions, MATATABI_CONSUMER_NAME,
};
pub use manufacture::{
    search_manufacture_triples, ManuScoreBreakdown, ManuSearchHit, ManuSearchOptions,
    ManuSearchReport,
};
pub use power::{
    search_power_assignment, search_power_top, PowerScoreBreakdown, PowerSearchHit,
    PowerSearchOptions, PowerSearchReport, PowerStationAssignment, VIRTUAL_POWER_MANU_EQUIV,
};
pub use role_pick::{hit_docus_syracusa_shortcut, pick_docus_trade_hit, pick_trade_role_hit};
pub use trade::{
    hit_blackkey_closure_shortcut, hit_closure_shortcut, hit_docus_solo_shortcut,
    hit_witch_shortcut, search_trade_triples, search_trade_triples_filtered, SearchTripleFilter,
    TradeScoreBreakdown, TradeSearchHit, TradeSearchOptions, TradeSearchReport,
};
