//! System 选型：产出 `AssignmentPlan`（不调 solve）。

use crate::error::Result;
use crate::layout::assignment::BaseAssignment;
use crate::layout::blueprint::BaseBlueprint;
use crate::layout::shift::AssignShiftMode;
use crate::layout::system::select_registry_systems;
use crate::layout::system::SlotFillMode;
use crate::layout::system_integrity::{
    evaluate_systems, EvaluateContext, PinusPlan, PinusVerdict, RosemaryPlan, RosemaryTier,
    RosemaryVerdict,
};
use crate::operbox::OperBox;

use super::plan::{
    registry_as_activated, ActivatedSystem, AnchorFillPolicy, AssignmentPlan, DegradationLadder,
    ProducerSlot, ShiftBind, SystemAnchor, SystemConstraint,
};

/// 根据 operbox / 蓝图 / 班次模式 / 种子编制构建编排计划。
/// `skip_system_ids`：静默跳过指定体系（如 system_integrity 已处理的 rosemary）。
pub fn build_plan(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    mode: AssignShiftMode,
    seed: &BaseAssignment,
    skip_system_ids: &std::collections::HashSet<String>,
) -> Result<AssignmentPlan> {
    if mode != AssignShiftMode::Peak {
        return Ok(AssignmentPlan::recovery(mode));
    }

    // 代码化体系层（system_integrity）与数据驱动 registry 汇合到统一 plan：
    // 迷迭香感知链产出 anchor / producer / constraint / degradation / shift_bind 语义片段。
    let mut anchors = Vec::new();
    let mut producers = Vec::new();
    let mut constraints = Vec::new();
    let mut degradations = Vec::new();
    let mut shift_binds = Vec::new();
    let mut code_activated = Vec::new();
    let ctx = EvaluateContext::new(blueprint, operbox, mode);
    let evaluated = evaluate_systems(&ctx);
    let rosemary_active = matches!(&evaluated.rosemary, RosemaryVerdict::Activate(_));
    let mut registry_skip = skip_system_ids.clone();
    if rosemary_active {
        // Pure fireworks is the no-perception branch; when perception is active,
        // do not claim a competing office/trade/control system.
        registry_skip.insert("human_fireworks_pure".to_string());
    } else {
        registry_skip.insert("human_fireworks_perception".to_string());
    }
    if let RosemaryVerdict::Activate(rplan) = evaluated.rosemary {
        merge_rosemary_into_plan(
            &rplan,
            &mut anchors,
            &mut producers,
            &mut constraints,
            &mut degradations,
            &mut shift_binds,
        );
    }
    if let PinusVerdict::Activate(pplan) = evaluated.pinus {
        merge_pinus_into_plan(&pplan, &mut anchors, &mut shift_binds);
        code_activated.push(ActivatedSystem {
            system_id: pplan.system_id.clone(),
            priority: pplan.priority,
            tier: crate::layout::tier::OperatorTier::CrossStation,
            slots: Vec::new(),
        });
    }

    // Registry selection must see codeized required anchors as occupied capacity/resources.
    // Execute uses the same first-compatible-room rule, so selection cannot admit a claim
    // that would only fit before those hard cores are placed.
    let mut scratch = seed.clone();
    for anchor in &anchors {
        let accepts = |room: &&crate::layout::blueprint::RoomBlueprint| {
            room.kind == anchor.facility
                && scratch.operators_in(&room.id).len() < room.operator_capacity()
                && anchor.recipe.is_none_or(|required| {
                    matches!(room.product, Some(crate::layout::blueprint::RoomProduct::Factory { recipe }) if recipe == required)
                })
        };
        let room_id = match &anchor.room_id {
            Some(id) => blueprint
                .rooms
                .iter()
                .find(|room| &room.id == id && accepts(room))
                .map(|room| room.id.clone()),
            None => blueprint
                .rooms
                .iter()
                .find(accepts)
                .map(|room| room.id.clone()),
        }
        .ok_or_else(|| {
            crate::error::Error::msg(format!(
                "required anchor {} for {} has no facility capacity",
                anchor.operator, anchor.system_id
            ))
        })?;
        let mut operators = scratch.operators_in(&room_id).to_vec();
        operators.push(crate::layout::AssignedOperator::new(
            anchor.operator.clone(),
            anchor.elite,
        ));
        scratch.set_room(room_id, operators);
    }
    let used = scratch.operator_names();
    let registry_claims =
        select_registry_systems(blueprint, operbox, mode, &scratch, &used, &registry_skip);
    let mut activated: Vec<_> = registry_claims.iter().map(registry_as_activated).collect();
    activated.extend(code_activated);
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

    Ok(AssignmentPlan {
        mode,
        activated,
        registry_claims,
        anchors,
        producers,
        constraints,
        degradations,
        shift_binds,
        control_candidate_requirements,
    })
}

/// 将代码化体系层的 `RosemaryPlan` 翻译为统一 plan 的语义片段。
/// 蓝本字段对齐：`system_integrity/plan.rs` 的 SystemAnchor / OptionalProducer /
/// RosemaryTier / ShiftBind → `orchestrate/plan.rs` 的同名一等公民类型。
fn merge_rosemary_into_plan(
    rplan: &RosemaryPlan,
    anchors: &mut Vec<SystemAnchor>,
    producers: &mut Vec<ProducerSlot>,
    constraints: &mut Vec<SystemConstraint>,
    degradations: &mut Vec<DegradationLadder>,
    shift_binds: &mut Vec<ShiftBind>,
) {
    for anchor in &rplan.anchors {
        anchors.push(SystemAnchor {
            system_id: rplan.system_id.clone(),
            operator: anchor.operator.clone(),
            elite: anchor.elite,
            facility: anchor.facility,
            room_id: anchor.room_id.clone(),
            recipe: None,
            fill_policy: match anchor.facility {
                crate::layout::FacilityKind::Factory => AnchorFillPolicy::ManufactureRecipe,
                _ => AnchorFillPolicy::Plain,
            },
        });
    }
    for producer in &rplan.optional_producers {
        producers.push(ProducerSlot {
            system_id: rplan.system_id.clone(),
            operator: producer.operator.clone(),
            elite: producer.elite,
            facility: producer.facility,
            optional: true,
        });
    }
    // 迷迭香制造 anchor 禁与自动化组（清流 / 温蒂）同房——「各占一条赤金线」
    // （ROSEMARY_PERCEPTION_CHAIN.md §3.3 / AUTOMATION_GROUP_CHAIN.md §2.4）。
    for anchor in &rplan.anchors {
        if anchor.operator == ROSEMARY_NAME {
            for peer in AUTOMATION_PEERS {
                constraints.push(SystemConstraint::ForbidSameRoom {
                    a: ROSEMARY_NAME.to_string(),
                    b: (*peer).to_string(),
                });
            }
        }
    }
    degradations.push(DegradationLadder {
        system_id: rplan.system_id.clone(),
        tier_label: rosemary_tier_label(rplan.tier).to_string(),
        priority: rplan.priority,
        producers_present: rplan.producers_present.clone(),
        producers_missing: rplan.producers_missing.clone(),
    });
    shift_binds.push(ShiftBind {
        operators: rplan.shift_bind.operators.clone(),
        on_shifts: rplan.shift_bind.on_shifts,
        off_shifts: rplan.shift_bind.off_shifts,
    });
}

fn merge_pinus_into_plan(
    pplan: &PinusPlan,
    anchors: &mut Vec<SystemAnchor>,
    shift_binds: &mut Vec<ShiftBind>,
) {
    anchors.extend(pplan.control_anchors.iter().map(|anchor| SystemAnchor {
        system_id: pplan.system_id.clone(),
        operator: anchor.operator.clone(),
        elite: anchor.elite,
        facility: anchor.facility,
        room_id: None,
        recipe: None,
        fill_policy: AnchorFillPolicy::Plain,
    }));
    anchors.extend(pplan.manufacture_anchors.iter().map(|anchor| SystemAnchor {
        system_id: pplan.system_id.clone(),
        operator: anchor.operator.clone(),
        elite: anchor.elite,
        facility: anchor.facility,
        room_id: None,
        recipe: Some(crate::types::RecipeKind::BattleRecord),
        fill_policy: AnchorFillPolicy::ManufactureRecipe,
    }));
    shift_binds.push(ShiftBind {
        operators: pplan.shift_bind.operators.clone(),
        on_shifts: pplan.shift_bind.on_shifts,
        off_shifts: pplan.shift_bind.off_shifts,
    });
}

const ROSEMARY_NAME: &str = "迷迭香";
const AUTOMATION_PEERS: &[&str] = &["清流", "温蒂"];

fn rosemary_tier_label(tier: RosemaryTier) -> &'static str {
    // 对齐 ROSEMARY_PERCEPTION_CHAIN.md §4 降级阶梯命名。
    match tier {
        RosemaryTier::Tier1 => "满配",
        RosemaryTier::Tier2 => "档2",
        RosemaryTier::Tier3 => "档3",
        RosemaryTier::Tier3Substitute => "替代感知源",
    }
}
