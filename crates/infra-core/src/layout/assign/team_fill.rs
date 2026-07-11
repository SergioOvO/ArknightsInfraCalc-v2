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
        blueprint, manu_pool, table, layout, options, manu_rooms, assignment, used,
    )
}

/// γ 替补半区：贸易沿用 meta 核心优先级，制造/发电仍站绑定搜索。
#[allow(clippy::too_many_arguments)]
pub fn assign_team_gamma_half(
    blueprint: &BaseBlueprint,
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
        blueprint, manu_pool, table, layout, options, manu_rooms, assignment, used,
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
    manu_pool: &ManuPool,
    table: &SkillTable,
    layout: &LayoutContext,
    options: &AssignBaseOptions,
    manu_rooms: &[RoomId],
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    let required_slots = manu_rooms
        .iter()
        .filter_map(|room_id| {
            if assignment.operators_in(room_id).is_empty() {
                blueprint.room(room_id).map(|room| room.operator_capacity())
            } else {
                None
            }
        })
        .sum();
    let candidate_pool = manufacture_candidate_pool_for_demand(manu_pool, used, required_slots);

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
        let opts = manu_options(layout, options, recipe, room.level);
        let hit = pick_manu_hit(&candidate_pool, table, opts.clone(), used, options.top_k)
            .or_else(|_| pick_manu_hit(manu_pool, table, opts, used, options.top_k))
            .or_else(|_| {
                pick_capacity_manu_hit(manu_pool, table, layout, options, recipe, used, room.level)
            })
            .map_err(|e| Error::msg(format!("team manu {}: {e}", room_id.0)))?;
        commit_manu_room(assignment, room_id, &hit, manu_pool, used)?;
    }
    Ok(())
}
