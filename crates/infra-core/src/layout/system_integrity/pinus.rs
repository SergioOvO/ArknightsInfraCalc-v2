//! 红松林体系完整性判定。

use crate::layout::blueprint::{FacilityKind, RoomProduct};
use crate::layout::shift::AssignShiftMode;
use crate::types::RecipeKind;

use super::{EvaluateContext, PinusPlan, PinusVerdict, ShiftBind, SkipReason, SystemAnchor};

const SYSTEM_ID: &str = "pinus_sylvestris";
const YANWEI: &str = "焰尾";
const VIVIANA: &str = "薇薇安娜";
const MANUFACTURERS: &[&str] = &["灰毫", "远牙", "野鬃"];

pub fn evaluate_pinus(ctx: &EvaluateContext<'_>) -> PinusVerdict {
    if ctx.mode == AssignShiftMode::Recovery {
        return PinusVerdict::Skip(SkipReason::RecoveryShift);
    }
    for name in [YANWEI, VIVIANA] {
        if !ctx.owns_at_least(name, 2) {
            return PinusVerdict::Skip(SkipReason::MissingOperator {
                name: name.into(),
                need_elite: 2,
            });
        }
    }
    let has_battle_record_room = ctx.blueprint.rooms.iter().any(|room| {
        room.kind == FacilityKind::Factory
            && matches!(
                room.product,
                Some(RoomProduct::Factory {
                    recipe: RecipeKind::BattleRecord
                })
            )
    });
    if !has_battle_record_room {
        return PinusVerdict::Skip(SkipReason::MissingFacility {
            facility: FacilityKind::Factory,
            recipe: Some(RecipeKind::BattleRecord),
        });
    }

    let manufacturers: Vec<_> = MANUFACTURERS
        .iter()
        .filter(|name| ctx.owns_at_least(name, 2))
        .map(|name| (*name).to_string())
        .collect();
    if manufacturers.len() < 2 {
        return PinusVerdict::Skip(SkipReason::InsufficientOperators {
            group: "红松制造成员".into(),
            required: 2,
            present: manufacturers.len(),
        });
    }

    let control_anchors = [YANWEI, VIVIANA]
        .into_iter()
        .map(|operator| SystemAnchor {
            operator: operator.into(),
            elite: 2,
            facility: FacilityKind::ControlCenter,
            room_id: None,
        })
        .collect();
    let manufacture_anchors = manufacturers
        .iter()
        .map(|operator| SystemAnchor {
            operator: operator.clone(),
            elite: 2,
            facility: FacilityKind::Factory,
            room_id: None,
        })
        .collect();
    let mut bound = vec![YANWEI.into(), VIVIANA.into()];
    bound.extend(manufacturers);
    PinusVerdict::Activate(PinusPlan {
        system_id: SYSTEM_ID.into(),
        priority: 19,
        control_anchors,
        manufacture_anchors,
        shift_bind: ShiftBind {
            operators: bound,
            on_shifts: 2,
            off_shifts: 1,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::BaseBlueprint;
    use crate::operbox::OperBox;
    use crate::roster::OperatorProgress;
    use std::collections::HashMap;

    fn operbox(names: &[&str]) -> OperBox {
        OperBox {
            entries: vec![],
            owned: names
                .iter()
                .map(|name| ((*name).to_string(), OperatorProgress::new(2, 90, 6)))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn three_members_activate_and_all_are_bound() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let box_ = operbox(&[YANWEI, VIVIANA, "灰毫", "远牙", "野鬃"]);
        let PinusVerdict::Activate(plan) = evaluate_pinus(&EvaluateContext::new(
            &blueprint,
            &box_,
            AssignShiftMode::Peak,
        )) else {
            panic!("full pinus should activate");
        };
        assert_eq!(plan.manufacture_anchors.len(), 3);
        assert_eq!(plan.shift_bind.operators.len(), 5);
        assert_eq!(
            (plan.shift_bind.on_shifts, plan.shift_bind.off_shifts),
            (2, 1)
        );
    }

    #[test]
    fn two_members_activate_but_one_member_closes() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let two = operbox(&[YANWEI, VIVIANA, "灰毫", "远牙"]);
        let PinusVerdict::Activate(plan) = evaluate_pinus(&EvaluateContext::new(
            &blueprint,
            &two,
            AssignShiftMode::Peak,
        )) else {
            panic!("two pinus manufacturers should activate");
        };
        assert_eq!(plan.manufacture_anchors.len(), 2);

        let one = operbox(&[YANWEI, VIVIANA, "灰毫"]);
        assert!(matches!(
            evaluate_pinus(&EvaluateContext::new(
                &blueprint,
                &one,
                AssignShiftMode::Peak
            )),
            PinusVerdict::Skip(SkipReason::InsufficientOperators { .. })
        ));
    }

    #[test]
    fn missing_either_control_core_and_recovery_close() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        for missing in [YANWEI, VIVIANA] {
            let names: Vec<_> = [YANWEI, VIVIANA, "灰毫", "远牙"]
                .into_iter()
                .filter(|name| *name != missing)
                .collect();
            let box_ = operbox(&names);
            assert!(matches!(
                evaluate_pinus(&EvaluateContext::new(
                    &blueprint,
                    &box_,
                    AssignShiftMode::Peak
                )),
                PinusVerdict::Skip(SkipReason::MissingOperator { .. })
            ));
        }
        let box_ = operbox(&[YANWEI, VIVIANA, "灰毫", "远牙"]);
        assert_eq!(
            evaluate_pinus(&EvaluateContext::new(
                &blueprint,
                &box_,
                AssignShiftMode::Recovery
            )),
            PinusVerdict::Skip(SkipReason::RecoveryShift)
        );
    }
}
