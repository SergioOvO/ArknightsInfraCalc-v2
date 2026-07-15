//! System 选型：产出 `AssignmentPlan`（不调 solve）。

use crate::error::{Error, Result};
use crate::instances::{default_instances_path, OperatorInstances};
use crate::layout::assignment::BaseAssignment;
use crate::layout::blueprint::BaseBlueprint;
use crate::layout::shift::AssignShiftMode;
use crate::layout::system::select_registry_systems;
use crate::layout::system::SlotFillMode;
use crate::operbox::OperBox;
use crate::skill_table::{default_skill_table_path, SkillTable};

use super::plan::{registry_as_activated, AssignmentPlan};
use super::rules::{
    compile_rules, default_rule_catalog_path, load_rule_catalog, validate_rule_preferences,
    RuleCatalog, RuleCompileContext,
};

/// 根据 operbox / 蓝图 / 班次模式 / 种子编制构建编排计划。
/// `skip_system_ids`：静默跳过指定体系。
pub fn build_plan(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    mode: AssignShiftMode,
    seed: &BaseAssignment,
    skip_system_ids: &std::collections::HashSet<String>,
) -> Result<AssignmentPlan> {
    let instances = OperatorInstances::load(&default_instances_path()?)?;
    let table = SkillTable::load(&default_skill_table_path()?)?;
    build_plan_with_runtime(
        blueprint,
        operbox,
        &instances,
        &table,
        24.0,
        &std::collections::HashMap::new(),
        mode,
        seed,
        skip_system_ids,
    )
}

/// 使用调用方已经加载的机制数据构建计划；资源 gate（例如感知 >= 50）读取候选计划
/// 的真实 resolve 结果。
#[allow(clippy::too_many_arguments)]
pub fn build_plan_with_runtime(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    mood: f64,
    preferences: &std::collections::HashMap<String, String>,
    mode: AssignShiftMode,
    seed: &BaseAssignment,
    skip_system_ids: &std::collections::HashSet<String>,
) -> Result<AssignmentPlan> {
    if mode != AssignShiftMode::Peak {
        return Ok(AssignmentPlan::recovery(mode));
    }

    let catalog = load_rule_catalog(&default_rule_catalog_path()?)?;
    validate_rule_preferences(&catalog, preferences)?;
    let mut disabled_conditional_packs = std::collections::HashSet::new();
    loop {
        let (plan, scratch) = compile_plan_once(
            &catalog,
            blueprint,
            operbox,
            instances,
            table,
            mood,
            preferences,
            mode,
            seed,
            skip_system_ids,
            &disabled_conditional_packs,
        )?;
        let Some(pack_key) = pre_split_pack_violation(blueprint, &scratch, &plan)? else {
            return Ok(plan);
        };
        if !disabled_conditional_packs.insert(pack_key.clone()) {
            return Err(Error::msg(format!(
                "conditional pack {pack_key} remained rotation-infeasible after downgrade"
            )));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn compile_plan_once(
    catalog: &RuleCatalog,
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    mood: f64,
    preferences: &std::collections::HashMap<String, String>,
    mode: AssignShiftMode,
    seed: &BaseAssignment,
    skip_system_ids: &std::collections::HashSet<String>,
    disabled_conditional_packs: &std::collections::HashSet<String>,
) -> Result<(AssignmentPlan, BaseAssignment)> {
    // priority >= 19 是用户确认的硬/主要体系；先于 legacy registry 编译。
    let mut compiled = compile_rules(
        catalog,
        &RuleCompileContext {
            blueprint,
            operbox,
            instances,
            table,
            mode,
            mood,
            preferences,
        },
        seed,
        &std::collections::HashSet::new(),
        skip_system_ids,
        disabled_conditional_packs,
        19..=i32::MAX,
    )?;
    let mut registry_skip = skip_system_ids.clone();
    registry_skip.extend(compiled.skip_registry_ids.iter().cloned());

    // Registry only sees the already resolved rule plan, so capacity and operator ownership have
    // a single ordering source.
    let mut scratch = compiled.scratch.clone();
    let mut used = scratch.operator_names();
    let inherited_excluded: std::collections::HashSet<_> =
        compiled.excluded_operators.iter().cloned().collect();
    used.extend(inherited_excluded.iter().cloned());
    let registry_claims =
        select_registry_systems(blueprint, operbox, mode, &scratch, &used, &registry_skip);
    for claim in &registry_claims {
        crate::layout::system::apply_registry_system_claim(
            blueprint,
            claim,
            &mut scratch,
            &mut used,
        )?;
    }
    // priority <= 18 的软候选只读取主要体系和 registry 都落位后的真实空位。
    let late = compile_rules(
        catalog,
        &RuleCompileContext {
            blueprint,
            operbox,
            instances,
            table,
            mode,
            mood,
            preferences,
        },
        &scratch,
        &inherited_excluded,
        &registry_skip,
        disabled_conditional_packs,
        i32::MIN..=18,
    )?;
    let final_scratch = late.scratch.clone();
    compiled.activated.extend(late.activated);
    compiled.selected.extend(late.selected);
    compiled.anchors.extend(late.anchors);
    compiled.constraints.extend(late.constraints);
    compiled.excluded_operators.extend(late.excluded_operators);
    compiled.shift_binds.extend(late.shift_binds);
    compiled
        .active_dependencies
        .extend(late.active_dependencies);
    compiled.continuous_roles.extend(late.continuous_roles);
    compiled.rotation_reserves.extend(late.rotation_reserves);
    let mut activated: Vec<_> = registry_claims.iter().map(registry_as_activated).collect();
    activated.extend(compiled.activated);
    let control_candidate_requirements = registry_claims
        .iter()
        .flat_map(|claim| &claim.slots)
        .filter(|slot| {
            slot.facility == crate::layout::FacilityKind::ControlCenter
                && slot.fill == SlotFillMode::Search
                && slot.required_count > 0
        })
        .map(|slot| super::plan::ControlCandidateRequirement {
            candidates: slot.operators.iter().map(|op| op.name.clone()).collect(),
            min_count: slot.required_count,
        })
        .collect();

    let plan = AssignmentPlan {
        mode,
        activated,
        registry_claims,
        selected_rules: compiled.selected,
        anchors: compiled.anchors,
        producers: Vec::new(),
        constraints: compiled.constraints,
        excluded_operators: compiled.excluded_operators,
        degradations: Vec::new(),
        shift_binds: compiled.shift_binds,
        active_dependencies: compiled.active_dependencies,
        continuous_roles: compiled.continuous_roles,
        rotation_reserves: compiled.rotation_reserves,
        control_candidate_requirements,
    };
    Ok((plan, final_scratch))
}

fn pre_split_pack_violation(
    blueprint: &BaseBlueprint,
    scratch: &BaseAssignment,
    plan: &AssignmentPlan,
) -> Result<Option<String>> {
    let guarded: Vec<_> = plan
        .rotation_reserves
        .iter()
        .filter(|reserve| {
            reserve.require_pre_split_halves
                && reserve.reuse_policy == super::plan::ReserveReusePolicy::EveryEligibleHalf
        })
        .collect();
    if guarded.is_empty() {
        return Ok(None);
    }
    let halves = super::pack_production_components(
        blueprint,
        scratch,
        plan.shift_binds
            .iter()
            .map(|bind| bind.operators.as_slice()),
    )
    .map_err(Error::msg)?;
    for reserve in guarded {
        let has_target = |components: &[Vec<crate::layout::RoomId>]| {
            components.iter().any(|component| {
                component
                    .iter()
                    .any(|room| reserve.eligible_rooms.contains(room))
            })
        };
        if has_target(&halves[0]) && has_target(&halves[1]) {
            continue;
        }
        let selected = plan
            .selected_rules
            .iter()
            .find(|selected| selected.rule_id == reserve.system_id)
            .ok_or_else(|| {
                Error::msg(format!(
                    "rotation reserve {} has no selected rule owner {}",
                    reserve.reserve_id, reserve.system_id
                ))
            })?;
        let pack_id = selected.conditional_pack_id.as_deref().ok_or_else(|| {
            Error::msg(format!(
                "guarded rotation reserve {} is not owned by a conditional pack",
                reserve.reserve_id
            ))
        })?;
        return Ok(Some(format!("{}:{pack_id}", reserve.system_id)));
    }
    Ok(None)
}
