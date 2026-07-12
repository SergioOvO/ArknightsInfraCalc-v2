//! 编排计划：声明式描述「启用哪些体系、各 slot 如何填」，不含 trade/manu solve 评分。

use crate::layout::blueprint::{FacilityKind, RoomId};
use crate::layout::shift::AssignShiftMode;
use crate::layout::system::{RegistrySystemClaim, SlotFillMode};
use crate::layout::tier::OperatorTier;
use crate::types::RecipeKind;
use std::collections::HashSet;

/// 单个 slot 的落位方式（Phase 0 以 fixed / optional 为主；bond / core 在 Phase 2+ 扩展）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotFill {
    Fixed {
        operator: String,
        elite: u8,
        facility: FacilityKind,
        room_id: Option<RoomId>,
    },
    PickOne {
        candidates: Vec<String>,
        elite: u8,
        facility: FacilityKind,
        room_id: Option<RoomId>,
    },
    /// 可选 producer：缺房 / 缺人时 execute 静默跳过。
    OptionalFixed {
        operator: String,
        elite: u8,
        facility: FacilityKind,
    },
}

/// 已选中的成套体系（来自 `base_systems` 认领）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActivatedSystem {
    pub system_id: String,
    pub priority: i32,
    pub tier: OperatorTier,
    pub slots: Vec<SlotFill>,
}

// ── Phase 2 体系语义类型（ADR 0001 决策 C） ──────────────────────────────
// 当前仅定义类型并挂在 `AssignmentPlan` 上（默认空），不改变排班结果。
// 由后续 Phase（体系层接线 / execute 三态 / anchor fill）逐步产出与消费。
// 设计依据：docs/TODO/CODEIZED_SYSTEM_ORCHESTRATION_PLAN.md §2。

/// anchor 填充策略：决定 fill 阶段如何补齐 anchor 房的队友。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AnchorFillPolicy {
    /// 贸易 anchor：走 trade role / L3 shortcut 补齐。
    TradeRole { role_id: Option<String> },
    /// 制造 anchor：按 recipe 约束搜索补齐。
    ManufactureRecipe,
    /// 普通搜索补齐（must_include = 核心）。
    Plain,
}

/// 体系锚点：钉核心干员与设施，队友由 fill 阶段补齐（蓝本 `system_integrity/plan.rs:30`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SystemAnchor {
    pub system_id: String,
    pub operator: String,
    pub elite: u8,
    pub facility: FacilityKind,
    /// `None` = 该设施类型首个空房，不绑定具体 `room_id`。
    pub room_id: Option<RoomId>,
    /// 制造 anchor 的配方约束；非制造设施为 `None`。
    pub recipe: Option<RecipeKind>,
    pub fill_policy: AnchorFillPolicy,
}

/// 只提供全局 / 跨设施资源的 producer slot（蓝本 `OptionalProducer`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ProducerSlot {
    pub system_id: String,
    pub operator: String,
    pub elite: u8,
    pub facility: FacilityKind,
    /// 缺人时裁剪，不拖死核心。
    pub optional: bool,
}

/// 不占房但影响补齐搜索的约束（补 pairwise；现仅有系统级 `exclusive_group`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SystemConstraint {
    /// 两名干员禁止同房（迷迭香 ↔ 清流+温蒂）。
    ForbidSameRoom { a: String, b: String },
    /// 两名干员禁止同贸易站（黑键 ↔ 巫恋）。
    ForbidSameStation { a: String, b: String },
    /// 体系要求蓝图存在某设施。
    RequireFacility { facility: FacilityKind },
}

/// 降级阶梯结果（蓝本 `RosemaryTier` + `producers_present/missing`）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DegradationLadder {
    pub system_id: String,
    /// 满配 / 档1 / 档2 / 档3 / 替代感知源。
    pub tier_label: String,
    /// priority-by-tier：高档保持高优先，低档降至核心体系之下。
    pub priority: i32,
    pub producers_present: Vec<String>,
    pub producers_missing: Vec<String>,
}

/// 同班绑定（已存在于 `system_integrity/plan.rs:21`，迁入 plan 供轮换消费）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ShiftBind {
    pub operators: Vec<String>,
    pub on_shifts: u8,
    pub off_shifts: u8,
}

/// 单班进驻前的编排计划；`execute_plan` 将其落成 `BaseAssignment`。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssignmentPlan {
    pub mode: AssignShiftMode,
    /// 计划阶段已确定的体系（`base_systems` 选型结果）。
    pub activated: Vec<ActivatedSystem>,
    /// `base_systems.json` 认领明细（`execute_plan` 落位用）。
    pub registry_claims: Vec<RegistrySystemClaim>,
    /// Phase 2 体系语义：两路径（registry + 代码化体系层）汇合产出。
    /// 当前默认空，由后续 Phase 产出与消费，不影响现有排班。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<SystemAnchor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub producers: Vec<ProducerSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<SystemConstraint>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degradations: Vec<DegradationLadder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shift_binds: Vec<ShiftBind>,
}

impl AssignmentPlan {
    /// Derive cross-facility rotation binds from operators that actually made the peak assignment.
    /// This constrains rotation without turning a bind into an admission requirement.
    pub fn derive_actual_shift_binds(
        &mut self,
        blueprint: &crate::layout::BaseBlueprint,
        assignment: &crate::layout::BaseAssignment,
    ) {
        let in_control = |name: &str| {
            assignment
                .control_operators()
                .iter()
                .any(|op| op.name == name)
        };
        let together_in = |facility: FacilityKind, names: &[&str]| {
            blueprint.rooms.iter().any(|room| {
                room.kind == facility
                    && names.iter().all(|name| {
                        assignment
                            .operators_in(&room.id)
                            .iter()
                            .any(|op| op.name == *name)
                    })
            })
        };
        let anywhere_in = |facility: FacilityKind, names: &[&str]| {
            names.iter().all(|name| {
                blueprint.rooms.iter().any(|room| {
                    room.kind == facility
                        && assignment
                            .operators_in(&room.id)
                            .iter()
                            .any(|op| op.name == *name)
                })
            })
        };
        let perception_active = self.activated.iter().any(|system| {
            matches!(
                system.system_id.as_str(),
                "rosemary_perception" | "rosemary_perception_core"
            )
        });

        let mut add = |operators: &[&str]| {
            if self.shift_binds.iter().any(|bind| {
                operators
                    .iter()
                    .all(|name| bind.operators.iter().any(|bound| bound == name))
            }) {
                return;
            }
            self.shift_binds.push(ShiftBind {
                operators: operators.iter().map(|name| (*name).to_string()).collect(),
                on_shifts: 2,
                off_shifts: 1,
            });
        };

        if in_control("戴菲恩") && together_in(FacilityKind::TradePost, &["推进之王", "摩根"])
        {
            add(&["戴菲恩", "推进之王", "摩根"]);
        }
        if in_control("灵知") && together_in(FacilityKind::TradePost, &["孑", "银灰"]) {
            add(&["灵知", "孑", "银灰"]);
        }
        if in_control("涤火杰西卡")
            && anywhere_in(FacilityKind::Factory, &["水月", "香草", "杰西卡"])
        {
            add(&["涤火杰西卡", "水月", "香草", "杰西卡"]);
        }
        if in_control("八幡海铃") {
            let mut operators = vec!["八幡海铃"];
            for name in ["伺夜", "贝洛内"] {
                if anywhere_in(FacilityKind::TradePost, &[name]) {
                    operators.push(name);
                }
            }
            if operators.len() > 1 {
                add(&operators);
            }
        }

        let wuyou_active = anywhere_in(FacilityKind::TradePost, &["乌有"]);
        if perception_active && wuyou_active && in_control("重岳") && in_control("令") {
            add(&["重岳", "令", "乌有"]);
        } else if !perception_active && wuyou_active && anywhere_in(FacilityKind::Office, &["桑葚"])
        {
            let mut operators = vec!["桑葚", "乌有"];
            if in_control("重岳") {
                operators.push("重岳");
            }
            if in_control("令") {
                operators.push("令");
            }
            if operators.len() >= 3 {
                add(&operators);
            }
        }
    }

    pub fn recovery(mode: AssignShiftMode) -> Self {
        Self {
            mode,
            activated: Vec::new(),
            registry_claims: Vec::new(),
            anchors: Vec::new(),
            producers: Vec::new(),
            constraints: Vec::new(),
            degradations: Vec::new(),
            shift_binds: Vec::new(),
        }
    }

    /// 已选 `base_systems` 体系 id（按 priority 降序）。
    pub fn registry_system_ids(&self) -> Vec<&str> {
        let mut claims: Vec<_> = self.registry_claims.iter().collect();
        claims.sort_by_key(|c| std::cmp::Reverse(c.priority));
        claims.into_iter().map(|c| c.system_id.as_str()).collect()
    }

    /// 编排认领的贸易站房间（meta 锚点；余站由 `assign_trade_remainder` 填）。
    pub fn registry_trade_room_ids(&self) -> HashSet<RoomId> {
        self.registry_claims
            .iter()
            .flat_map(|c| &c.slots)
            .filter(|s| s.facility == FacilityKind::TradePost)
            .map(|s| s.room_id.clone())
            .collect()
    }

    /// 编排认领的全部干员名（registry fixed/pick 落位结果）。
    pub fn registry_operator_names(&self) -> HashSet<String> {
        self.registry_claims
            .iter()
            .flat_map(|c| &c.slots)
            .flat_map(|s| s.operators.iter().map(|o| o.name.clone()))
            .collect()
    }

    /// peak 切半后，registry 贸易 meta 房间与干员应落在 α 或 β（不在 γ 替补池）。
    pub fn verify_registry_trade_in_alpha_beta(
        &self,
        alpha: &crate::layout::BaseAssignment,
        beta: &crate::layout::BaseAssignment,
    ) -> Result<(), String> {
        for claim in &self.registry_claims {
            for slot in &claim.slots {
                if slot.facility != FacilityKind::TradePost {
                    continue;
                }
                if slot.fill == SlotFillMode::Search {
                    // Search slots declare eligible cross-station members, not required anchors.
                    // Only operators selected into peak are expected to appear in an alpha/beta
                    // slice; absence is valid and must not fail rotation verification.
                    continue;
                }
                let room = &slot.room_id;
                let alpha_ops = alpha.operators_in(room);
                let beta_ops = beta.operators_in(room);
                if alpha_ops.is_empty() && beta_ops.is_empty() {
                    return Err(format!(
                        "registry trade {} @ {} missing from α/β",
                        claim.system_id, room.0
                    ));
                }
                for op in &slot.operators {
                    let in_ab = alpha_ops.iter().any(|a| a.name == op.name)
                        || beta_ops.iter().any(|a| a.name == op.name);
                    if !in_ab {
                        return Err(format!(
                            "registry operator {} @ {} not in α/β slice",
                            op.name, room.0
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn registry_as_activated(claim: &RegistrySystemClaim) -> ActivatedSystem {
    let slots = claim
        .slots
        .iter()
        .flat_map(|slot| {
            slot.operators.iter().map(|op| SlotFill::Fixed {
                operator: op.name.clone(),
                elite: op.elite,
                facility: slot.facility,
                room_id: Some(slot.room_id.clone()),
            })
        })
        .collect();
    ActivatedSystem {
        system_id: claim.system_id.clone(),
        priority: claim.priority,
        tier: claim.tier,
        slots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{AssignedOperator, BaseAssignment, BaseBlueprint};

    fn set_trade_group(assignment: &mut BaseAssignment, names: &[&str]) {
        assignment.set_room(
            "trade_1",
            names
                .iter()
                .map(|name| AssignedOperator::new(*name, 2))
                .collect(),
        );
    }

    #[test]
    fn actual_vina_bind_requires_control_producer_and_both_trade_consumers() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut assignment = BaseAssignment::default();
        assignment.set_room("control", vec![AssignedOperator::new("戴菲恩", 2)]);
        set_trade_group(&mut assignment, &["推进之王", "摩根"]);

        let mut plan = AssignmentPlan::recovery(AssignShiftMode::Peak);
        plan.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(plan.shift_binds.iter().any(|bind| {
            ["戴菲恩", "推进之王", "摩根"]
                .iter()
                .all(|name| bind.operators.iter().any(|bound| bound == name))
        }));

        set_trade_group(&mut assignment, &["推进之王"]);
        let mut missing = AssignmentPlan::recovery(AssignShiftMode::Peak);
        missing.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(missing.shift_binds.is_empty());
    }

    #[test]
    fn actual_karlan_bind_does_not_admit_missing_control_producer() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut assignment = BaseAssignment::default();
        set_trade_group(&mut assignment, &["孑", "银灰"]);

        let mut plan = AssignmentPlan::recovery(AssignShiftMode::Peak);
        plan.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(plan.shift_binds.is_empty());

        assignment.set_room("control", vec![AssignedOperator::new("灵知", 2)]);
        plan.derive_actual_shift_binds(&blueprint, &assignment);
        assert_eq!(plan.shift_binds.len(), 1);
    }
}
