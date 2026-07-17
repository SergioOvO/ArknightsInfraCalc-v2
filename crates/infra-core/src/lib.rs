pub mod bake;
pub mod box_profile;
pub mod candidate;
pub mod cross_facility;
pub mod eff_ramp;
pub mod efficiency;
pub mod error;
pub mod global_resource;
pub mod instances;
pub mod mood;
pub mod operbox;
pub mod pool;
pub mod profile;
pub mod response_dependency;
pub mod roster;
pub mod schedule;
pub mod scoring;
pub mod search;
pub mod skill_table;
pub mod tier;
pub mod training_advice;
pub mod types;

pub mod control;
pub mod export;
pub mod layout;
pub mod manufacture;
pub mod office;
pub mod power;
pub mod trade;

pub use bake::{
    bake_catalogs, default_baked_out_dir, load_complete_baked_trade_catalog,
    validate_baked_catalog, verify_baked_catalog_responses, warm_runtime_baked_table,
    BakeGeneratorFingerprint, BakeMode, BakeOptions, BakeProgressEvent, BakeReport,
    BakedTradeRowId, CompleteBakedTradeCatalog, CompleteBakedTradeRow, CompleteBakedTradeRows,
    BAKE_SCHEMA_VERSION,
};
pub use box_profile::{
    baseline_path_or_default, build_box_profile, render_box_profile_narrative, ActionKind,
    BoxProfile, BoxProfileOptions, GapSeverity, ProfileAction,
};
pub use candidate::{CandidateSource, CandidateStationKind, TeamCandidate, TeamMetric};
pub use control::{
    apply_control_to_layout, solve_control, ControlCenterResult, ControlOperator, ControlRoomInput,
};
pub use efficiency::Efficiency;
pub use error::{Error, Result};
pub use export::{build_from_team_rotation, MaaExportOptions, MaaSchedule};
pub use global_resource::{
    GlobalInjectManifest, GlobalResourceConversion, GlobalResourceEntry, GlobalResourceKey,
    GlobalResourcePool, GlobalResourceTier, CONVERSIONS, INJECT_FAMILY_MANU_GLOBAL_ALL,
    INJECT_FAMILY_TRADE_GLOBAL_FLAT, REGISTRY,
};
pub use instances::{buff_stem, resolve_buff_ids, OperatorInstances};
pub use layout::{
    assign_base_greedy, assign_shift, assignment_operator_names, pinned_assignment,
    resolve_automation_group_1_layout, resolve_base, resolve_search_baseline_layout,
    rotating_workers, AssignBaseOptions, AssignShiftMode, AssignedOperator, BaseAssignment,
    BaseBlueprint, FacilityKind, LayoutContext, OperatorTier, ResolvedBase, RoomId, RoomProduct,
    SharedLayout, DEFAULT_DORM_OCCUPANT_COUNT,
};
pub use manufacture::{
    evaluate_manufacture_lines, solve_manufacture, ManuLineEfficiency, ManuLineScenario,
    ManuOperator, ManuProdBreakdown, ManuResult, ManuRoomInput, ManuSearchRecipeMode,
    ManuStorageBreakdown,
};
pub use mood::{
    dorm_recovery_rates, facility_key as mood_facility_key, operator_net_drain, shift_eta,
    workable_hours, DormOccupant, DrainInputs, MoodModel, OperatorEta, ShiftEta,
};
pub use operbox::{
    default_layout_243_path, default_operbox_full_e2_path, default_operbox_gongsun_path,
    from_xlsx_path, OperBox,
};
pub use pool::{
    build_control_pool, build_manufacture_pool, build_power_pool, build_trade_pool,
    filter_control_pool, filter_manufacture_pool, filter_trade_pool, jie_e0_trade_operator,
    jie_market_trade_operator, ControlPool, ControlPoolEntry, ManuPool, ManuPoolEntry, PowerPool,
    PowerPoolEntry, TradePool, TradePoolEntry, JIE_TRADE_NAME,
};
pub use power::{
    apply_power_to_layout, charge_ramp_from_buffs, solve_power, PowerOperator, PowerResult,
    PowerRoomInput,
};
pub use response_dependency::{
    build_response_dependency_report, build_response_dependency_report_for_blueprint,
    DependencyScenario, DependencyScope, DomainDependencyContributor, DomainDependencyInput,
    DomainDependencyInputDecl, DomainInputSource, ResourceClosureEdge, ResourceClosureEdgeKind,
    ResourceConversionDependency, ResourceEquivalenceClass, ResourceReachableRange,
    ResourceReadFormula, ResourceReverseClosure, ResourceValueDomainFact, ResponseDependencyReport,
    ResponseDependencyRow, ResponseField, UnresolvedDelegatedDependency,
};
pub use roster::{OperatorProgress, Roster};
pub use schedule::{
    evaluate_base_assignment_efficiencies, operator_team_map, schedule_team_rotation, DailyTotals,
    ShiftEfficiencies, TeamAssignment, TeamLabel, TeamRotationReport, TeamShiftResult,
};
pub use scoring::{
    evaluate_control_inject_policy, EffPct, PolicyEvaluation, ScoringPolicyId,
    TradeManuEfficiencyComponents,
};
pub use search::{
    hit_closure_shortcut, hit_docus_solo_shortcut, hit_witch_shortcut, search_control_combos,
    search_manufacture_triples, search_power_assignment, search_power_top, search_trade_triples,
    search_trade_triples_filtered, ControlSearchHit, ControlSearchOptions, ManuSearchHit,
    ManuSearchOptions, ManuSearchReport, PowerSearchHit, PowerSearchOptions, PowerSearchReport,
    PowerStationAssignment, SearchTripleFilter, TradeSearchHit, TradeSearchOptions,
    TradeSearchReport,
};
pub use skill_table::SkillTable;
pub use tier::PromotionTier;
pub use training_advice::{
    build_training_advice, default_training_recommendations_path,
    load_training_recommendation_rules, OperatorTrainingState, PickOneCoreRule, RagContextItem,
    RecommendationKind, RecommendationPriority, StandaloneRecommendationRule,
    SystemRecommendationRule, SystemStatus, TrainingAdviceOptions, TrainingAdviceReport,
    TrainingAdviceSummary, TrainingRecommendation, TrainingRecommendationRules,
    TrainingSystemReport, TrainingTarget,
};
pub use types::*;
