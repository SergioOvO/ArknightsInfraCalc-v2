use std::collections::BTreeMap;

use serde::Serialize;

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
    pub atom_scope: String,
    pub response_field: ResponseField,
    pub requires_external_state: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponseDependencyReport {
    pub skill_count: usize,
    pub atom_count: usize,
    pub external_atom_count: usize,
    pub by_target_facility: BTreeMap<String, usize>,
    pub external_by_target_facility: BTreeMap<String, usize>,
    pub dependency_edges_by_scope: BTreeMap<DependencyScope, usize>,
    pub by_response_field: BTreeMap<ResponseField, usize>,
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

    ResponseDependencyReport {
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
        rows,
    }
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
        assert!(report.rows.iter().any(|row| {
            row.target_facility == "manufacture"
                && row.selector.as_deref() == Some("tagged_count_in_manu_sum")
        }));
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
