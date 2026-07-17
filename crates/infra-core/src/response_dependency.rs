use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use crate::global_resource::CONVERSIONS;
use crate::skill_table::SkillTable;
use crate::types::{Action, AtomScope, Condition, Selector};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyScope {
    RoomLocal,
    SameFacility,
    CrossFacility,
    GlobalLayout,
    RuntimeState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseField {
    Efficiency,
    LimitOrStorage,
    UnitOutput,
    Mood,
    StateResource,
    GlobalInject,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseDependencyRow {
    pub skill_id: String,
    pub source_facility: String,
    pub target_facility: String,
    pub atom_index: usize,
    pub selector: Option<String>,
    pub selector_scope: Option<DependencyScope>,
    pub condition: Option<String>,
    pub condition_scope: Option<DependencyScope>,
    pub action_dependency: String,
    pub action_scope: DependencyScope,
    pub reads_resources: Vec<String>,
    pub writes_resources: Vec<String>,
    pub atom_scope: String,
    pub response_field: ResponseField,
    pub requires_external_state: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceConversionDependency {
    pub from: String,
    pub to: String,
    pub from_per: f64,
    pub to_per: f64,
    pub provider_buff_id: String,
    pub converter_buff_id: String,
    pub activation: String,
    pub operation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClosureEdgeKind {
    AtomProduce,
    AtomConvert,
    RegistryConversion,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceClosureEdge {
    pub kind: ResourceClosureEdgeKind,
    pub from: Option<String>,
    pub to: String,
    pub skill_id: Option<String>,
    pub atom_index: Option<usize>,
    pub source_facility: Option<String>,
    pub provider_buff_id: Option<String>,
    pub converter_buff_id: Option<String>,
    pub requires_same_shift_activation: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceReverseClosure {
    pub target_facility: String,
    pub response_field: ResponseField,
    pub seed_resources: Vec<String>,
    pub resources: Vec<String>,
    pub edges: Vec<ResourceClosureEdge>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceReadFormula {
    FloorDivideMultiplier {
        div: f64,
        multiplier: f64,
    },
    FloorStepDelta {
        step_size: f64,
        delta_per_step: f64,
    },
    FloorThenRoundMultiplier {
        multiplier: f64,
    },
    LinearConvert {
        ratio: f64,
    },
    IntegerTruncateSaturatingAdd {
        cap: u64,
        action_multiplier: f64,
        requires_physical_base: bool,
    },
    DeclaredConsume {
        div: f64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ResourceValueDomainFact {
    pub skill_id: String,
    pub atom_index: usize,
    pub target_facility: String,
    pub response_field: ResponseField,
    pub resource: String,
    pub formula: ResourceReadFormula,
    pub requires_producer_range_analysis: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnresolvedDelegatedDependency {
    pub facility: String,
    pub mechanism: String,
    pub resource: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseDependencyReport {
    pub coverage: String,
    pub skill_count: usize,
    pub atom_count: usize,
    pub external_atom_count: usize,
    pub by_target_facility: BTreeMap<String, usize>,
    pub external_by_target_facility: BTreeMap<String, usize>,
    pub dependency_edges_by_scope: BTreeMap<DependencyScope, usize>,
    pub by_response_field: BTreeMap<ResponseField, usize>,
    pub resource_conversions: Vec<ResourceConversionDependency>,
    pub resource_reverse_closures: Vec<ResourceReverseClosure>,
    pub resource_value_domains: Vec<ResourceValueDomainFact>,
    pub unresolved_delegated_dependencies: Vec<UnresolvedDelegatedDependency>,
    pub rows: Vec<ResponseDependencyRow>,
}

pub fn build_response_dependency_report(table: &SkillTable) -> ResponseDependencyReport {
    let mut rows = Vec::new();
    for skill in table.skills() {
        for (atom_index, atom) in skill.atoms.iter().enumerate() {
            let selector = atom.selector.as_ref().map(selector_dependency);
            let condition = atom.condition.as_ref().map(condition_dependency);
            let action = action_dependency(&atom.action, atom.scope);
            let atom_scope = match atom.scope {
                AtomScope::Room => "room",
                AtomScope::Global => "global",
            };
            let requires_external_state = atom.scope == AtomScope::Global
                || selector.is_some_and(|(_, scope)| scope != DependencyScope::RoomLocal)
                || condition.is_some_and(|(_, scope)| scope != DependencyScope::RoomLocal)
                || action.1 != DependencyScope::RoomLocal;
            rows.push(ResponseDependencyRow {
                skill_id: skill.id.clone(),
                source_facility: skill.facility.clone(),
                target_facility: target_facility(&skill.facility, &atom.action, atom.scope)
                    .to_string(),
                atom_index,
                selector: selector.map(|(name, _)| name.to_string()),
                selector_scope: selector.map(|(_, scope)| scope),
                condition: condition.map(|(name, _)| name.to_string()),
                condition_scope: condition.map(|(_, scope)| scope),
                action_dependency: action.0.to_string(),
                action_scope: action.1,
                reads_resources: read_resources(atom),
                writes_resources: written_resources(atom),
                atom_scope: atom_scope.to_string(),
                response_field: response_field(&atom.action),
                requires_external_state,
            });
        }
    }

    let mut by_target_facility = BTreeMap::new();
    let mut by_dependency_scope = BTreeMap::new();
    let mut by_response_field = BTreeMap::new();
    let mut external_by_target_facility = BTreeMap::new();
    for row in &rows {
        *by_target_facility
            .entry(row.target_facility.clone())
            .or_insert(0) += 1;
        for scope in [
            row.selector_scope,
            row.condition_scope,
            Some(row.action_scope),
        ]
        .into_iter()
        .flatten()
        {
            *by_dependency_scope.entry(scope).or_insert(0) += 1;
        }
        *by_response_field.entry(row.response_field).or_insert(0) += 1;
        if row.requires_external_state {
            *external_by_target_facility
                .entry(row.target_facility.clone())
                .or_insert(0) += 1;
        }
    }

    let resource_conversions: Vec<_> = CONVERSIONS
        .iter()
        .map(|conversion| ResourceConversionDependency {
            from: conversion.from.id().to_string(),
            to: conversion.to.id().to_string(),
            from_per: conversion.from_per,
            to_per: conversion.to_per,
            provider_buff_id: conversion.provider_buff_id.to_string(),
            converter_buff_id: conversion.converter_buff_id.to_string(),
            activation: "provider_and_converter_active_in_same_shift".to_string(),
            operation: "destructive_floor_conversion".to_string(),
        })
        .collect();
    let resource_value_domains = build_resource_value_domains(table);
    let resource_reverse_closures = build_resource_reverse_closures(&rows, &resource_conversions);

    ResponseDependencyReport {
        coverage: "effect_atom_plus_global_conversion_registry".to_string(),
        skill_count: table.skills().len(),
        atom_count: rows.len(),
        external_atom_count: rows
            .iter()
            .filter(|row| row.requires_external_state)
            .count(),
        by_target_facility,
        external_by_target_facility,
        dependency_edges_by_scope: by_dependency_scope,
        by_response_field,
        resource_conversions,
        resource_reverse_closures,
        resource_value_domains,
        unresolved_delegated_dependencies: [
            "real_gold_lines",
            "virtual_gold_lines",
            "durin_virtual_lines",
        ]
        .into_iter()
        .map(|resource| UnresolvedDelegatedDependency {
            facility: "trade".to_string(),
            mechanism: "gold_flow".to_string(),
            resource: resource.to_string(),
            reason: "L2 delegated mechanism is not represented by EffectAtom resource edges"
                .to_string(),
        })
        .collect(),
        rows,
    }
}

fn build_resource_reverse_closures(
    rows: &[ResponseDependencyRow],
    conversions: &[ResourceConversionDependency],
) -> Vec<ResourceReverseClosure> {
    let mut seeds: BTreeMap<(String, ResponseField), BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        if row.target_facility == "global_resource" || row.reads_resources.is_empty() {
            continue;
        }
        seeds
            .entry((
                row.target_facility.clone(),
                closure_response_field(row.response_field),
            ))
            .or_default()
            .extend(row.reads_resources.iter().cloned());
    }

    seeds
        .into_iter()
        .map(|((target_facility, response_field), seed_resources)| {
            let mut resources = seed_resources.clone();
            let mut queue: VecDeque<_> = seed_resources.iter().cloned().collect();
            let mut edges = BTreeMap::new();
            while let Some(resource) = queue.pop_front() {
                for row in rows
                    .iter()
                    .filter(|row| row.writes_resources.contains(&resource))
                {
                    let kind = if row.reads_resources.is_empty() {
                        ResourceClosureEdgeKind::AtomProduce
                    } else {
                        ResourceClosureEdgeKind::AtomConvert
                    };
                    let predecessors: Vec<Option<String>> = if row.reads_resources.is_empty() {
                        vec![None]
                    } else {
                        row.reads_resources.iter().cloned().map(Some).collect()
                    };
                    for from in predecessors {
                        if let Some(predecessor) = &from {
                            if resources.insert(predecessor.clone()) {
                                queue.push_back(predecessor.clone());
                            }
                        }
                        let key = (
                            kind,
                            from.clone(),
                            resource.clone(),
                            Some(row.skill_id.clone()),
                            Some(row.atom_index),
                            None,
                            None,
                        );
                        edges.entry(key).or_insert_with(|| ResourceClosureEdge {
                            kind,
                            from,
                            to: resource.clone(),
                            skill_id: Some(row.skill_id.clone()),
                            atom_index: Some(row.atom_index),
                            source_facility: Some(row.source_facility.clone()),
                            provider_buff_id: None,
                            converter_buff_id: None,
                            requires_same_shift_activation: false,
                        });
                    }
                }
                for conversion in conversions.iter().filter(|edge| edge.to == resource) {
                    if resources.insert(conversion.from.clone()) {
                        queue.push_back(conversion.from.clone());
                    }
                    let key = (
                        ResourceClosureEdgeKind::RegistryConversion,
                        Some(conversion.from.clone()),
                        resource.clone(),
                        None,
                        None,
                        Some(conversion.provider_buff_id.clone()),
                        Some(conversion.converter_buff_id.clone()),
                    );
                    edges.entry(key).or_insert_with(|| ResourceClosureEdge {
                        kind: ResourceClosureEdgeKind::RegistryConversion,
                        from: Some(conversion.from.clone()),
                        to: resource.clone(),
                        skill_id: None,
                        atom_index: None,
                        source_facility: None,
                        provider_buff_id: Some(conversion.provider_buff_id.clone()),
                        converter_buff_id: Some(conversion.converter_buff_id.clone()),
                        requires_same_shift_activation: true,
                    });
                }
            }
            ResourceReverseClosure {
                target_facility,
                response_field,
                seed_resources: seed_resources.into_iter().collect(),
                resources: resources.into_iter().collect(),
                edges: edges.into_values().collect(),
            }
        })
        .collect()
}

fn build_resource_value_domains(table: &SkillTable) -> Vec<ResourceValueDomainFact> {
    let mut facts = Vec::new();
    for skill in table.skills() {
        for (atom_index, atom) in skill.atoms.iter().enumerate() {
            let target = target_facility(&skill.facility, &atom.action, atom.scope).to_string();
            let field = closure_response_field(response_field(&atom.action));
            if let Some(Selector::StatePoolFloored { key, div }) = atom.selector.as_ref() {
                let multiplier = match atom.action {
                    Action::AddFlatEffFromSelector { multiplier, .. } => multiplier,
                    Action::GlobalInjectTradeEff { value }
                    | Action::GlobalInjectManuEff { value, .. }
                    | Action::GlobalInjectManuTaggedEff { value, .. } => value,
                    _ => 1.0,
                };
                facts.push(ResourceValueDomainFact {
                    skill_id: skill.id.clone(),
                    atom_index,
                    target_facility: target.clone(),
                    response_field: field,
                    resource: normalize_resource(key),
                    formula: ResourceReadFormula::FloorDivideMultiplier {
                        div: *div,
                        multiplier,
                    },
                    requires_producer_range_analysis: true,
                });
            }
            let (resource, formula) = match &atom.action {
                Action::StateConsumeToEff {
                    key,
                    div,
                    multiplier,
                } => (
                    key,
                    ResourceReadFormula::FloorDivideMultiplier {
                        div: *div,
                        multiplier: multiplier.unwrap_or(1.0),
                    },
                ),
                Action::MoodDrainPerStateStep {
                    key,
                    step_size,
                    delta_per_step,
                    ..
                } => (
                    key,
                    ResourceReadFormula::FloorStepDelta {
                        step_size: *step_size,
                        delta_per_step: *delta_per_step,
                    },
                ),
                Action::AddLimitFromState { key, multiplier } => (
                    key,
                    ResourceReadFormula::FloorThenRoundMultiplier {
                        multiplier: *multiplier,
                    },
                ),
                Action::StateConvert { from, ratio, .. } => {
                    (from, ResourceReadFormula::LinearConvert { ratio: *ratio })
                }
                Action::StateConsume { key, div } => {
                    (key, ResourceReadFormula::DeclaredConsume { div: *div })
                }
                _ => continue,
            };
            facts.push(ResourceValueDomainFact {
                skill_id: skill.id.clone(),
                atom_index,
                target_facility: target,
                response_field: field,
                resource: normalize_resource(resource),
                formula,
                requires_producer_range_analysis: true,
            });
        }
    }
    for skill in table.skills() {
        for (atom_index, atom) in skill.atoms.iter().enumerate() {
            if atom.selector != Some(Selector::PowerStationCount) {
                continue;
            }
            facts.push(ResourceValueDomainFact {
                skill_id: skill.id.clone(),
                atom_index,
                target_facility: target_facility(&skill.facility, &atom.action, atom.scope)
                    .to_string(),
                response_field: closure_response_field(response_field(&atom.action)),
                resource: "virtual_power".to_string(),
                formula: ResourceReadFormula::IntegerTruncateSaturatingAdd {
                    cap: 255,
                    action_multiplier: selector_action_multiplier(&atom.action),
                    requires_physical_base: true,
                },
                requires_producer_range_analysis: true,
            });
        }
    }
    facts
}

fn closure_response_field(field: ResponseField) -> ResponseField {
    match field {
        ResponseField::GlobalInject => ResponseField::Efficiency,
        other => other,
    }
}

fn selector_action_multiplier(action: &Action) -> f64 {
    match action {
        Action::AddFlatEffFromSelector { multiplier, .. } => *multiplier,
        Action::GlobalInjectTradeEff { value }
        | Action::GlobalInjectManuEff { value, .. }
        | Action::GlobalInjectManuTaggedEff { value, .. } => *value,
        _ => 1.0,
    }
}

fn read_resources(atom: &crate::types::EffectAtom) -> Vec<String> {
    let mut resources = Vec::new();
    if let Some(Selector::StatePoolFloored { key, .. }) = atom.selector.as_ref() {
        resources.push(normalize_resource(key));
    }
    if atom.selector == Some(Selector::PowerStationCount) {
        resources.push("virtual_power".to_string());
    }
    let action_resource = match &atom.action {
        Action::AddLimitFromState { key, .. }
        | Action::MoodDrainPerStateStep { key, .. }
        | Action::StateConsume { key, .. }
        | Action::StateConsumeToEff { key, .. } => Some(normalize_resource(key)),
        Action::StateConvert { from, .. } => Some(normalize_resource(from)),
        _ => None,
    };
    if let Some(resource) = action_resource {
        if !resources.contains(&resource) {
            resources.push(resource);
        }
    }
    resources
}

fn written_resources(atom: &crate::types::EffectAtom) -> Vec<String> {
    match &atom.action {
        Action::StateProduce { key, .. } => vec![normalize_resource(key)],
        Action::StateConvert { to, .. } => vec![normalize_resource(to)],
        _ => Vec::new(),
    }
}

fn normalize_resource(key: &str) -> String {
    crate::global_resource::GlobalResourceKey::parse(key)
        .map(|resource| resource.id().to_string())
        .unwrap_or_else(|| key.to_ascii_lowercase())
}

fn target_facility<'a>(source: &'a str, action: &Action, atom_scope: AtomScope) -> &'a str {
    match action {
        Action::GlobalInjectTradeEff { .. } | Action::GlobalInjectKarlanPrecision { .. } => "trade",
        Action::GlobalInjectManuEff { .. } | Action::GlobalInjectManuTaggedEff { .. } => {
            "manufacture"
        }
        Action::StateProduce { .. } if atom_scope == AtomScope::Global => "global_resource",
        _ => source,
    }
}

fn action_dependency(action: &Action, atom_scope: AtomScope) -> (&'static str, DependencyScope) {
    use DependencyScope::*;
    match action {
        Action::AddPerGapEff { .. } => ("order_gap", RuntimeState),
        Action::AddLimitFromState { .. }
        | Action::MoodDrainPerStateStep { .. }
        | Action::StateConsume { .. }
        | Action::StateConvert { .. }
        | Action::StateConsumeToEff { .. } => ("state_pool", RuntimeState),
        Action::GlobalInjectTradeEff { .. }
        | Action::GlobalInjectManuEff { .. }
        | Action::GlobalInjectManuTaggedEff { .. }
        | Action::GlobalInjectKarlanPrecision { .. } => ("global_inject", CrossFacility),
        Action::StateProduce { .. } if atom_scope == AtomScope::Global => {
            ("global_resource_produce", CrossFacility)
        }
        Action::StateProduce { .. } => ("room_state_produce", RoomLocal),
        Action::AddFlatEff { .. }
        | Action::TagOrder { .. }
        | Action::AddGoldDelivery { .. }
        | Action::ReduceLimit { .. }
        | Action::AddLimitFromSelector { .. }
        | Action::AddFlatEffFromSelector { .. }
        | Action::AddLimitDelta { .. }
        | Action::MoodDrainDelta { .. }
        | Action::AddOrderLmdBonus { .. }
        | Action::PeerEffAbsorb { .. }
        | Action::AddBucketEffFromSelector { .. }
        | Action::AddEffFromLimitContribSum { .. }
        | Action::AddEffFromLimitContribTiered { .. }
        | Action::AddEffRamp { .. } => ("no_implicit_input", RoomLocal),
    }
}

fn response_field(action: &Action) -> ResponseField {
    match action {
        Action::AddFlatEff { .. }
        | Action::AddPerGapEff { .. }
        | Action::PeerEffAbsorb { .. }
        | Action::AddBucketEffFromSelector { .. }
        | Action::AddEffFromLimitContribSum { .. }
        | Action::AddEffFromLimitContribTiered { .. }
        | Action::StateConsumeToEff { .. }
        | Action::AddFlatEffFromSelector { .. }
        | Action::AddEffRamp { .. } => ResponseField::Efficiency,
        Action::ReduceLimit { .. }
        | Action::AddLimitFromSelector { .. }
        | Action::AddLimitFromState { .. }
        | Action::AddLimitDelta { .. } => ResponseField::LimitOrStorage,
        Action::TagOrder { .. }
        | Action::AddGoldDelivery { .. }
        | Action::AddOrderLmdBonus { .. } => ResponseField::UnitOutput,
        Action::MoodDrainDelta { .. } | Action::MoodDrainPerStateStep { .. } => ResponseField::Mood,
        Action::StateProduce { .. } | Action::StateConsume { .. } | Action::StateConvert { .. } => {
            ResponseField::StateResource
        }
        Action::GlobalInjectTradeEff { .. }
        | Action::GlobalInjectManuEff { .. }
        | Action::GlobalInjectManuTaggedEff { .. }
        | Action::GlobalInjectKarlanPrecision { .. } => ResponseField::GlobalInject,
    }
}

fn selector_dependency(selector: &Selector) -> (&'static str, DependencyScope) {
    use DependencyScope::*;
    match selector {
        Selector::GoldDeliveryCount => ("gold_delivery_count", RuntimeState),
        Selector::OtherOpsDirectEff => ("other_ops_direct_eff", RoomLocal),
        Selector::OtherOpsTotalEff => ("other_ops_total_eff", RoomLocal),
        Selector::RoomPeerCount => ("room_peer_count", RoomLocal),
        Selector::RoomOperatorCount => ("room_operator_count", RoomLocal),
        Selector::FinalOrderLimit => ("final_order_limit", RuntimeState),
        Selector::LimitExcess => ("limit_excess", RuntimeState),
        Selector::FacilityLevel => ("facility_level", RoomLocal),
        Selector::FacilityLevelMinusOne => ("facility_level_minus_one", RoomLocal),
        Selector::TaggedCountInRoom { .. } => ("tagged_count_in_room", RoomLocal),
        Selector::LimitContribSum => ("limit_contrib_sum", RoomLocal),
        Selector::MeetingMaxLevel => ("meeting_max_level", CrossFacility),
        Selector::DormLevelSum => ("dorm_level_sum", CrossFacility),
        Selector::ManuRecipeKinds => ("manu_recipe_kinds", CrossFacility),
        Selector::EliteFacilityCount => ("elite_facility_count", GlobalLayout),
        Selector::SuiFacilityCount => ("sui_facility_count", GlobalLayout),
        Selector::CappedSuiFacilityCount { .. } => ("capped_sui_facility_count", GlobalLayout),
        Selector::DormOccupantCount => ("dorm_occupant_count", CrossFacility),
        Selector::OrderGap => ("order_gap", RuntimeState),
        Selector::OrderCount => ("order_count", RuntimeState),
        Selector::PeerSettledEffSum => ("peer_settled_eff_sum", RoomLocal),
        Selector::PeerSkillEffSum => ("peer_skill_eff_sum", RoomLocal),
        Selector::OtherOpsSettledEff => ("other_ops_settled_eff", RoomLocal),
        Selector::TradeStationCount => ("trade_station_count", CrossFacility),
        Selector::PowerStationCount => ("power_station_count", CrossFacility),
        Selector::PlatformCountInPower => ("platform_count_in_power", CrossFacility),
        Selector::TaggedCountInControl { .. } => ("tagged_count_in_control", CrossFacility),
        Selector::ControlOperatorCount => ("control_operator_count", CrossFacility),
        Selector::DroneCap => ("drone_cap", GlobalLayout),
        Selector::RhineLifeInBase => ("rhine_life_in_base", GlobalLayout),
        Selector::RhineLifeInBaseExcludingSelf => {
            ("rhine_life_in_base_excluding_self", GlobalLayout)
        }
        Selector::MetalFormulaSkillCountInRoom => ("metal_formula_skill_count_in_room", RoomLocal),
        Selector::StandardSkillCountInRoom => ("standard_skill_count_in_room", RoomLocal),
        Selector::RhineSkillCountInRoom => ("rhine_skill_count_in_room", RoomLocal),
        Selector::FacilityLevelSumExclMeeting => ("facility_level_sum_excl_meeting", GlobalLayout),
        Selector::TrainingRoomLevel => ("training_room_level", CrossFacility),
        Selector::Mood => ("mood", RuntimeState),
        Selector::TaggedCountInTradeSum { .. } => ("tagged_count_in_trade_sum", SameFacility),
        Selector::TradeStationsWithTaggedGte { .. } => {
            ("trade_stations_with_tagged_gte", SameFacility)
        }
        Selector::TaggedCountInManuSum { .. } => ("tagged_count_in_manu_sum", SameFacility),
        Selector::StatePoolFloored { .. } => ("state_pool_floored", RuntimeState),
    }
}

fn condition_dependency(condition: &Condition) -> (&'static str, DependencyScope) {
    use DependencyScope::*;
    match condition {
        Condition::GoldDeliveryBelow { .. } => ("gold_delivery_below", RuntimeState),
        Condition::GoldDeliveryAbove { .. } => ("gold_delivery_above", RuntimeState),
        Condition::GoldOrderInvestEligible { .. } => ("gold_order_invest_eligible", RuntimeState),
        Condition::OrderHasTag { .. } => ("order_has_tag", RuntimeState),
        Condition::OrderNotHasTag { .. } => ("order_not_has_tag", RuntimeState),
        Condition::MoodAbove { .. } => ("mood_above", RuntimeState),
        Condition::MoodAboveOrEq { .. } => ("mood_above_or_eq", RuntimeState),
        Condition::MoodBelow { .. } => ("mood_below", RuntimeState),
        Condition::MoodBelowOrEq { .. } => ("mood_below_or_eq", RuntimeState),
        Condition::OwnerEliteGte { .. } => ("owner_elite_gte", RoomLocal),
        Condition::OwnerEliteBelow { .. } => ("owner_elite_below", RoomLocal),
        Condition::PartnerInRoom { .. } => ("partner_in_room", RoomLocal),
        Condition::TagPresentInRoom { .. } => ("tag_present_in_room", RoomLocal),
        Condition::PeerTagInRoom { .. } => ("peer_tag_in_room", RoomLocal),
        Condition::OperatorInBase { .. } => ("operator_in_base", GlobalLayout),
        Condition::OperatorInPower { .. } => ("operator_in_power", CrossFacility),
        Condition::OperatorInTraining { .. } => ("operator_in_training", CrossFacility),
        Condition::OperatorInTrade { .. } => ("operator_in_trade", CrossFacility),
        Condition::NoPlatformInOtherPower { .. } => ("no_platform_in_other_power", SameFacility),
        Condition::OtherPlatformInPower { .. } => ("other_platform_in_power", SameFacility),
        Condition::OtherLateranoInPower { .. } => ("other_laterano_in_power", SameFacility),
        Condition::TiandaoEffVarAllowed { .. } => ("tiandao_eff_var_allowed", RoomLocal),
        Condition::ActiveRecipe { .. } => ("active_recipe", RoomLocal),
        Condition::OwnerLacksBuff { .. } => ("owner_lacks_buff", RoomLocal),
        Condition::ExternalMomentumGteField { .. } => ("external_momentum_gte_field", GlobalLayout),
        Condition::FieldMomentumGtExternal { .. } => ("field_momentum_gt_external", GlobalLayout),
        Condition::PlatformCountGte { .. } => ("platform_count_gte", CrossFacility),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skill_table::default_skill_table_path;

    #[test]
    fn report_finds_external_trade_and_manufacture_dependencies() {
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let report = build_response_dependency_report(&table);
        assert!(report.atom_count > 0);
        assert!(report.external_atom_count > 0);
        assert!(report.rows.iter().any(|row| {
            row.target_facility == "trade"
                && row.selector.as_deref() == Some("tagged_count_in_trade_sum")
        }));
        assert!(report.resource_conversions.iter().any(|conversion| {
            conversion.from == "dream"
                && conversion.to == "perception"
                && conversion.provider_buff_id == "dorm_rec_bd_n1_n2[000]"
                && conversion.converter_buff_id == "dorm_rec_bd_n1_n2[000]"
        }));
        assert!(report.rows.iter().any(|row| {
            row.skill_id == "trade_ord_spd_bd[010]"
                && row
                    .reads_resources
                    .iter()
                    .any(|resource| resource == "silent_echo")
        }));
        assert!(report.rows.iter().any(|row| {
            row.target_facility == "manufacture"
                && row.selector.as_deref() == Some("tagged_count_in_manu_sum")
        }));
        let trade_efficiency = report
            .resource_reverse_closures
            .iter()
            .find(|closure| {
                closure.target_facility == "trade"
                    && closure.response_field == ResponseField::Efficiency
            })
            .expect("trade efficiency resource closure");
        for resource in [
            "silent_echo",
            "perception",
            "passion",
            "dream",
            "musical_section",
            "memory_fragment",
        ] {
            assert!(
                trade_efficiency
                    .resources
                    .iter()
                    .any(|item| item == resource),
                "trade closure missing {resource}"
            );
        }
        assert!(trade_efficiency.edges.iter().any(|edge| {
            edge.kind == ResourceClosureEdgeKind::RegistryConversion
                && edge.from.as_deref() == Some("dream")
                && edge.to == "perception"
                && edge.requires_same_shift_activation
        }));
        let manufacture_efficiency = report
            .resource_reverse_closures
            .iter()
            .find(|closure| {
                closure.target_facility == "manufacture"
                    && closure.response_field == ResponseField::Efficiency
            })
            .expect("manufacture efficiency resource closure");
        for resource in ["thought_chain_ring", "perception", "virtual_power"] {
            assert!(
                manufacture_efficiency
                    .resources
                    .iter()
                    .any(|item| item == resource),
                "manufacture closure missing {resource}"
            );
        }
        assert!(report.resource_value_domains.iter().any(|fact| {
            fact.skill_id == "trade_ord_spd_bd[010]"
                && fact.resource == "silent_echo"
                && matches!(
                    fact.formula,
                    ResourceReadFormula::FloorDivideMultiplier {
                        div: 2.0,
                        multiplier: 1.0
                    }
                )
        }));
        assert!(report.resource_value_domains.iter().any(|fact| {
            fact.skill_id == "control_prod_bd_spd[000]"
                && fact.resource == "passion"
                && matches!(
                    fact.formula,
                    ResourceReadFormula::FloorDivideMultiplier {
                        div: 20.0,
                        multiplier: 0.5
                    }
                )
        }));
        assert!(report.resource_value_domains.iter().any(|fact| {
            fact.resource == "virtual_power"
                && matches!(
                    fact.formula,
                    ResourceReadFormula::IntegerTruncateSaturatingAdd {
                        cap: 255,
                        requires_physical_base: true,
                        ..
                    }
                )
        }));
        for resource in [
            "real_gold_lines",
            "virtual_gold_lines",
            "durin_virtual_lines",
        ] {
            assert!(report
                .unresolved_delegated_dependencies
                .iter()
                .any(|item| { item.mechanism == "gold_flow" && item.resource == resource }));
        }
    }

    #[test]
    fn global_inject_targets_production_facility() {
        assert_eq!(
            target_facility(
                "control",
                &Action::GlobalInjectTradeEff { value: 7.0 },
                AtomScope::Room,
            ),
            "trade"
        );
        assert_eq!(
            target_facility(
                "control",
                &Action::GlobalInjectManuEff {
                    value: 2.0,
                    recipe: None,
                },
                AtomScope::Room,
            ),
            "manufacture"
        );
        let injects = [
            Action::GlobalInjectTradeEff { value: 7.0 },
            Action::GlobalInjectManuEff {
                value: 2.0,
                recipe: None,
            },
            Action::GlobalInjectManuTaggedEff {
                value: 3.0,
                target_tag: "test".to_string(),
                recipe: None,
            },
            Action::GlobalInjectKarlanPrecision {
                eff_per_karlan: 5.0,
                limit_per_karlan: 1,
            },
        ];
        for action in injects {
            assert_eq!(
                action_dependency(&action, AtomScope::Room).1,
                DependencyScope::CrossFacility
            );
        }
    }

    #[test]
    fn action_implicit_inputs_are_never_false_local() {
        assert_eq!(
            action_dependency(&Action::AddPerGapEff { rate: 1.0 }, AtomScope::Room).1,
            DependencyScope::RuntimeState
        );
        assert_eq!(
            action_dependency(
                &Action::StateConsumeToEff {
                    key: "Matatabi".to_string(),
                    div: 1.0,
                    multiplier: None,
                },
                AtomScope::Room,
            )
            .1,
            DependencyScope::RuntimeState
        );
        let implicit_state_readers = [
            Action::AddLimitFromState {
                key: "Perception".to_string(),
                multiplier: 1.0,
            },
            Action::MoodDrainPerStateStep {
                key: "HumanFireworks".to_string(),
                step_size: 1.0,
                delta_per_step: -0.1,
                scope: crate::types::MoodDrainScope::SelfOp,
            },
            Action::StateConsume {
                key: "Perception".to_string(),
                div: 1.0,
            },
            Action::StateConvert {
                from: "Perception".to_string(),
                to: "SilentEcho".to_string(),
                ratio: 1.0,
            },
        ];
        for action in implicit_state_readers {
            assert_eq!(
                action_dependency(&action, AtomScope::Room).1,
                DependencyScope::RuntimeState
            );
        }
        assert_eq!(
            target_facility(
                "dorm",
                &Action::StateProduce {
                    key: "MonsterCuisine".to_string(),
                    amount: 1.0,
                },
                AtomScope::Global,
            ),
            "global_resource"
        );
        assert_eq!(
            target_facility(
                "trade",
                &Action::StateConsumeToEff {
                    key: "SilentEcho".to_string(),
                    div: 2.0,
                    multiplier: None,
                },
                AtomScope::Room,
            ),
            "trade"
        );
    }
}
