//! 编排计划：声明式描述「启用哪些体系、各 slot 如何填」，不含 trade/manu solve 评分。

use crate::layout::blueprint::{FacilityKind, RoomId};
use crate::layout::shift::AssignShiftMode;
use crate::layout::system::{RegistrySystemClaim, SlotFillMode};
use crate::layout::tier::OperatorTier;
use crate::trade::input::TradeOrderKind;
use crate::types::RecipeKind;
use std::collections::HashSet;

/// 兼容 registry claim 的单个 slot 落位方式；声明式 Rule 直接产出 `SystemAnchor`。
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

// ── 当前声明式体系语义类型 ─────────────────────────────────────────────

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

/// 体系锚点：钉核心干员与已解析设施/房间，队友由 fill 阶段补齐。
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
    /// 贸易 anchor 的订单约束；非贸易设施为 `None`。
    pub trade_order: Option<TradeOrderKind>,
    /// 单个干员的特殊工作心情；例如 Lancet-2 红脸上班。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_mood: Option<u8>,
    /// 该绑定组休息时的非宿舍去向；由轮换导出通用消费。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rest_facility: Option<FacilityKind>,
    pub fill_policy: AnchorFillPolicy,
}

/// 通用规则编译器已经选定的有限方案。后续阶段只消费结果，不再重跑体系选型。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SelectedRuleAlternative {
    pub rule_id: String,
    pub alternative_id: String,
    pub priority: i32,
    pub operators: Vec<String>,
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

/// exact 同班绑定：plan 供轮换消费，不承担进编责任。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ShiftBind {
    pub operators: Vec<String>,
    /// 0 表示只绑定同队，不声明固定工作班次数。
    pub on_shifts: u8,
    /// 0 表示不声明固定休息班次数。
    pub off_shifts: u8,
}

/// 单向在岗依赖：任一 consumer 在岗时，实际入选的 required 成员必须在基建内。
/// 当前轮换器把 required 作为全周期共享岗位；consumer 休息时不反向要求 required 离岗。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ActiveDependency {
    pub consumers: Vec<String>,
    pub required: Vec<String>,
}

/// 需要在所有可持续工作班次优先回岗的已解析角色；轮换层只负责通用恢复策略。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ContinuousRole {
    pub operator: String,
    pub facility: FacilityKind,
    pub room_id: RoomId,
}

/// 中枢搜索候选组约束：由 solver 在候选中至少选择 `min_count` 人。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ControlCandidateRequirement {
    pub candidates: Vec<String>,
    pub min_count: u8,
}

/// 单班进驻前的编排计划；`execute_plan` 将其落成 `BaseAssignment`。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssignmentPlan {
    pub mode: AssignShiftMode,
    /// 计划阶段已确定的规则 alternatives 与兼容 registry claims 的统一视图。
    pub activated: Vec<ActivatedSystem>,
    /// `base_systems.json` 认领明细（`execute_plan` 落位用）。
    pub registry_claims: Vec<RegistrySystemClaim>,
    /// 声明式规则编译结果；一条 rule 最多选择一个 finite alternative。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_rules: Vec<SelectedRuleAlternative>,
    /// 规则编译器与兼容 registry 汇合后的实际 required placements；room 已解析。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<SystemAnchor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub producers: Vec<ProducerSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<SystemConstraint>,
    /// 已选 alternative 明确排除的干员；fill / rotation 不得从普通候选重新引入。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded_operators: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub degradations: Vec<DegradationLadder>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub shift_binds: Vec<ShiftBind>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_dependencies: Vec<ActiveDependency>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continuous_roles: Vec<ContinuousRole>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub control_candidate_requirements: Vec<ControlCandidateRequirement>,
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
        let pure_fireworks_active = self
            .activated
            .iter()
            .any(|system| system.system_id == "human_fireworks_pure");

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
        let family_recognition_active = assignment
            .control_operators()
            .iter()
            .any(|operator| operator.name == "八幡海铃" && operator.elite >= 2);
        if family_recognition_active {
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
        if pure_fireworks_active && wuyou_active && anywhere_in(FacilityKind::Office, &["桑葚"]) {
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
            selected_rules: Vec::new(),
            anchors: Vec::new(),
            producers: Vec::new(),
            constraints: Vec::new(),
            excluded_operators: Vec::new(),
            degradations: Vec::new(),
            shift_binds: Vec::new(),
            active_dependencies: Vec::new(),
            continuous_roles: Vec::new(),
            control_candidate_requirements: Vec::new(),
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
        .filter(|slot| slot.fill != SlotFillMode::Search)
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

    #[test]
    fn actual_syracusa_bind_uses_only_present_cross_station_consumers() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut assignment = BaseAssignment::default();
        assignment.set_room("control", vec![AssignedOperator::new("八幡海铃", 2)]);

        let mut no_consumer = AssignmentPlan::recovery(AssignShiftMode::Peak);
        no_consumer.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(no_consumer.shift_binds.is_empty());

        set_trade_group(&mut assignment, &["伺夜"]);
        assignment.set_room("control", vec![AssignedOperator::new("八幡海铃", 0)]);
        let mut e0_no_skill = AssignmentPlan::recovery(AssignShiftMode::Peak);
        e0_no_skill.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(
            e0_no_skill.shift_binds.is_empty(),
            "精0 八幡海铃没有家族认可，不能仅因补位与 consumer 共存就派生 bind"
        );

        assignment.set_room("control", vec![AssignedOperator::new("八幡海铃", 2)]);
        let mut single = AssignmentPlan::recovery(AssignShiftMode::Peak);
        single.derive_actual_shift_binds(&blueprint, &assignment);
        assert_eq!(single.shift_binds.len(), 1);
        assert_eq!(single.shift_binds[0].operators.len(), 2);
        assert!(single.shift_binds[0]
            .operators
            .contains(&"八幡海铃".to_string()));
        assert!(single.shift_binds[0]
            .operators
            .contains(&"伺夜".to_string()));

        assignment.set_room("trade_2", vec![AssignedOperator::new("贝洛内", 2)]);
        let mut cross_station = AssignmentPlan::recovery(AssignShiftMode::Peak);
        cross_station.derive_actual_shift_binds(&blueprint, &assignment);
        assert_eq!(cross_station.shift_binds.len(), 1);
        for name in ["八幡海铃", "伺夜", "贝洛内"] {
            assert!(cross_station.shift_binds[0]
                .operators
                .contains(&name.to_string()));
        }

        assignment.set_room("control", vec![]);
        let mut no_producer = AssignmentPlan::recovery(AssignShiftMode::Peak);
        no_producer.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(no_producer.shift_binds.is_empty());
    }

    #[test]
    fn natural_standardization_members_do_not_create_exact_shift_bind() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut assignment = BaseAssignment::default();
        assignment.set_room("control", vec![AssignedOperator::new("涤火杰西卡", 2)]);
        assignment.set_room(
            "manu_1",
            vec![
                AssignedOperator::new("水月", 2),
                AssignedOperator::new("香草", 2),
                AssignedOperator::new("杰西卡", 2),
            ],
        );

        let mut plan = AssignmentPlan::recovery(AssignShiftMode::Peak);
        plan.derive_actual_shift_binds(&blueprint, &assignment);

        assert!(
            plan.shift_binds.is_empty(),
            "普通制造自然入选不得升级为固定成员/班次绑定: {:?}",
            plan.shift_binds
        );
    }

    #[test]
    fn pure_fireworks_legacy_bind_uses_actual_selected_members() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut assignment = BaseAssignment::default();
        set_trade_group(&mut assignment, &["乌有"]);
        let office_id = blueprint
            .rooms
            .iter()
            .find(|room| room.kind == FacilityKind::Office)
            .unwrap()
            .id
            .clone();
        assignment.set_room(office_id, vec![AssignedOperator::new("桑葚", 2)]);
        assignment.set_room("control", vec![AssignedOperator::new("令", 2)]);
        let mut pure = AssignmentPlan::recovery(AssignShiftMode::Peak);
        pure.activated.push(ActivatedSystem {
            system_id: "human_fireworks_pure".to_string(),
            priority: 18,
            tier: crate::layout::tier::OperatorTier::CrossStation,
            slots: Vec::new(),
        });
        pure.derive_actual_shift_binds(&blueprint, &assignment);
        assert!(pure.shift_binds.iter().any(|bind| {
            ["乌有", "桑葚", "令"]
                .iter()
                .all(|name| bind.operators.iter().any(|bound| bound == name))
                && !bind.operators.iter().any(|bound| bound == "重岳")
        }));
    }
}
