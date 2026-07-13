use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::layout::assignment::BaseAssignment;
use crate::layout::blueprint::{BaseBlueprint, RoomId, RoomProduct};
use crate::layout::context::LayoutContext;
use crate::pool::{ManuPool, PowerPool, TradePool};
use crate::skill_table::SkillTable;

use super::commit::{commit_manu_room, commit_trade_room};
use super::manufacture_fill::{
    manu_options, manufacture_candidate_pool_for_demand, pick_capacity_manu_hit, pick_manu_hit,
};
use super::power_fill::assign_power_rooms;
use super::trade_fill::{
    pick_trade_meta_then_plain, prioritize_docus_trade_rooms, trade_order_from_room,
};
use super::AssignBaseOptions;

/// 为一支队伍填满指定的贸易/制造房间（站绑定），共享 `used` 实现跨队互斥。
/// 贸易站取当前可用最优三人组（shortcut 自然高分），制造站同理；发电/中枢/宿舍不在此处理。
#[allow(clippy::too_many_arguments)]
pub fn assign_team_producer_rooms(
    blueprint: &BaseBlueprint,
    instances: &crate::instances::OperatorInstances,
    coexist_assignment: &BaseAssignment,
    durin_dorm_planning: Option<u8>,
    trade_pool: &TradePool,
    manu_pool: &ManuPool,
    table: &SkillTable,
    layout: &LayoutContext,
    options: &AssignBaseOptions,
    trade_rooms: &[RoomId],
    manu_rooms: &[RoomId],
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    assign_team_trade_meta_rooms(
        blueprint,
        trade_pool,
        table,
        layout,
        options,
        trade_rooms,
        assignment,
        used,
    )?;
    assign_team_manu_rooms(
        blueprint,
        instances,
        coexist_assignment,
        durin_dorm_planning,
        manu_pool,
        table,
        layout,
        options,
        manu_rooms,
        assignment,
        used,
    )
}

/// γ 替补半区：贸易沿用 meta 核心优先级，制造/发电仍站绑定搜索。
#[allow(clippy::too_many_arguments)]
pub fn assign_team_gamma_half(
    blueprint: &BaseBlueprint,
    instances: &crate::instances::OperatorInstances,
    coexist_assignment: &BaseAssignment,
    durin_dorm_planning: Option<u8>,
    trade_pool: &TradePool,
    manu_pool: &ManuPool,
    power_pool: &PowerPool,
    table: &SkillTable,
    layout: &LayoutContext,
    options: &AssignBaseOptions,
    trade_rooms: &[RoomId],
    manu_rooms: &[RoomId],
    power_rooms: &[RoomId],
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    assign_team_trade_meta_rooms(
        blueprint,
        trade_pool,
        table,
        layout,
        options,
        trade_rooms,
        assignment,
        used,
    )?;
    assign_team_manu_rooms(
        blueprint,
        instances,
        coexist_assignment,
        durin_dorm_planning,
        manu_pool,
        table,
        layout,
        options,
        manu_rooms,
        assignment,
        used,
    )?;
    assign_power_rooms(
        blueprint,
        power_pool,
        table,
        layout,
        options,
        power_rooms,
        assignment,
        used,
    )
}

#[allow(clippy::too_many_arguments)]
fn assign_team_trade_meta_rooms(
    blueprint: &BaseBlueprint,
    trade_pool: &TradePool,
    table: &SkillTable,
    layout: &LayoutContext,
    options: &AssignBaseOptions,
    trade_rooms: &[RoomId],
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    let gold_lines = blueprint.gold_manu_line_count();
    let mut rooms = trade_rooms
        .iter()
        .map(|room_id| {
            blueprint.room(room_id).ok_or_else(|| {
                Error::msg(format!("team trade room {} not in blueprint", room_id.0))
            })
        })
        .collect::<Result<Vec<_>>>()?;
    prioritize_docus_trade_rooms(&mut rooms, trade_pool, assignment, used);

    for room in rooms {
        if !assignment.operators_in(&room.id).is_empty() {
            continue;
        }
        let order = trade_order_from_room(room)?;
        let hit = pick_trade_meta_then_plain(
            trade_pool, table, layout, gold_lines, options, order, room.level, used,
        )
        .map_err(|e| Error::msg(format!("team trade {}: {e}", room.id.0)))?;
        commit_trade_room(assignment, &room.id, &hit, trade_pool, used)?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn assign_team_manu_rooms(
    blueprint: &BaseBlueprint,
    instances: &crate::instances::OperatorInstances,
    coexist_assignment: &BaseAssignment,
    durin_dorm_planning: Option<u8>,
    manu_pool: &ManuPool,
    table: &SkillTable,
    _layout: &LayoutContext,
    options: &AssignBaseOptions,
    manu_rooms: &[RoomId],
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    let candidate_pool = manufacture_candidate_pool_for_demand(manu_pool, used);

    for room_id in manu_rooms {
        if !assignment.operators_in(room_id).is_empty() {
            continue;
        }
        let room = blueprint
            .room(room_id)
            .ok_or_else(|| Error::msg(format!("team manu room {} not in blueprint", room_id.0)))?;
        let recipe = match room.product.as_ref() {
            Some(RoomProduct::Factory { recipe }) => *recipe,
            _ => {
                return Err(Error::msg(format!(
                    "team manu room {} missing factory product",
                    room_id.0
                )))
            }
        };
        let mut current_assignment = coexist_assignment.clone();
        for assigned_room in &assignment.rooms {
            current_assignment.set_room_assignment(assigned_room.clone());
        }
        let current_layout = crate::layout::resolve_base(
            blueprint,
            &current_assignment,
            Some(instances),
            Some(table),
            options.mood,
            durin_dorm_planning,
        )?
        .layout;
        let opts = manu_options(&current_layout, options, recipe, room.level);
        let hit = pick_manu_hit(&candidate_pool, table, opts.clone(), used, options.top_k)
            .or_else(|_| pick_manu_hit(manu_pool, table, opts, used, options.top_k))
            .or_else(|_| {
                pick_capacity_manu_hit(
                    manu_pool,
                    table,
                    &current_layout,
                    options,
                    recipe,
                    used,
                    room.level,
                )
            })
            .map_err(|e| Error::msg(format!("team manu {}: {e}", room_id.0)))?;
        commit_manu_room(assignment, room_id, &hit, manu_pool, used)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::assignment::AssignedOperator;
    use crate::operbox::{OperBox, OperBoxEntry};

    #[test]
    fn gamma_manufacture_search_consumes_coexisting_rhine_workforce() {
        let mut blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let manu_id = blueprint
            .rooms
            .iter_mut()
            .find(|room| room.kind == crate::FacilityKind::Factory)
            .map(|room| {
                room.product = Some(RoomProduct::Factory {
                    recipe: crate::types::RecipeKind::Gold,
                });
                room.id.clone()
            })
            .unwrap();
        let dorm_id = blueprint
            .rooms
            .iter()
            .find(|room| room.kind == crate::FacilityKind::Dormitory)
            .unwrap()
            .id
            .clone();
        let instances = crate::instances::OperatorInstances::load(
            &crate::instances::default_instances_path().unwrap(),
        )
        .unwrap();
        let table =
            SkillTable::load(&crate::skill_table::default_skill_table_path().unwrap()).unwrap();
        let operbox = OperBox::from_entries(
            [("娜斯提", 6), ("史都华德", 3), ("杰西卡", 4)]
                .into_iter()
                .enumerate()
                .map(|(index, (name, rarity))| OperBoxEntry {
                    id: format!("gamma_{index}"),
                    name: name.into(),
                    elite: 2,
                    level: 60,
                    own: true,
                    potential: 1,
                    rarity,
                })
                .collect(),
        );
        let pool = crate::pool::build_manufacture_pool(
            &operbox.manufacture_roster(&instances),
            &instances,
            &table,
        )
        .unwrap();
        let run = |coexist: &BaseAssignment, used: HashSet<String>| {
            let mut assignment = BaseAssignment::default();
            let mut used = used;
            assign_team_manu_rooms(
                &blueprint,
                &instances,
                coexist,
                Some(2),
                &pool,
                &table,
                &LayoutContext::search_baseline(),
                &AssignBaseOptions::default(),
                std::slice::from_ref(&manu_id),
                &mut assignment,
                &mut used,
            )
            .unwrap();
            assignment
        };
        let baseline = run(&BaseAssignment::default(), HashSet::new());
        let mut coexist = BaseAssignment::default();
        coexist.set_room(
            dorm_id,
            ["多萝西", "星源"]
                .into_iter()
                .map(|name| AssignedOperator::new(name, 2))
                .collect(),
        );
        let enhanced = run(
            &coexist,
            HashSet::from(["多萝西".to_string(), "星源".to_string()]),
        );
        assert_eq!(
            baseline.operators_in(&manu_id),
            enhanced.operators_in(&manu_id)
        );
        let base_eff = baseline
            .efficiency_in(&manu_id)
            .unwrap()
            .manufacture_final_efficiency;
        let enhanced_eff = enhanced
            .efficiency_in(&manu_id)
            .unwrap()
            .manufacture_final_efficiency;
        assert_eq!(
            enhanced_eff - base_eff,
            crate::efficiency::Efficiency::from_percent_points(6.0)
        );

        let mut merged = coexist.clone();
        merged.set_room_assignment(enhanced.room_assignment(&manu_id).unwrap().clone());
        let resolved = crate::layout::resolve_base(
            &blueprint,
            &merged,
            Some(&instances),
            Some(&table),
            24.0,
            Some(2),
        )
        .unwrap();
        let room = resolved
            .manu_rooms
            .iter()
            .find(|room| room.id == manu_id)
            .unwrap();
        let solved = crate::manufacture::solve_manufacture(
            &crate::manufacture::ManuRoomInput {
                level: room.level,
                operators: room.operators.clone(),
                active_recipe: room.recipe,
                mood: 24.0,
                layout: std::sync::Arc::new(room.layout.clone()),
            },
            &table,
        )
        .unwrap();
        assert_eq!(enhanced_eff, solved.final_efficiency);
    }

    #[test]
    fn gamma_manufacture_search_consumes_earlier_room_in_current_partial_assignment() {
        let mut blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let factory_ids: Vec<_> = blueprint
            .rooms
            .iter_mut()
            .filter(|room| room.kind == crate::FacilityKind::Factory)
            .take(2)
            .map(|room| {
                room.product = Some(RoomProduct::Factory {
                    recipe: crate::types::RecipeKind::Gold,
                });
                room.id.clone()
            })
            .collect();
        let first = &factory_ids[0];
        let second = &factory_ids[1];
        let instances = crate::instances::OperatorInstances::load(
            &crate::instances::default_instances_path().unwrap(),
        )
        .unwrap();
        let table =
            SkillTable::load(&crate::skill_table::default_skill_table_path().unwrap()).unwrap();

        for name in ["多萝西", "星源"] {
            assert!(instances
                .tags_for(name, crate::tier::PromotionTier::TierUp)
                .iter()
                .any(|tag| tag == crate::layout::TAG_RHINE));
        }
        for name in ["泡普卡", "卡达", "香草"] {
            assert!(!instances
                .tags_for(name, crate::tier::PromotionTier::TierUp)
                .iter()
                .any(|tag| tag == crate::layout::TAG_RHINE));
        }

        let operbox = OperBox::from_entries(
            [("娜斯提", 6), ("史都华德", 3), ("杰西卡", 4)]
                .into_iter()
                .enumerate()
                .map(|(index, (name, rarity))| OperBoxEntry {
                    id: format!("gamma_partial_{index}"),
                    name: name.into(),
                    elite: 2,
                    level: 60,
                    own: true,
                    potential: 1,
                    rarity,
                })
                .collect(),
        );
        let pool = crate::pool::build_manufacture_pool(
            &operbox.manufacture_roster(&instances),
            &instances,
            &table,
        )
        .unwrap();
        let pool_names: HashSet<_> = pool
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect();
        assert_eq!(pool_names, HashSet::from(["娜斯提", "史都华德", "杰西卡"]));

        let coexist = BaseAssignment::default();
        let run = |first_room_names: [&str; 3]| {
            let mut assignment = BaseAssignment::default();
            assignment.set_room(
                first.clone(),
                first_room_names
                    .into_iter()
                    .map(|name| AssignedOperator::new(name, 2))
                    .collect(),
            );
            let mut used: HashSet<String> =
                first_room_names.into_iter().map(str::to_string).collect();
            assert_eq!(used, assignment.operator_names());
            assign_team_manu_rooms(
                &blueprint,
                &instances,
                &coexist,
                Some(2),
                &pool,
                &table,
                &LayoutContext::search_baseline(),
                &AssignBaseOptions::default(),
                &[first.clone(), second.clone()],
                &mut assignment,
                &mut used,
            )
            .unwrap();
            assignment
        };

        let baseline = run(["泡普卡", "卡达", "香草"]);
        let enhanced = run(["多萝西", "星源", "香草"]);
        assert_eq!(
            baseline
                .operators_in(first)
                .iter()
                .map(|operator| operator.name.as_str())
                .collect::<Vec<_>>(),
            vec!["泡普卡", "卡达", "香草"]
        );
        assert_eq!(
            enhanced
                .operators_in(first)
                .iter()
                .map(|operator| operator.name.as_str())
                .collect::<Vec<_>>(),
            vec!["多萝西", "星源", "香草"]
        );
        assert_eq!(baseline.operators_in(second), enhanced.operators_in(second));

        let baseline_eff = baseline
            .efficiency_in(second)
            .unwrap()
            .manufacture_final_efficiency;
        let enhanced_eff = enhanced
            .efficiency_in(second)
            .unwrap()
            .manufacture_final_efficiency;
        assert_eq!(
            enhanced_eff - baseline_eff,
            crate::efficiency::Efficiency::from_percent_points(6.0)
        );

        for (assignment, searched_efficiency) in
            [(&baseline, baseline_eff), (&enhanced, enhanced_eff)]
        {
            let mut merged = coexist.clone();
            for room in &assignment.rooms {
                merged.set_room_assignment(room.clone());
            }
            let resolved = crate::layout::resolve_base(
                &blueprint,
                &merged,
                Some(&instances),
                Some(&table),
                24.0,
                Some(2),
            )
            .unwrap();
            let room = resolved
                .manu_rooms
                .iter()
                .find(|room| room.id == *second)
                .unwrap();
            let solved = crate::manufacture::solve_manufacture(
                &crate::manufacture::ManuRoomInput {
                    level: room.level,
                    operators: room.operators.clone(),
                    active_recipe: room.recipe,
                    mood: 24.0,
                    layout: std::sync::Arc::new(room.layout.clone()),
                },
                &table,
            )
            .unwrap();
            assert_eq!(searched_efficiency, solved.final_efficiency);
        }
    }
}
