//! System 选型：产出 `AssignmentPlan`（不调 solve）。

use crate::error::Result;
use crate::layout::assignment::BaseAssignment;
use crate::layout::blueprint::BaseBlueprint;
use crate::layout::shift::AssignShiftMode;
use crate::layout::system::select_registry_systems;
use crate::layout::system_integrity::{
    evaluate_systems, EvaluateContext, RosemaryPlan, RosemaryTier, RosemaryVerdict,
};
use crate::operbox::OperBox;

use super::plan::{
    registry_as_activated, AnchorFillPolicy, AssignmentPlan, DegradationLadder, ProducerSlot,
    ShiftBind, SystemAnchor, SystemConstraint,
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

    let scratch = seed.clone();
    let used = scratch.operator_names();
    let registry_claims =
        select_registry_systems(blueprint, operbox, mode, &scratch, &used, skip_system_ids);
    let activated = registry_claims.iter().map(registry_as_activated).collect();

    // 代码化体系层（system_integrity）与数据驱动 registry 汇合到统一 plan：
    // 迷迭香感知链产出 anchor / producer / constraint / degradation / shift_bind 语义片段。
    let mut anchors = Vec::new();
    let mut producers = Vec::new();
    let mut constraints = Vec::new();
    let mut degradations = Vec::new();
    let mut shift_binds = Vec::new();
    let ctx = EvaluateContext::new(blueprint, operbox, mode);
    if let RosemaryVerdict::Activate(rplan) = evaluate_systems(&ctx).rosemary {
        merge_rosemary_into_plan(
            &rplan,
            &mut anchors,
            &mut producers,
            &mut constraints,
            &mut degradations,
            &mut shift_binds,
        );
    }

    Ok(AssignmentPlan {
        mode,
        activated,
        registry_claims,
        anchors,
        producers,
        constraints,
        degradations,
        shift_binds,
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
            // 迷迭香制造 anchor：队友按 recipe 约束搜索补齐。
            fill_policy: AnchorFillPolicy::ManufactureRecipe,
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
