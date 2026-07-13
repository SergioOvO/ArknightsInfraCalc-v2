use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::efficiency::Efficiency;
use crate::error::{Error, Result};
use crate::instances::OperatorInstances;
#[cfg(test)]
use crate::layout::RoomAssignment;
use crate::layout::{
    assign_control, assign_manu_room_with_anchors, assign_shift_with_plan_skip,
    assign_team_gamma_half, pinned_assignment_excluding, resolve_base, ActivatedSystem,
    AssignBaseOptions, AssignShiftMode, AssignedOperator, AssignmentPlan, BaseAssignment,
    BaseBlueprint, FacilityKind, LayoutContext, RoomId, RoomProduct, SlotFill,
};
use crate::mood::{shift_eta, MoodModel, ShiftEta};
use crate::office::{solve_office, OfficeOperator, OfficeRoomInput};
use crate::operbox::OperBox;
use crate::pool::{
    add_jie_market_to_trade_pool, build_control_pool_with_fillers, build_manufacture_pool,
    build_power_pool, build_trade_pool, karlan_precision_active, ManuPool, ManuPoolEntry,
};
use crate::search::{control_efficiency_fill_sort_weight, control_entry_plugin_fill};
use crate::skill_table::SkillTable;
use crate::tier::PromotionTier;

use super::base_rotation::{evaluate_base_assignment_efficiencies, ShiftEfficiencies};
use super::shift_bind::{shift_binds_from_plan, RuntimeShiftBind};

/// αβγ 三队标签。每班两队上岗、一队休息；设施每班全部满编（不空转）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamLabel {
    Alpha,
    Beta,
    Gamma,
}

impl TeamLabel {
    pub const ALL: [TeamLabel; 3] = [TeamLabel::Alpha, TeamLabel::Beta, TeamLabel::Gamma];
}

/// 一支队伍（轮休 cohort）：休息一个班次的一批干员。
#[derive(Debug, Clone, Serialize)]
pub struct TeamAssignment {
    pub label: TeamLabel,
    pub operators: Vec<String>,
}

/// 菲亚梅塔在某个班次执行的一次主力回岗覆盖。
#[derive(Debug, Clone, Serialize)]
pub struct FiammettaShiftAction {
    /// 获得换心情并重新回到原岗位的主力。
    pub target: String,
    /// 被主力替换下岗、应进入休息区的原当班干员。
    pub displaced: String,
    /// 主力在 peak 班中的原房间，也是本次替换发生的房间。
    pub room_id: RoomId,
}

/// 单个班次结果：当班两队合起来铺满全部设施。
#[derive(Debug, Clone, Serialize)]
pub struct TeamShiftResult {
    pub index: usize,
    pub duration_hours: f64,
    pub active_teams: Vec<TeamLabel>,
    pub resting_team: TeamLabel,
    pub assignment: BaseAssignment,
    /// 菲亚梅塔使休息队主力额外回岗的单次覆盖；没有可接受目标时为 `None`。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fiammetta: Option<FiammettaShiftAction>,
    pub efficiencies: ShiftEfficiencies,
    /// 贸易效率按时长折算（三类各自独立，不混合量纲）。
    pub weighted_trade: Efficiency,
    /// 制造效率按时长折算。
    pub weighted_manufacture: Efficiency,
    /// 发电效率按时长折算。
    pub weighted_power: Efficiency,
}

/// 三类各自的每日加权产出（贸易/制造/发电分开，不相加）。
#[derive(Debug, Clone, Default, Serialize)]
pub struct DailyTotals {
    pub trade: Efficiency,
    pub manufacture: Efficiency,
    pub power: Efficiency,
}

/// αβγ 三队轮换报告。
#[derive(Debug, Clone, Serialize)]
pub struct TeamRotationReport {
    /// peak 班编排计划（只读；α/β 切半与 γ 贸易 role 填充均据此对齐）。
    pub peak_plan: AssignmentPlan,
    /// 最高效率 peak 编制从满心情工作到首个瓶颈触发的最长时间。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_mood_eta: Option<ShiftEta>,
    pub teams: Vec<TeamAssignment>,
    pub shifts: Vec<TeamShiftResult>,
    /// 三类各自的每日加权产出（12h×αβ + 6h×βγ + 6h×γα，分别汇总）。
    pub daily: DailyTotals,
    pub elapsed: Duration,
}

/// 生产设施一个半区（trade/manu/power 各一组完整房间）。
#[derive(Debug, Clone, Default)]
pub struct FacilityHalf {
    pub trade: Vec<RoomId>,
    pub manu: Vec<RoomId>,
    pub power: Vec<RoomId>,
}

const SUI_MOOD_OPERATORS: [&str; 2] = ["令", "夕"];

fn control_mood_for_shift(
    mood_by_operator: &HashMap<String, f64>,
    active_names: &HashSet<String>,
    default_mood: f64,
) -> f64 {
    SUI_MOOD_OPERATORS
        .iter()
        .filter(|name| active_names.contains(**name))
        .filter_map(|name| mood_by_operator.get(*name).copied())
        .reduce(f64::min)
        .unwrap_or(default_mood)
}

fn advance_sui_mood(
    model: &MoodModel,
    blueprint: &BaseBlueprint,
    assignment: &BaseAssignment,
    resting_names: &HashSet<String>,
    hours: f64,
    mood_by_operator: &mut HashMap<String, f64>,
) {
    let eta = shift_eta(model, blueprint, assignment);
    let drain_by_name: HashMap<&str, f64> = eta
        .per_op
        .iter()
        .map(|op| (op.name.as_str(), op.drain_per_hour))
        .collect();
    for name in SUI_MOOD_OPERATORS {
        let mood = mood_by_operator
            .entry(name.to_string())
            .or_insert(model.mood_cap);
        if let Some(drain) = drain_by_name.get(name) {
            *mood = (*mood - drain * hours).max(0.0);
        } else if resting_names.contains(name) {
            *mood = (*mood + model.dorm_base_recovery(5) * hours).min(model.mood_cap);
        }
    }
}

fn bound_office_operators(
    blueprint: &BaseBlueprint,
    peak: &BaseAssignment,
    binds: &[RuntimeShiftBind],
) -> HashSet<String> {
    binds
        .iter()
        .flat_map(|bind| &bind.operators)
        .filter(|name| {
            peak.rooms.iter().any(|room| {
                blueprint
                    .room(&room.room_id)
                    .is_some_and(|bp| bp.kind == FacilityKind::Office)
                    && room.operators.iter().any(|op| op.name == **name)
            })
        })
        .cloned()
        .collect()
}

fn office_candidates(
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    layout: &LayoutContext,
    mood: f64,
) -> Vec<AssignedOperator> {
    let roster = operbox.roster();
    let mut candidates: Vec<(f64, AssignedOperator)> = roster
        .names()
        .filter_map(|name| {
            let progress = roster.progress(name)?;
            let buff_ids =
                instances.resolve_office_buff_ids(name, PromotionTier::from_progress(progress));
            let score = if buff_ids.is_empty() {
                0.0
            } else {
                solve_office(
                    &OfficeRoomInput {
                        operators: vec![OfficeOperator {
                            name: name.clone(),
                            elite: progress.elite,
                            buff_ids,
                        }],
                        mood,
                        layout: layout.clone(),
                    },
                    table,
                )
                .hire_spd_pct
            };
            Some((score, AssignedOperator::from_progress(name, progress)))
        })
        .collect();
    candidates.sort_by(|(a_score, a), (b_score, b)| {
        b_score
            .partial_cmp(a_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    candidates.into_iter().map(|(_, op)| op).collect()
}

fn assign_rotating_offices(
    assignment: &mut BaseAssignment,
    blueprint: &BaseBlueprint,
    active_teams: &[TeamLabel],
    rotating_team: &HashMap<String, TeamLabel>,
    bound_names: &HashSet<String>,
    candidates: &[AssignedOperator],
) -> Result<()> {
    let mut used = assignment.operator_names();
    let mut active_rotating: Vec<&String> = rotating_team
        .iter()
        .filter(|(_, team)| active_teams.contains(team))
        .map(|(name, _)| name)
        .collect();
    active_rotating.sort_by_key(|name| (!bound_names.contains(*name), name.as_str()));
    for room in blueprint
        .rooms
        .iter()
        .filter(|room| room.kind == FacilityKind::Office)
    {
        let mut operators = assignment.operators_in(&room.id).to_vec();
        while operators.len() < room.operator_capacity() {
            let rotating = active_rotating
                .iter()
                .find(|name| !used.contains(name.as_str()));
            let replacement = candidates
                .iter()
                .find(|op| !used.contains(&op.name) && !rotating_team.contains_key(&op.name));
            let Some(op) = rotating
                .and_then(|name| candidates.iter().find(|op| op.name == name.as_str()))
                .or(replacement)
                .cloned()
            else {
                return Err(Error::msg(format!(
                    "office {} has no legal rotating operator or replacement",
                    room.id.0
                )));
            };
            used.insert(op.name.clone());
            operators.push(op);
        }
        assignment.set_room(room.id.clone(), operators);
    }
    Ok(())
}

fn active_control_candidate_count(
    by_team: &HashMap<TeamLabel, Vec<String>>,
    active_teams: &[TeamLabel],
    unavailable: &HashSet<String>,
) -> usize {
    active_teams
        .iter()
        .flat_map(|team| by_team.get(team).into_iter().flatten())
        .filter(|name| !unavailable.contains(*name))
        .collect::<HashSet<_>>()
        .len()
}

fn least_available_control_team(
    by_team: &HashMap<TeamLabel, Vec<String>>,
    unavailable: &HashSet<String>,
) -> TeamLabel {
    TeamLabel::ALL
        .into_iter()
        .min_by_key(|team| active_control_candidate_count(by_team, &[*team], unavailable))
        .unwrap_or(TeamLabel::Alpha)
}

/// 把全部生产设施（贸易/制造/发电）按同类房间交替切成两半，尽量均衡负载。
fn split_production_facilities(
    blueprint: &BaseBlueprint,
    peak: &BaseAssignment,
    binds: &[RuntimeShiftBind],
) -> Result<[FacilityHalf; 2]> {
    let production_rooms: Vec<RoomId> = blueprint
        .rooms
        .iter()
        .filter(|room| {
            matches!(
                room.kind,
                FacilityKind::TradePost | FacilityKind::Factory | FacilityKind::PowerPlant
            )
        })
        .map(|room| room.id.clone())
        .collect();
    let room_kind: HashMap<_, _> = blueprint
        .rooms
        .iter()
        .map(|room| (room.id.clone(), room.kind))
        .collect();
    let mut components: Vec<Vec<RoomId>> = production_rooms
        .iter()
        .cloned()
        .map(|room| vec![room])
        .collect();
    for bind in binds {
        let mut bound_rooms = Vec::new();
        for name in &bind.operators {
            let room = peak
                .rooms
                .iter()
                .find(|room| room.operators.iter().any(|op| &op.name == name))
                .ok_or_else(|| {
                    Error::msg(format!("shift bind operator {name} missing from peak"))
                })?;
            if production_rooms.contains(&room.room_id) && !bound_rooms.contains(&room.room_id) {
                bound_rooms.push(room.room_id.clone());
            }
        }
        if bound_rooms.len() < 2 {
            continue;
        }
        let mut merged = Vec::new();
        components.retain(|component| {
            if component.iter().any(|room| bound_rooms.contains(room)) {
                merged.extend(component.iter().cloned());
                false
            } else {
                true
            }
        });
        merged.sort_by(|a, b| a.0.cmp(&b.0));
        merged.dedup();
        components.push(merged);
    }
    components.sort_by_key(|component| std::cmp::Reverse(component.len()));
    let mut halves: [FacilityHalf; 2] = Default::default();
    for component in components {
        let load = |half: &FacilityHalf| half.trade.len() + half.manu.len() + half.power.len();
        let target = usize::from(load(&halves[1]) < load(&halves[0]));
        for room in component {
            match room_kind[&room] {
                FacilityKind::TradePost => halves[target].trade.push(room),
                FacilityKind::Factory => halves[target].manu.push(room),
                FacilityKind::PowerPlant => halves[target].power.push(room),
                _ => unreachable!(),
            }
        }
    }
    Ok(halves)
}

/// γ 替补半区：贸易沿用 core role 顺序，制造/发电站绑定搜索。
#[allow(clippy::too_many_arguments)]
fn assign_gamma_half(
    blueprint: &BaseBlueprint,
    instances: &OperatorInstances,
    coexist_assignment: &BaseAssignment,
    durin_dorm_planning: Option<u8>,
    pools: &ProductionPools,
    table: &SkillTable,
    layout: &crate::layout::LayoutContext,
    options: &AssignBaseOptions,
    half: &FacilityHalf,
    assignment: &mut BaseAssignment,
    used: &mut HashSet<String>,
) -> Result<()> {
    assign_team_gamma_half(
        blueprint,
        instances,
        coexist_assignment,
        durin_dorm_planning,
        &pools.trade,
        &pools.manu,
        &pools.power,
        table,
        layout,
        options,
        &half.trade,
        &half.manu,
        &half.power,
        assignment,
        used,
    )
}

fn production_half_from_peak(peak: &BaseAssignment, half: &FacilityHalf) -> BaseAssignment {
    let mut half_assignment = BaseAssignment::default();
    for room_id in half
        .trade
        .iter()
        .chain(half.manu.iter())
        .chain(half.power.iter())
    {
        if let Some(room) = peak.room_assignment(room_id) {
            if !room.operators.is_empty() {
                half_assignment.set_room_assignment(room.clone());
            }
        }
    }
    half_assignment
}

struct ProductionPools {
    trade: crate::pool::TradePool,
    manu: crate::pool::ManuPool,
    power: crate::pool::PowerPool,
}

fn operators_of(assignment: &BaseAssignment) -> Vec<String> {
    let mut names: Vec<String> = assignment
        .rooms
        .iter()
        .flat_map(|r| r.operators.iter().map(|o| o.name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

fn merge_rooms(target: &mut BaseAssignment, source: &BaseAssignment) {
    for room in &source.rooms {
        if room.operators.is_empty() {
            continue;
        }
        target.set_room_assignment(room.clone());
    }
}

fn clear_room(assignment: &mut BaseAssignment, room_id: &str) {
    assignment.rooms.retain(|room| room.room_id.0 != room_id);
}

fn clear_room_efficiency(assignment: &mut BaseAssignment, room_id: &RoomId) {
    if let Some(room) = assignment
        .rooms
        .iter_mut()
        .find(|room| room.room_id == *room_id)
    {
        room.efficiency = None;
    }
}

fn clear_production_efficiencies(blueprint: &BaseBlueprint, assignment: &mut BaseAssignment) {
    for room in &blueprint.rooms {
        if matches!(
            room.kind,
            FacilityKind::TradePost | FacilityKind::Factory | FacilityKind::PowerPlant
        ) {
            clear_room_efficiency(assignment, &room.id);
        }
    }
}

/// 当前轻量策略每个 24h αβγ 周期只安排一次菲亚梅塔回岗。
///
/// 顺序是公孙长乐确认的常规线性 fallback；布局动态排序和龙巫成组服务留给
/// 后续完整心情排班器。
pub const FIAMMETTA_RETURN_PRIORITY: [&str; 5] = ["但书", "巫恋", "龙舌兰", "清流", "可露希尔"];

fn production_efficiency(
    efficiencies: &ShiftEfficiencies,
    kind: FacilityKind,
) -> Option<Efficiency> {
    match kind {
        FacilityKind::TradePost => Some(efficiencies.trade_efficiency),
        FacilityKind::Factory => Some(efficiencies.manufacture_efficiency),
        FacilityKind::PowerPlant => Some(efficiencies.power_efficiency),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_fiammetta_return(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    durin_plan: u8,
    peak: &BaseAssignment,
    teams: &[TeamAssignment],
    shifts: &mut [TeamShiftResult],
) -> Result<()> {
    if !operbox.owns("菲亚梅塔") {
        return Ok(());
    }

    for target_name in FIAMMETTA_RETURN_PRIORITY {
        let Some(source_room) = peak
            .rooms
            .iter()
            .find(|room| room.operators.iter().any(|op| op.name == target_name))
        else {
            continue;
        };
        let Some(room_blueprint) = blueprint.room(&source_room.room_id) else {
            continue;
        };
        if !matches!(
            room_blueprint.kind,
            FacilityKind::TradePost | FacilityKind::Factory | FacilityKind::PowerPlant
        ) {
            continue;
        }
        let Some(target_op) = source_room
            .operators
            .iter()
            .find(|op| op.name == target_name)
            .cloned()
        else {
            continue;
        };
        let Some(target_team) = teams
            .iter()
            .find(|team| team.operators.iter().any(|name| name == target_name))
            .map(|team| team.label)
        else {
            continue;
        };
        let Some(shift_index) = shifts
            .iter()
            .position(|shift| shift.resting_team == target_team)
        else {
            continue;
        };
        if shifts[shift_index]
            .assignment
            .operator_names()
            .contains(target_name)
        {
            continue;
        }

        let hours = shifts[shift_index].duration_hours;
        let Some(current_room) = shifts[shift_index]
            .assignment
            .room_assignment(&source_room.room_id)
        else {
            continue;
        };

        let mut best: Option<(
            BaseAssignment,
            ShiftEfficiencies,
            AssignedOperator,
            Efficiency,
        )> = None;
        for slot in 0..current_room.operators.len() {
            let mut candidate = shifts[shift_index].assignment.clone();
            let Some(room) = candidate
                .rooms
                .iter_mut()
                .find(|room| room.room_id == source_room.room_id)
            else {
                continue;
            };
            let displaced = room.operators[slot].clone();
            room.operators[slot] = target_op.clone();
            room.efficiency = None;

            let Ok(scores) = evaluate_base_assignment_efficiencies(
                blueprint,
                &candidate,
                instances,
                table,
                hours,
                Some(durin_plan),
            ) else {
                continue;
            };
            let candidate_score = production_efficiency(&scores, room_blueprint.kind)
                .expect("production room kind checked above");
            let replace = best
                .as_ref()
                .is_none_or(|(_, _, _, best_score)| candidate_score > *best_score);
            if replace {
                best = Some((candidate, scores, displaced, candidate_score));
            }
        }

        let Some((assignment, scores, displaced, _)) = best else {
            continue;
        };
        let shift = &mut shifts[shift_index];
        shift.assignment = assignment;
        shift.weighted_trade = scores.weighted_trade(hours);
        shift.weighted_manufacture = scores.weighted_manufacture(hours);
        shift.weighted_power = scores.weighted_power(hours);
        shift.efficiencies = scores;
        shift.fiammetta = Some(FiammettaShiftAction {
            target: target_name.to_string(),
            displaced: displaced.name,
            room_id: source_room.room_id.clone(),
        });
        break;
    }

    Ok(())
}

// ── 深海链 S2 短班入口 ──

const ABYSSAL_GLADIIA: &str = "歌蕾蒂娅";
const ABYSSAL_HUNTERS: [&str; 4] = ["乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"];
const ABYSSAL_FORBID_SAME_ROOM_MANU_BUFFS: [&str; 3] = [
    "manu_prod_spd&power[000]",
    "manu_prod_spd&power[010]",
    "manu_prod_spd&power[020]",
];
const TAG_ABYSSAL: &str = "cc.g.abyssal";
const DAIFEEN: &str = "戴菲恩";
#[cfg(test)]
const VINA_TRADE_GROUP: [&str; 3] = ["推进之王", "摩根", "维娜·维多利亚"];
const WARMUP_STICKY_TRADE_OPERATORS: [&str; 2] = ["巫恋", "龙舌兰"];
const WARMUP_MANU_BUFF_PREFIX: &str = "manu_prod_spd_addition[";
const WARMUP_TRADE_BUFF_PREFIX: &str = "trade_ord_wt&cost[";

#[cfg(test)]
fn room_has_all(room: &RoomAssignment, names: &[&str]) -> bool {
    names
        .iter()
        .all(|name| room.operators.iter().any(|op| op.name == *name))
}

fn rooms_compatible_for_swap(
    blueprint: &BaseBlueprint,
    a: &RoomId,
    b: &RoomId,
    kind: FacilityKind,
) -> bool {
    if a == b {
        return true;
    }
    let Some(room_a) = blueprint.room(a) else {
        return false;
    };
    let Some(room_b) = blueprint.room(b) else {
        return false;
    };
    room_a.kind == kind
        && room_b.kind == kind
        && room_a.level == room_b.level
        && room_a.product == room_b.product
        && room_a.product.is_some()
}

#[cfg(test)]
fn trade_room_containing_group(
    assignment: &BaseAssignment,
    blueprint: &BaseBlueprint,
    names: &[&str],
) -> Option<RoomId> {
    assignment.rooms.iter().find_map(|room| {
        let bp_room = blueprint.room(&room.room_id)?;
        (bp_room.kind == FacilityKind::TradePost && room_has_all(room, names))
            .then(|| room.room_id.clone())
    })
}

fn swap_room_assignments(assignment: &mut BaseAssignment, a: &RoomId, b: &RoomId) -> bool {
    let Some(ai) = assignment.rooms.iter().position(|room| &room.room_id == a) else {
        return false;
    };
    let Some(bi) = assignment.rooms.iter().position(|room| &room.room_id == b) else {
        return false;
    };
    if ai == bi {
        return true;
    }
    let ops_a = assignment.rooms[ai].operators.clone();
    let eff_a = assignment.rooms[ai].efficiency.clone();
    assignment.rooms[ai].operators = assignment.rooms[bi].operators.clone();
    assignment.rooms[ai].efficiency = assignment.rooms[bi].efficiency.clone();
    assignment.rooms[bi].operators = ops_a;
    assignment.rooms[bi].efficiency = eff_a;
    true
}

fn room_containing_operator(
    assignment: &BaseAssignment,
    blueprint: &BaseBlueprint,
    name: &str,
    kind: FacilityKind,
) -> Option<RoomId> {
    assignment.rooms.iter().find_map(|room| {
        let bp_room = blueprint.room(&room.room_id)?;
        (bp_room.kind == kind && room.operators.iter().any(|op| op.name == name))
            .then(|| room.room_id.clone())
    })
}

fn is_warmup_operator(
    name: &str,
    tier: PromotionTier,
    kind: FacilityKind,
    instances: &OperatorInstances,
) -> bool {
    match kind {
        FacilityKind::TradePost => {
            if WARMUP_STICKY_TRADE_OPERATORS.contains(&name) {
                return true;
            }
            instances
                .resolve_trade_buff_ids(name, tier)
                .iter()
                .any(|buff_id| buff_id.starts_with(WARMUP_TRADE_BUFF_PREFIX))
        }
        FacilityKind::Factory => instances
            .resolve_manufacture_buff_ids(name, tier)
            .iter()
            .any(|buff_id| buff_id.starts_with(WARMUP_MANU_BUFF_PREFIX)),
        _ => false,
    }
}

/// 暖机类技能跨短班连续上岗时保持同一房间；只能在同设施/同等级/同产物房间之间交换。
fn align_warmup_rooms(
    blueprint: &BaseBlueprint,
    instances: &OperatorInstances,
    assignment: &mut BaseAssignment,
    sticky_rooms: &mut HashMap<String, RoomId>,
) {
    let ops: Vec<(String, FacilityKind)> = assignment
        .rooms
        .iter()
        .filter_map(|room| {
            blueprint
                .room(&room.room_id)
                .map(|bp_room| (bp_room.kind, room.operators.as_slice()))
        })
        .filter(|(kind, _)| matches!(kind, FacilityKind::TradePost | FacilityKind::Factory))
        .flat_map(|(kind, ops)| ops.iter().map(move |op| (op.name.clone(), op.tier(), kind)))
        .filter(|(name, tier, kind)| is_warmup_operator(name, *tier, *kind, instances))
        .map(|(name, _, kind)| (name, kind))
        .collect();

    for (name, kind) in ops {
        let Some(current_room) = room_containing_operator(assignment, blueprint, &name, kind)
        else {
            continue;
        };
        let Some(anchor_room) = sticky_rooms.get(&name).cloned() else {
            sticky_rooms.insert(name, current_room);
            continue;
        };
        if current_room == anchor_room {
            continue;
        }
        if rooms_compatible_for_swap(blueprint, &current_room, &anchor_room, kind) {
            if swap_room_assignments(assignment, &current_room, &anchor_room) {
                clear_room_efficiency(assignment, &current_room);
                clear_room_efficiency(assignment, &anchor_room);
            }
        }
    }
}

struct AbyssalCandidate {
    assignment: BaseAssignment,
    gamma_ops: Vec<String>,
}

/// 构造 S2 深海短班候选：四名深海猎手视作等价生产锚，按房间人数计数枚举。
/// 歌蕾蒂娅只能承受约 7h 短班；深海链不进入普通 base_systems registry。
struct AbyssalBuildCtx<'a> {
    operbox: &'a OperBox,
    instances: &'a OperatorInstances,
    table: &'a SkillTable,
    blueprint: &'a BaseBlueprint,
    layout: &'a LayoutContext,
    options: &'a AssignBaseOptions,
    manu_pool: &'a ManuPool,
    used_ab: &'a HashSet<String>,
    blocked_ops: &'a HashSet<String>,
    shared: &'a BaseAssignment,
    beta: &'a BaseAssignment,
    gamma_h1: &'a BaseAssignment,
    mutable_manu_rooms: &'a [RoomId],
}

fn owned_abyssal_hunters(operbox: &OperBox, used_ab: &HashSet<String>) -> Vec<String> {
    ABYSSAL_HUNTERS
        .iter()
        .filter(|name| operbox.owns(name) && !used_ab.contains(**name))
        .map(|name| (*name).to_string())
        .collect()
}

fn abyssal_manu_entry(
    name: &str,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
) -> Option<ManuPoolEntry> {
    let progress = operbox.progress_of(name)?;
    let tier = PromotionTier::from_progress(progress);
    let buff_ids = instances.resolve_manufacture_buff_ids(name, tier);
    for buff_id in &buff_ids {
        let skill = table.get(buff_id)?;
        if skill.facility != "manufacture" {
            return None;
        }
    }
    let mut tags = instances.tags_for(name, tier);
    if !tags.iter().any(|tag| tag == TAG_ABYSSAL) {
        tags.push(TAG_ABYSSAL.to_string());
    }
    Some(ManuPoolEntry {
        name: name.to_string(),
        elite: progress.elite,
        progress,
        buff_ids,
        tags,
        flat_eff_hint: 0.0,
        has_l2_delegate: false,
        tier: crate::layout::OperatorTier::CrossStation,
    })
}

fn build_abyssal_s2_candidates(ctx: &AbyssalBuildCtx<'_>) -> Vec<AbyssalCandidate> {
    let Some(gladiia_elite) = ctx.operbox.elite_of(ABYSSAL_GLADIIA) else {
        return Vec::new();
    };
    if gladiia_elite < 2 || ctx.used_ab.contains(ABYSSAL_GLADIIA) {
        return Vec::new();
    }

    let hunters = owned_abyssal_hunters(ctx.operbox, ctx.used_ab);
    if hunters.len() < 3 {
        return Vec::new();
    }

    if ctx.mutable_manu_rooms.is_empty() {
        return Vec::new();
    }

    let mut base = ctx.shared.clone();
    merge_rooms(&mut base, ctx.beta);
    merge_rooms(&mut base, ctx.gamma_h1);

    let hunter_entries: Vec<ManuPoolEntry> = hunters
        .iter()
        .filter_map(|name| abyssal_manu_entry(name, ctx.operbox, ctx.instances, ctx.table))
        .collect();
    if hunter_entries.len() != hunters.len() {
        return Vec::new();
    }

    let mut count_vectors = Vec::new();
    enumerate_abyssal_counts(
        hunters.len(),
        ctx.mutable_manu_rooms.len(),
        &mut Vec::new(),
        &mut count_vectors,
    );

    let mut out = Vec::new();
    for counts in count_vectors {
        let mut candidate = base.clone();
        for room_id in ctx.mutable_manu_rooms {
            clear_room(&mut candidate, room_id.0.as_str());
        }
        let mut used = candidate.operator_names();
        used.extend(ctx.blocked_ops.iter().cloned());
        let mut next_hunter = 0;
        let mut ok = true;
        for (room_idx, count) in counts.iter().copied().enumerate() {
            if count == 0 {
                continue;
            }
            let room_id = &ctx.mutable_manu_rooms[room_idx];
            let Some(room) = ctx.blueprint.room(room_id) else {
                ok = false;
                break;
            };
            let Some(RoomProduct::Factory { recipe }) = room.product.as_ref() else {
                ok = false;
                break;
            };
            let anchors = hunter_entries[next_hunter..next_hunter + count].to_vec();
            next_hunter += count;
            if assign_manu_room_with_anchors(
                &mut candidate,
                room_id,
                anchors,
                ctx.manu_pool,
                ctx.table,
                ctx.layout,
                ctx.options,
                *recipe,
                room.level,
                &mut used,
                &ABYSSAL_FORBID_SAME_ROOM_MANU_BUFFS,
            )
            .is_err()
            {
                ok = false;
                break;
            }
        }
        if !ok {
            continue;
        }

        let mut gamma_ops: Vec<String> = candidate
            .rooms
            .iter()
            .filter(|room| ctx.mutable_manu_rooms.contains(&room.room_id))
            .flat_map(|room| room.operators.iter())
            .map(|op| op.name.clone())
            .collect();
        gamma_ops.sort();
        gamma_ops.dedup();
        out.push(AbyssalCandidate {
            assignment: candidate,
            gamma_ops,
        });
    }

    out
}

fn enumerate_abyssal_counts(
    remaining: usize,
    room_count: usize,
    current: &mut Vec<usize>,
    out: &mut Vec<Vec<usize>>,
) {
    if current.len() == room_count {
        if remaining == 0 {
            out.push(current.clone());
        }
        return;
    }
    for count in 0..=remaining.min(3) {
        current.push(count);
        enumerate_abyssal_counts(remaining - count, room_count, current, out);
        current.pop();
    }
}

// ── 深海链短班入口结束 ──

// ── 中枢队伍归属 ──

/// 从编排计划中提取体系绑定的中枢干员名（含 PickOne 候选人）。
fn system_control_operators(plan: &AssignmentPlan) -> HashSet<String> {
    let mut names = HashSet::new();
    names.extend(
        plan.anchors
            .iter()
            .filter(|anchor| anchor.facility == FacilityKind::ControlCenter)
            .map(|anchor| anchor.operator.clone()),
    );
    for sys in &plan.activated {
        let has_production_slot = sys.slots.iter().any(|slot| {
            let facility = match slot {
                SlotFill::Fixed { facility, .. }
                | SlotFill::PickOne { facility, .. }
                | SlotFill::OptionalFixed { facility, .. } => facility,
            };
            matches!(facility, FacilityKind::TradePost | FacilityKind::Factory)
        });
        if !has_production_slot {
            continue;
        }
        for slot in &sys.slots {
            let facility = match slot {
                SlotFill::Fixed { facility, .. }
                | SlotFill::PickOne { facility, .. }
                | SlotFill::OptionalFixed { facility, .. } => facility,
            };
            if *facility != FacilityKind::ControlCenter {
                continue;
            }
            match slot {
                SlotFill::Fixed { operator, .. } | SlotFill::OptionalFixed { operator, .. } => {
                    names.insert(operator.clone());
                }
                SlotFill::PickOne { candidates, .. } => {
                    for c in candidates {
                        names.insert(c.clone());
                    }
                }
            }
        }
    }
    names
}

/// 判断体系的控制中枢干员应归属哪个队伍（基于生产 slot 所在半区）。
/// H1 → Alpha, H2 → Beta；纯中枢体系（无生产 slot）返回 None。
fn system_control_team(sys: &ActivatedSystem, h1: &FacilityHalf) -> Option<TeamLabel> {
    for slot in &sys.slots {
        let (facility, room_id) = match slot {
            SlotFill::Fixed {
                facility, room_id, ..
            }
            | SlotFill::PickOne {
                facility, room_id, ..
            } => (facility, room_id.as_ref()),
            SlotFill::OptionalFixed { .. } => continue,
        };
        let Some(rid) = room_id else {
            continue;
        };
        if !matches!(facility, FacilityKind::TradePost | FacilityKind::Factory) {
            continue;
        }
        if h1.trade.contains(rid) || h1.manu.contains(rid) || h1.power.contains(rid) {
            return Some(TeamLabel::Alpha);
        }
        return Some(TeamLabel::Beta);
    }
    None
}

/// 按"体系归属 + 散件均分"规则将 peak 中枢干员分入 α/β/γ 三队。
///
/// - 体系绑定干员：跟随其生产体系所在半区（H1→α, H2→β；纯中枢体系默认归 α）
/// - 补位散件：按人数最少优先轮询分入三队
fn build_team_control_map(
    peak_ctrl: &[String],
    plan: &AssignmentPlan,
    h1: &FacilityHalf,
) -> HashMap<TeamLabel, Vec<String>> {
    let system_names = system_control_operators(plan);
    let peak_set: HashSet<String> = peak_ctrl.iter().cloned().collect();
    let mut team_map: HashMap<TeamLabel, Vec<String>> = HashMap::new();

    // Step 1: 体系干员归属到对应队伍
    for sys in &plan.activated {
        let team = system_control_team(sys, h1).unwrap_or(TeamLabel::Alpha);
        for slot in &sys.slots {
            let facility = match slot {
                SlotFill::Fixed { facility, .. }
                | SlotFill::PickOne { facility, .. }
                | SlotFill::OptionalFixed { facility, .. } => facility,
            };
            if *facility != FacilityKind::ControlCenter {
                continue;
            }
            let op_name: Option<String> = match slot {
                SlotFill::Fixed { operator, .. } | SlotFill::OptionalFixed { operator, .. } => {
                    if peak_set.contains(operator) {
                        Some(operator.clone())
                    } else {
                        None
                    }
                }
                SlotFill::PickOne { candidates, .. } => {
                    candidates.iter().find(|c| peak_set.contains(*c)).cloned()
                }
            };
            if let Some(name) = op_name {
                team_map.entry(team).or_default().push(name);
            }
        }
    }

    // Step 2: 散件（非体系绑定）按人数最少优先轮询分入三队
    let filler: Vec<String> = peak_ctrl
        .iter()
        .filter(|n| !system_names.contains(*n))
        .cloned()
        .collect();
    let teams = [TeamLabel::Alpha, TeamLabel::Beta, TeamLabel::Gamma];
    for name in filler {
        let t = teams
            .iter()
            .min_by_key(|t| team_map.get(t).map(|v| v.len()).unwrap_or(0))
            .copied()
            .unwrap();
        team_map.entry(t).or_default().push(name);
    }

    team_map
}

fn control_rotation_candidate(
    entry: &crate::pool::ControlPoolEntry,
    _table: &SkillTable,
    _layout: &crate::layout::LayoutContext,
    _mood: f64,
) -> bool {
    control_entry_plugin_fill(entry)
}

fn control_entry_trade_inject(entry: &crate::pool::ControlPoolEntry) -> bool {
    entry
        .buff_ids
        .iter()
        .any(|bid| bid.starts_with("control_tra_spd") || bid == "control_token_tra_spd[000]")
}

fn control_entry_manu_inject(entry: &crate::pool::ControlPoolEntry) -> bool {
    entry.buff_ids.iter().any(|bid| {
        bid.starts_with("control_prod_spd")
            || bid.starts_with("control_token_prod_spd")
            || bid == "control_bd_spd[000]"
            || bid == "control_prod_tra_spd[000]"
    })
}

fn team_control_class_count(
    team_ctrl: &HashMap<TeamLabel, Vec<String>>,
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
    team: TeamLabel,
    class: fn(&crate::pool::ControlPoolEntry) -> bool,
) -> usize {
    team_ctrl
        .get(&team)
        .into_iter()
        .flatten()
        .filter(|name| entry_by_name.get(*name).is_some_and(class))
        .count()
}

fn pick_control_plugin_team(
    entry: &crate::pool::ControlPoolEntry,
    team_ctrl: &HashMap<TeamLabel, Vec<String>>,
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
) -> TeamLabel {
    let class = if control_entry_manu_inject(entry) {
        Some(control_entry_manu_inject as fn(&crate::pool::ControlPoolEntry) -> bool)
    } else if control_entry_trade_inject(entry) {
        Some(control_entry_trade_inject as fn(&crate::pool::ControlPoolEntry) -> bool)
    } else {
        None
    };
    TeamLabel::ALL
        .iter()
        .min_by_key(|team| {
            let class_count = class
                .map(|c| team_control_class_count(team_ctrl, entry_by_name, **team, c))
                .unwrap_or(0);
            let total_count = team_ctrl.get(team).map(|v| v.len()).unwrap_or(0);
            (class_count, total_count)
        })
        .copied()
        .unwrap()
}

fn balance_control_plugin_class(
    team_ctrl: &mut HashMap<TeamLabel, Vec<String>>,
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
    class: fn(&crate::pool::ControlPoolEntry) -> bool,
) {
    loop {
        let uncovered: Vec<_> = TeamLabel::ALL
            .into_iter()
            .filter(|team| team_control_class_count(team_ctrl, entry_by_name, *team, class) == 0)
            .collect();
        if uncovered.is_empty() {
            break;
        }
        let Some(from_team) = TeamLabel::ALL
            .into_iter()
            .filter(|team| team_control_class_count(team_ctrl, entry_by_name, *team, class) > 1)
            .max_by_key(|team| {
                (
                    team_control_class_count(team_ctrl, entry_by_name, *team, class),
                    team_ctrl.get(team).map(|names| names.len()).unwrap_or(0),
                )
            })
        else {
            break;
        };
        let to_team = *uncovered
            .iter()
            .min_by_key(|team| team_ctrl.get(team).map(|names| names.len()).unwrap_or(0))
            .unwrap();
        let Some(move_idx) = team_ctrl.get(&from_team).and_then(|names| {
            names
                .iter()
                .enumerate()
                .filter(|(_, name)| entry_by_name.get(*name).is_some_and(class))
                .max_by(|(_, a), (_, b)| {
                    let aw = entry_by_name
                        .get(*a)
                        .map(control_efficiency_fill_sort_weight)
                        .unwrap_or(0.0);
                    let bw = entry_by_name
                        .get(*b)
                        .map(control_efficiency_fill_sort_weight)
                        .unwrap_or(0.0);
                    aw.partial_cmp(&bw)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| b.cmp(a))
                })
                .map(|(idx, _)| idx)
        }) else {
            break;
        };
        let moved = team_ctrl.get_mut(&from_team).unwrap().remove(move_idx);
        team_ctrl.entry(to_team).or_default().push(moved);
    }
}

fn normalize_control_team_membership(
    team_ctrl: &mut HashMap<TeamLabel, Vec<String>>,
    production_team_by_name: &HashMap<String, TeamLabel>,
) {
    let mut chosen: HashMap<String, TeamLabel> = HashMap::new();
    for team in TeamLabel::ALL {
        for name in team_ctrl.get(&team).into_iter().flatten() {
            let target = production_team_by_name.get(name).copied().unwrap_or(team);
            chosen.entry(name.clone()).or_insert(target);
        }
    }
    for names in team_ctrl.values_mut() {
        names.clear();
    }
    for (name, team) in chosen {
        team_ctrl.entry(team).or_default().push(name);
    }
}

fn control_room_has_class(
    ops: &[AssignedOperator],
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
    class: fn(&crate::pool::ControlPoolEntry) -> bool,
) -> bool {
    ops.iter()
        .any(|op| entry_by_name.get(&op.name).is_some_and(class))
}

fn control_replace_rank(
    op: &AssignedOperator,
    system_ctrl_names: &HashSet<String>,
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
) -> i32 {
    if op.name == ABYSSAL_GLADIIA || op.name == DAIFEEN {
        return -1;
    }
    if op.name == "八幡海铃" {
        return 3;
    }
    if system_ctrl_names.contains(&op.name) {
        return -1;
    }
    let Some(entry) = entry_by_name.get(&op.name) else {
        return 0;
    };
    if control_entry_trade_inject(entry) || control_entry_manu_inject(entry) {
        1
    } else {
        3
    }
}

fn ensure_control_inject_coverage(
    assignment: &mut BaseAssignment,
    final_pool: &crate::pool::ControlPool,
    system_ctrl_names: &HashSet<String>,
    entry_by_name: &HashMap<String, crate::pool::ControlPoolEntry>,
) {
    let mut ops = assignment.control_operators();
    let assigned = assignment.operator_names();
    for class in [
        control_entry_trade_inject as fn(&crate::pool::ControlPoolEntry) -> bool,
        control_entry_manu_inject as fn(&crate::pool::ControlPoolEntry) -> bool,
    ] {
        if control_room_has_class(&ops, entry_by_name, class) {
            continue;
        }
        let Some(candidate) = final_pool
            .entries
            .iter()
            .filter(|entry| class(entry))
            .filter(|entry| {
                !assigned.contains(&entry.name) || ops.iter().any(|op| op.name == entry.name)
            })
            .max_by(|a, b| {
                control_efficiency_fill_sort_weight(a)
                    .partial_cmp(&control_efficiency_fill_sort_weight(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| b.name.cmp(&a.name))
            })
        else {
            continue;
        };
        if ops.iter().any(|op| op.name == candidate.name) {
            continue;
        }
        let Some(drop_idx) = ops
            .iter()
            .enumerate()
            .filter_map(|(idx, op)| {
                let rank = control_replace_rank(op, system_ctrl_names, entry_by_name);
                (rank >= 0).then_some((idx, rank))
            })
            .max_by_key(|(_, rank)| *rank)
            .map(|(idx, _)| idx)
        else {
            continue;
        };
        ops[drop_idx] = AssignedOperator::from_progress(&candidate.name, candidate.progress);
    }
    assignment.set_room(RoomId::from("control"), ops);
}

fn move_control_operator_to_team(
    team_ctrl: &mut HashMap<TeamLabel, Vec<String>>,
    operator: &str,
    team: TeamLabel,
) {
    for names in team_ctrl.values_mut() {
        names.retain(|name| name != operator);
    }
    team_ctrl
        .entry(team)
        .or_default()
        .push(operator.to_string());
}

// ── 中枢队伍归属结束 ──

/// 全基建 αβγ 三队均衡轮休排班（公孙长乐替补池模型）。
///
/// - **设施每班全部满编，绝不空转**：每班由当班两队合力铺满所有贸易/制造/发电站。
/// - 生产设施切成两半 H1/H2：α 跑 H1、β 跑 H2；γ 作为轮换替补，第 2 班接 H1、第 3 班接 H2。
/// - 班次结构 12h + 6h + 6h；每队休息一个班次（α 休 S2、β 休 S3、γ 休 S1）。
/// - 宿舍与未绑定办公室成员共享；绑定办公室成员按消费方 cohort 上二休一。
/// - 中枢按 αβγ 队伍轮休重分配：每班只用活跃两队候选，体系中枢先 pin，再补满 5 人。
pub fn schedule_team_rotation(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &AssignBaseOptions,
) -> Result<TeamRotationReport> {
    let t0 = Instant::now();
    blueprint.validate()?;

    let durin_plan = operbox.durin_dorm_planning_count(instances);

    // 1) 参考高峰班 + 编排计划 → 取宿舍/办公室作为三班共享脚手架。
    // 中枢后续按 αβγ 队伍轮休重分配，不能从 peak 直接钉死。
    // 深海链因歌蕾蒂娅心情消耗 ~3.2/h（最长约 7h），不可进入 12h 主班；
    // 唯一入口是后续 S2 6h 短班的有/无深海双路径评分对比。
    let peak_result = assign_shift_with_plan_skip(
        blueprint,
        operbox,
        instances,
        table,
        options,
        AssignShiftMode::Peak,
        &BaseAssignment::default(),
        &HashSet::new(),
    )?;
    let peak = peak_result.assignment;
    let mut peak_plan = peak_result.plan;
    peak_plan.derive_actual_shift_binds(blueprint, &peak);
    let shift_binds = shift_binds_from_plan(&peak_plan);
    let rotating_office_names = bound_office_operators(blueprint, &peak, &shift_binds);
    let mood_model = MoodModel::load_default()?;
    let peak_mood_eta = Some(shift_eta(&mood_model, blueprint, &peak));
    let shared = pinned_assignment_excluding(&peak, blueprint, &rotating_office_names);
    let scaffold_used: HashSet<String> = operators_of(&shared).into_iter().collect();
    let t1 = Instant::now();

    // 2) 以脚手架解算共享 layout；生产搜索另带 peak 中枢注入，供喀兰等贸易 role 使用。
    let layout = resolve_base(
        blueprint,
        &shared,
        Some(instances),
        Some(table),
        options.mood,
        Some(durin_plan),
    )?
    .layout_snapshot();

    let mut production_seed = shared.clone();
    let production_control = peak.control_operators();
    if !production_control.is_empty() {
        production_seed.set_room("control", production_control);
    }
    let production_layout = resolve_base(
        blueprint,
        &production_seed,
        Some(instances),
        Some(table),
        options.mood,
        Some(durin_plan),
    )?
    .layout_snapshot();

    let mut trade_pool = build_trade_pool(&operbox.trade_roster(instances), instances, table)?;
    if karlan_precision_active(&production_layout.global_inject) {
        add_jie_market_to_trade_pool(&mut trade_pool, instances, table);
    }
    let pools = ProductionPools {
        trade: trade_pool,
        manu: build_manufacture_pool(&operbox.manufacture_roster(instances), instances, table)?,
        power: build_power_pool(&operbox.power_roster(instances), instances, table)?,
    };
    let control_pool = build_control_pool_with_fillers(operbox, instances, table)?;

    // 班次绑定（上2休1）来自统一 plan，不再硬编码具体体系。
    let [h1, h2] = split_production_facilities(blueprint, &peak, &shift_binds)?;

    // 3) α/β 从 peak 编制按 H1/H2 切半（保留编排已认领的 meta 锚点）；γ 走贸易 role 替补。
    let alpha = production_half_from_peak(&peak, &h1);
    let beta = production_half_from_peak(&peak, &h2);
    peak_plan
        .verify_registry_trade_in_alpha_beta(&alpha, &beta)
        .map_err(Error::msg)?;
    let mut used = scaffold_used.clone();
    for name in operators_of(&alpha).into_iter().chain(operators_of(&beta)) {
        used.insert(name);
    }
    let t2 = Instant::now();

    // 4) γ 为替补：S2 接 H1、S3 接 H2，干员与 α/β 互斥（两次各自从剩余池取，可复用同人）。
    let used_ab = used.clone();

    let mut gamma_h1 = BaseAssignment::default();
    let mut used_g1 = used_ab.clone();
    assign_gamma_half(
        blueprint,
        instances,
        &{
            let mut coexist = production_seed.clone();
            merge_rooms(&mut coexist, &beta);
            coexist
        },
        Some(durin_plan),
        &pools,
        table,
        &production_layout,
        options,
        &h1,
        &mut gamma_h1,
        &mut used_g1,
    )?;

    let mut gamma_h2 = BaseAssignment::default();
    let mut used_g2 = used_ab.clone();
    assign_gamma_half(
        blueprint,
        instances,
        &{
            let mut coexist = production_seed.clone();
            merge_rooms(&mut coexist, &alpha);
            coexist
        },
        Some(durin_plan),
        &pools,
        table,
        &production_layout,
        options,
        &h2,
        &mut gamma_h2,
        &mut used_g2,
    )?;
    let t3 = Instant::now();

    // 中枢每班独立分配：体系绑定干员跟随生产队同上同下，散件均分三队，
    // 每班仅允许活跃两队的中枢干员入池（保证每人休息一班）。
    let peak_ctrl: Vec<String> = peak
        .control_operators()
        .iter()
        .map(|o| o.name.clone())
        .collect();
    let mut system_ctrl_names = system_control_operators(&peak_plan);
    system_ctrl_names.extend(
        peak_plan
            .control_candidate_requirements
            .iter()
            .flat_map(|requirement| &requirement.candidates)
            .filter(|name| peak_ctrl.contains(name))
            .cloned(),
    );
    let mut team_ctrl = build_team_control_map(&peak_ctrl, &peak_plan, &h1);
    for sys in &peak_plan.activated {
        let team = system_control_team(sys, &h1).unwrap_or(TeamLabel::Alpha);
        for slot in &sys.slots {
            let facility = match slot {
                SlotFill::Fixed { facility, .. }
                | SlotFill::PickOne { facility, .. }
                | SlotFill::OptionalFixed { facility, .. } => facility,
            };
            if *facility != FacilityKind::ControlCenter {
                continue;
            }
            match slot {
                SlotFill::Fixed { operator, .. } | SlotFill::OptionalFixed { operator, .. } => {
                    if operbox.owns(operator) {
                        team_ctrl.entry(team).or_default().push(operator.clone());
                    }
                }
                SlotFill::PickOne { candidates, .. } => {
                    for candidate in candidates.iter().filter(|name| operbox.owns(name)) {
                        team_ctrl.entry(team).or_default().push(candidate.clone());
                    }
                }
            }
        }
    }
    let mut production_team_by_name: HashMap<String, TeamLabel> = HashMap::new();
    for name in operators_of(&alpha) {
        production_team_by_name
            .entry(name)
            .or_insert(TeamLabel::Alpha);
    }
    for name in operators_of(&beta) {
        production_team_by_name
            .entry(name)
            .or_insert(TeamLabel::Beta);
    }
    for name in operators_of(&gamma_h1)
        .into_iter()
        .chain(operators_of(&gamma_h2))
    {
        production_team_by_name
            .entry(name)
            .or_insert(TeamLabel::Gamma);
    }
    let mut rotating_office_team = HashMap::new();
    for bind in &shift_binds {
        let Some(team) = bind
            .operators
            .iter()
            .find_map(|name| production_team_by_name.get(name).copied())
        else {
            continue;
        };
        for name in bind
            .operators
            .iter()
            .filter(|name| rotating_office_names.contains(*name))
        {
            rotating_office_team.insert(name.clone(), team);
        }
    }
    let entry_by_name: HashMap<String, crate::pool::ControlPoolEntry> = control_pool
        .entries
        .iter()
        .map(|entry| (entry.name.clone(), entry.clone()))
        .collect();
    let mut plugin_entries: Vec<_> = control_pool
        .entries
        .iter()
        .filter(|entry| !team_ctrl.values().any(|names| names.contains(&entry.name)))
        .filter(|entry| !production_team_by_name.contains_key(&entry.name))
        .filter(|entry| control_rotation_candidate(entry, table, &layout, options.mood))
        .collect();
    plugin_entries.sort_by(|a, b| {
        control_efficiency_fill_sort_weight(b)
            .partial_cmp(&control_efficiency_fill_sort_weight(a))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });
    for entry in plugin_entries {
        if team_ctrl.values().any(|names| names.contains(&entry.name)) {
            continue;
        }
        let team = production_team_by_name
            .get(&entry.name)
            .copied()
            .unwrap_or_else(|| pick_control_plugin_team(entry, &team_ctrl, &entry_by_name));
        team_ctrl.entry(team).or_default().push(entry.name.clone());
    }
    balance_control_plugin_class(&mut team_ctrl, &entry_by_name, control_entry_trade_inject);
    balance_control_plugin_class(&mut team_ctrl, &entry_by_name, control_entry_manu_inject);
    normalize_control_team_membership(&mut team_ctrl, &production_team_by_name);
    // 跨设施 shift bind 是最终归属约束，不能被中枢均衡策略再次拆开。
    for bind in &shift_binds {
        let Some(team) = bind
            .operators
            .iter()
            .find_map(|name| production_team_by_name.get(name).copied())
        else {
            continue;
        };
        for name in &bind.operators {
            if peak_ctrl.contains(name) {
                move_control_operator_to_team(&mut team_ctrl, name, team);
            }
        }
    }
    for names in team_ctrl.values_mut() {
        names.sort();
        names.dedup();
    }
    let unavailable_for_control: HashSet<String> = production_team_by_name
        .keys()
        .cloned()
        .chain(scaffold_used.iter().cloned())
        .chain(rotating_office_names.iter().cloned())
        .collect();
    let active_pair_has_five = |by_team: &HashMap<TeamLabel, Vec<String>>| {
        TeamLabel::ALL.iter().all(|resting| {
            let active: Vec<_> = TeamLabel::ALL
                .iter()
                .filter(|team| *team != resting)
                .copied()
                .collect();
            active_control_candidate_count(by_team, &active, &unavailable_for_control) >= 5
        })
    };
    let mut assigned_control: HashSet<String> = team_ctrl.values().flatten().cloned().collect();
    let mut skillless_fillers: Vec<_> = control_pool
        .entries
        .iter()
        .filter(|entry| entry.buff_ids.is_empty())
        .filter(|entry| !assigned_control.contains(&entry.name))
        .filter(|entry| !production_team_by_name.contains_key(&entry.name))
        .filter(|entry| !scaffold_used.contains(&entry.name))
        .filter(|entry| !rotating_office_names.contains(&entry.name))
        .collect();
    skillless_fillers.sort_by(|a, b| a.name.cmp(&b.name));
    for entry in skillless_fillers {
        if active_pair_has_five(&team_ctrl) {
            break;
        }
        let team = least_available_control_team(&team_ctrl, &unavailable_for_control);
        team_ctrl.entry(team).or_default().push(entry.name.clone());
        assigned_control.insert(entry.name.clone());
    }
    let office_pool = office_candidates(operbox, instances, table, &layout, options.mood);
    let mut reserved: HashSet<String> = operators_of(&alpha)
        .into_iter()
        .chain(operators_of(&beta))
        .chain(operators_of(&gamma_h1))
        .chain(operators_of(&gamma_h2))
        .chain(team_ctrl.values().flatten().cloned())
        .chain(operators_of(&shared))
        .chain(rotating_office_names.iter().cloned())
        .collect();
    let bound_office_teams: Vec<TeamLabel> = rotating_office_names
        .iter()
        .filter_map(|name| rotating_office_team.get(name).copied())
        .collect();
    for bound_team in bound_office_teams {
        let replacement = office_pool.iter().find(|op| !reserved.contains(&op.name));
        let replacement_team = TeamLabel::ALL.into_iter().find(|team| *team != bound_team);
        match (replacement, replacement_team) {
            (Some(op), Some(team)) => {
                rotating_office_team.insert(op.name.clone(), team);
                reserved.insert(op.name.clone());
            }
            _ => {
                return Err(Error::msg(
                    "rotating office has no legal replacement cohort",
                ))
            }
        }
    }

    // 队伍花名册（cohort）。生产干员 + 中枢归队干员共同构成轮休队伍。
    let mut alpha_ops = operators_of(&alpha);
    alpha_ops.extend(
        team_ctrl
            .get(&TeamLabel::Alpha)
            .cloned()
            .unwrap_or_default(),
    );
    alpha_ops.sort();
    alpha_ops.dedup();

    let mut beta_ops = operators_of(&beta);
    beta_ops.extend(team_ctrl.get(&TeamLabel::Beta).cloned().unwrap_or_default());
    beta_ops.sort();
    beta_ops.dedup();

    let mut gamma_ops: Vec<String> = operators_of(&gamma_h1);
    gamma_ops.extend(operators_of(&gamma_h2));
    gamma_ops.extend(
        team_ctrl
            .get(&TeamLabel::Gamma)
            .cloned()
            .unwrap_or_default(),
    );
    gamma_ops.sort();
    gamma_ops.dedup();
    for (name, team) in &rotating_office_team {
        match team {
            TeamLabel::Alpha => alpha_ops.push(name.clone()),
            TeamLabel::Beta => beta_ops.push(name.clone()),
            TeamLabel::Gamma => gamma_ops.push(name.clone()),
        }
    }
    alpha_ops.sort();
    alpha_ops.dedup();
    beta_ops.sort();
    beta_ops.dedup();
    gamma_ops.sort();
    gamma_ops.dedup();

    let mut teams = vec![
        TeamAssignment {
            label: TeamLabel::Alpha,
            operators: alpha_ops,
        },
        TeamAssignment {
            label: TeamLabel::Beta,
            operators: beta_ops,
        },
        TeamAssignment {
            label: TeamLabel::Gamma,
            operators: gamma_ops,
        },
    ];

    // 5) 组装三班（每班满编）并评分：
    //    S1(12h)=脚手架+α(H1)+β(H2)；S2(6h)=脚手架+β(H2)+γ(H1)；S3(6h)=脚手架+α(H1)+γ(H2)。
    //
    //    S2 深海链双路径对比：预占猎手 → γ fill 绕开 → 补第3人 → 评分，与无深海基线对比，选优胜。
    let shift_specs: [(f64, [TeamLabel; 2], TeamLabel, [&BaseAssignment; 2]); 3] = [
        (
            12.0,
            [TeamLabel::Alpha, TeamLabel::Beta],
            TeamLabel::Gamma,
            [&alpha, &beta],
        ),
        (
            6.0,
            [TeamLabel::Beta, TeamLabel::Gamma],
            TeamLabel::Alpha,
            [&beta, &gamma_h1],
        ),
        (
            6.0,
            [TeamLabel::Gamma, TeamLabel::Alpha],
            TeamLabel::Beta,
            [&gamma_h2, &alpha],
        ),
    ];

    let mut shifts = Vec::with_capacity(3);
    let mut control_options = options.clone();
    control_options.skip_standalone_control = true;
    let mut sui_mood: HashMap<String, f64> = SUI_MOOD_OPERATORS
        .into_iter()
        .map(|name| (name.to_string(), options.mood.min(mood_model.mood_cap)))
        .collect();
    let mut warmup_sticky_rooms: HashMap<String, RoomId> = HashMap::new();
    for (index, (hours, active, resting, parts)) in shift_specs.into_iter().enumerate() {
        // 活跃两队的中枢干员名册
        let active_names: HashSet<String> = active
            .iter()
            .flat_map(|t| team_ctrl.get(t).cloned().unwrap_or_default())
            .collect();
        control_options.mood = control_mood_for_shift(&sui_mood, &active_names, options.mood);
        // 从全池按名提取活跃队中枢干员；体系中枢干员可能不在 standalone 中枢白名单，
        // 但已经由 base_systems/peak_plan 验证可用，因此要按名补回。
        // 所有中枢候选已在进入三班前归入 α/β/γ；这里不再用全局池补位，
        // 避免未归队散件绕过轮休、三班连续上岗。
        let mut team_entries: Vec<crate::pool::ControlPoolEntry> = control_pool
            .entries
            .iter()
            .filter(|e| active_names.contains(&e.name))
            .cloned()
            .collect();
        let present_entries: HashSet<String> =
            team_entries.iter().map(|e| e.name.clone()).collect();
        for name in active_names
            .iter()
            .filter(|n| system_ctrl_names.contains(*n) && !present_entries.contains(*n))
        {
            let progress = operbox.progress_of(name).unwrap_or_default();
            team_entries.push(crate::pool::ControlPoolEntry {
                name: name.clone(),
                elite: progress.elite,
                progress,
                buff_ids: instances.resolve_control_buff_ids(
                    name,
                    crate::tier::PromotionTier::from_progress(progress),
                ),
                tags: vec![],
                tier: crate::layout::tier::OperatorTier::CrossStation,
            });
        }
        let base_control_pool = crate::pool::ControlPool {
            entries: team_entries,
            skipped: vec![],
        };

        // 体系中枢干员是当班硬锚点：先 pin 到 control，再由 assign_control 补满 5 人。
        // 这样薇薇安娜/夕等不在 standalone 中枢白名单内的体系位不会被搜索阶段丢掉。
        let pin_active_system_control = |a: &mut BaseAssignment, extra_control_pins: &[&str]| {
            let previous = a.control_operators();
            let mut required: Vec<String> = active_names
                .iter()
                .filter(|name| system_ctrl_names.contains(*name))
                .cloned()
                .collect();
            required.extend(extra_control_pins.iter().map(|name| (*name).to_string()));
            required.sort();
            required.dedup();
            let mut ops = Vec::new();
            for name in &required {
                let progress = operbox.progress_of(name).unwrap_or_default();
                ops.push(AssignedOperator::from_progress(name, progress));
            }
            if ops.len() > 5 {
                return Err(Error::msg(format!(
                    "active required control anchors exceed capacity: {:?}",
                    required
                )));
            }
            for op in previous {
                if ops.len() >= 5 {
                    break;
                }
                if !required.contains(&op.name) {
                    ops.push(op);
                }
            }
            let mut room_names: HashSet<String> = ops.iter().map(|o| o.name.clone()).collect();
            let assigned_names = a.operator_names();
            for name in extra_control_pins {
                if ops.len() >= 5 {
                    break;
                }
                if room_names.contains(*name) || assigned_names.contains(*name) {
                    continue;
                }
                let Some(progress) = operbox.progress_of(name) else {
                    continue;
                };
                ops.push(AssignedOperator::from_progress(*name, progress));
                room_names.insert((*name).to_string());
            }
            for name in active_names
                .iter()
                .filter(|n| system_ctrl_names.contains(*n))
            {
                if ops.len() >= 5 {
                    break;
                }
                if room_names.contains(name) || assigned_names.contains(name) {
                    continue;
                }
                let op = operbox
                    .progress_of(name)
                    .map(|progress| AssignedOperator::from_progress(name, progress))
                    .unwrap_or_else(|| AssignedOperator::new(name, 0));
                ops.push(op);
                room_names.insert(name.clone());
            }
            if !ops.is_empty() {
                a.set_room(RoomId::from("control"), ops);
            }
            Ok::<(), Error>(())
        };

        // 从队池分配中枢（池小不报错，有多少填多少）
        let assign_ctrl = |a: &mut BaseAssignment,
                           used: &mut HashSet<String>,
                           extra_control_pins: &[&str]| {
            pin_active_system_control(a, extra_control_pins)?;
            *used = a.operator_names();
            let mut final_pool = base_control_pool.clone();
            let present: HashSet<String> =
                final_pool.entries.iter().map(|e| e.name.clone()).collect();
            for op in a.control_operators() {
                if present.contains(&op.name) {
                    continue;
                }
                final_pool.entries.push(crate::pool::ControlPoolEntry {
                    name: op.name.clone(),
                    elite: op.elite,
                    progress: crate::roster::OperatorProgress::new(op.elite, op.level, op.rarity),
                    buff_ids: instances.resolve_control_buff_ids(&op.name, op.tier()),
                    tags: vec![],
                    tier: crate::layout::tier::OperatorTier::CrossStation,
                });
            }
            assign_control(a, &final_pool, table, &layout, &control_options, &[], used)?;
            ensure_control_inject_coverage(a, &final_pool, &system_ctrl_names, &entry_by_name);

            let mut ops = a.control_operators();
            if ops.len() < 5 {
                let mut names: HashSet<String> = ops.iter().map(|o| o.name.clone()).collect();
                let assigned = a.operator_names();
                let mut entries = final_pool.entries.iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| {
                    control_efficiency_fill_sort_weight(b)
                        .partial_cmp(&control_efficiency_fill_sort_weight(a))
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });
                for entry in entries {
                    if ops.len() >= 5 {
                        break;
                    }
                    if names.contains(&entry.name) || assigned.contains(&entry.name) {
                        continue;
                    }
                    ops.push(AssignedOperator::from_progress(&entry.name, entry.progress));
                    names.insert(entry.name.clone());
                }
                a.set_room(RoomId::from("control"), ops);
                *used = a.operator_names();
            }
            Ok::<(), Error>(())
        };

        let mut assignment;
        if index == 1 {
            // ── S2 双路径对比 ──
            // 路径 A: 无深海（基线）
            let mut base = shared.clone();
            for part in parts {
                merge_rooms(&mut base, part);
            }
            assign_rotating_offices(
                &mut base,
                blueprint,
                &active,
                &rotating_office_team,
                &rotating_office_names,
                &office_pool,
            )?;
            clear_room(&mut base, "control");
            let mut base_used = base.operator_names();
            assign_ctrl(&mut base, &mut base_used, &[])?;
            let mut base_warmup_rooms = warmup_sticky_rooms.clone();
            align_warmup_rooms(blueprint, instances, &mut base, &mut base_warmup_rooms);
            clear_production_efficiencies(blueprint, &mut base);
            let score_base = evaluate_base_assignment_efficiencies(
                blueprint,
                &base,
                instances,
                table,
                hours,
                Some(durin_plan),
            )?;

            // 路径 B: 有深海（S2 短班候选）
            let alpha_blocked_ops: HashSet<String> = operators_of(&alpha).into_iter().collect();
            let abyssal_candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
                operbox,
                instances,
                table,
                blueprint,
                layout: &production_layout,
                options,
                manu_pool: &pools.manu,
                used_ab: &used_ab,
                blocked_ops: &alpha_blocked_ops,
                shared: &shared,
                beta: &beta,
                gamma_h1: &gamma_h1,
                mutable_manu_rooms: &h1.manu,
            });
            let mut best_abyssal: Option<(
                AbyssalCandidate,
                ShiftEfficiencies,
                HashMap<String, RoomId>,
            )> = None;
            for mut candidate in abyssal_candidates {
                assign_rotating_offices(
                    &mut candidate.assignment,
                    blueprint,
                    &active,
                    &rotating_office_team,
                    &rotating_office_names,
                    &office_pool,
                )?;
                clear_room(&mut candidate.assignment, "control");
                let mut aby_used = candidate.assignment.operator_names();
                assign_ctrl(&mut candidate.assignment, &mut aby_used, &[ABYSSAL_GLADIIA])?;
                let mut candidate_warmup_rooms = warmup_sticky_rooms.clone();
                align_warmup_rooms(
                    blueprint,
                    instances,
                    &mut candidate.assignment,
                    &mut candidate_warmup_rooms,
                );
                if !candidate
                    .assignment
                    .control_operators()
                    .iter()
                    .any(|o| o.name == ABYSSAL_GLADIIA)
                {
                    return Err(Error::msg("abyssal S2 candidate lost Gladiia control pin"));
                }
                clear_production_efficiencies(blueprint, &mut candidate.assignment);
                let score_aby = evaluate_base_assignment_efficiencies(
                    blueprint,
                    &candidate.assignment,
                    instances,
                    table,
                    hours,
                    Some(durin_plan),
                )?;
                let replace = best_abyssal.as_ref().is_none_or(|(_, best, _)| {
                    score_aby.manufacture_efficiency > best.manufacture_efficiency
                });
                if replace {
                    best_abyssal = Some((candidate, score_aby, candidate_warmup_rooms));
                }
            }
            let (best_assignment, best_scores, best_warmup_rooms) =
                if let Some((candidate, score_aby, candidate_warmup_rooms)) = best_abyssal {
                    if score_aby.manufacture_efficiency > score_base.manufacture_efficiency {
                        let alpha_beta: HashSet<String> = teams
                            .iter()
                            .filter(|team| matches!(team.label, TeamLabel::Alpha | TeamLabel::Beta))
                            .flat_map(|team| team.operators.iter().cloned())
                            .collect();
                        if let Some(team) =
                            teams.iter_mut().find(|team| team.label == TeamLabel::Gamma)
                        {
                            let mut ops = team.operators.clone();
                            ops.extend(candidate.gamma_ops.clone());
                            ops.push(ABYSSAL_GLADIIA.to_string());
                            ops.sort();
                            ops.dedup();
                            ops.retain(|name| !alpha_beta.contains(name));
                            team.operators = ops;
                        }
                        (candidate.assignment, score_aby, candidate_warmup_rooms)
                    } else {
                        (base, score_base, base_warmup_rooms)
                    }
                } else {
                    (base, score_base, base_warmup_rooms)
                };
            assignment = best_assignment;
            let scores = best_scores;
            warmup_sticky_rooms = best_warmup_rooms;
            let weighted_trade = scores.weighted_trade(hours);
            let weighted_manufacture = scores.weighted_manufacture(hours);
            let weighted_power = scores.weighted_power(hours);
            shifts.push(TeamShiftResult {
                index,
                duration_hours: hours,
                active_teams: active.to_vec(),
                resting_team: resting,
                assignment,
                fiammetta: None,
                efficiencies: scores,
                weighted_trade,
                weighted_manufacture,
                weighted_power,
            });
        } else {
            // ── S1 / S3 正常组装 ──
            assignment = shared.clone();
            for part in parts {
                merge_rooms(&mut assignment, part);
            }
            assign_rotating_offices(
                &mut assignment,
                blueprint,
                &active,
                &rotating_office_team,
                &rotating_office_names,
                &office_pool,
            )?;
            clear_room(&mut assignment, "control");
            let mut s13_used = assignment.operator_names();
            assign_ctrl(&mut assignment, &mut s13_used, &[])?;
            align_warmup_rooms(
                blueprint,
                instances,
                &mut assignment,
                &mut warmup_sticky_rooms,
            );
            clear_production_efficiencies(blueprint, &mut assignment);
            let scores = evaluate_base_assignment_efficiencies(
                blueprint,
                &assignment,
                instances,
                table,
                hours,
                Some(durin_plan),
            )?;
            let weighted_trade = scores.weighted_trade(hours);
            let weighted_manufacture = scores.weighted_manufacture(hours);
            let weighted_power = scores.weighted_power(hours);
            shifts.push(TeamShiftResult {
                index,
                duration_hours: hours,
                active_teams: active.to_vec(),
                resting_team: resting,
                assignment,
                fiammetta: None,
                efficiencies: scores,
                weighted_trade,
                weighted_manufacture,
                weighted_power,
            });
        }
        let resting_names: HashSet<String> = teams
            .iter()
            .find(|team| team.label == resting)
            .map(|team| team.operators.iter().cloned().collect())
            .unwrap_or_default();
        let assignment = &shifts.last().expect("shift just appended").assignment;
        advance_sui_mood(
            &mood_model,
            blueprint,
            assignment,
            &resting_names,
            hours,
            &mut sui_mood,
        );
    }

    apply_fiammetta_return(
        blueprint,
        operbox,
        instances,
        table,
        durin_plan,
        &peak,
        &teams,
        &mut shifts,
    )?;
    let daily = DailyTotals {
        trade: shifts.iter().map(|shift| shift.weighted_trade).sum(),
        manufacture: shifts.iter().map(|shift| shift.weighted_manufacture).sum(),
        power: shifts.iter().map(|shift| shift.weighted_power).sum(),
    };
    let t4 = Instant::now();

    fn ms(a: Instant, b: Instant) -> f64 {
        a.duration_since(b).as_secs_f64() * 1000.0
    }

    eprintln!(
        "[计时] 轮换·peak班={:.2}ms  resolve+建池+切半={:.2}ms  γ替补={:.2}ms  三班评分={:.2}ms  轮换总计={:.2}ms",
        ms(t1, t0), ms(t2, t1), ms(t3, t2), ms(t4, t3), ms(t4, t0),
    );

    Ok(TeamRotationReport {
        peak_plan,
        peak_mood_eta,
        teams,
        shifts,
        daily,
        elapsed: t4.duration_since(t0),
    })
}

/// 干员 → 所属队伍 的查表（输出层给每个设施打队伍标签用）。
pub fn operator_team_map(report: &TeamRotationReport) -> HashMap<String, TeamLabel> {
    let mut map = HashMap::new();
    for team in &report.teams {
        for op in &team.operators {
            map.entry(op.clone()).or_insert(team.label);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::default_instances_path;
    use crate::layout::{assign_shift, blackkey_witch_same_trade_room};
    use crate::operbox::{
        default_operbox_full_e2_path, default_operbox_gongsun_path, OperBoxEntry,
    };

    #[test]
    fn sui_control_mood_uses_active_operator_state() {
        let moods = HashMap::from([("令".to_string(), 11.5), ("夕".to_string(), 18.0)]);
        let active = HashSet::from(["令".to_string()]);
        assert_eq!(control_mood_for_shift(&moods, &active, 24.0), 11.5);
    }

    #[test]
    fn sui_mood_propagates_work_and_rest_between_shifts() {
        let blueprint: BaseBlueprint = serde_json::from_str(
            r#"{"rooms":[{"id":"control","kind":"control_center","level":5}]}"#,
        )
        .expect("blueprint");
        let model = MoodModel::load_default().expect("mood model");
        let mut assignment = BaseAssignment::default();
        assignment.set_room("control", vec![AssignedOperator::new("令", 2)]);
        let mut moods = HashMap::from([("令".to_string(), 24.0), ("夕".to_string(), 12.0)]);

        advance_sui_mood(
            &model,
            &blueprint,
            &assignment,
            &HashSet::from(["夕".to_string()]),
            6.0,
            &mut moods,
        );

        assert!(moods["令"] < 24.0, "在岗令应消耗心情");
        assert_eq!(moods["夕"], 24.0, "休息夕应按宿舍基础回复并封顶");
    }

    #[test]
    fn rotating_office_uses_bound_member_then_replacement() {
        let blueprint: BaseBlueprint =
            serde_json::from_str(r#"{"rooms":[{"id":"office_any","kind":"office","level":3}]}"#)
                .expect("blueprint");
        let candidates = vec![
            AssignedOperator::new("体系办公室", 2),
            AssignedOperator::new("办公室替补", 2),
        ];
        let teams = HashMap::from([
            ("体系办公室".to_string(), TeamLabel::Alpha),
            ("办公室替补".to_string(), TeamLabel::Beta),
        ]);
        let bound = HashSet::from(["体系办公室".to_string()]);

        let mut active = BaseAssignment::default();
        assign_rotating_offices(
            &mut active,
            &blueprint,
            &[TeamLabel::Alpha, TeamLabel::Beta],
            &teams,
            &bound,
            &candidates,
        )
        .expect("active office");
        assert_eq!(
            active.operators_in(&RoomId::from("office_any"))[0].name,
            "体系办公室"
        );

        let mut resting = BaseAssignment::default();
        assign_rotating_offices(
            &mut resting,
            &blueprint,
            &[TeamLabel::Beta, TeamLabel::Gamma],
            &teams,
            &bound,
            &candidates,
        )
        .expect("replacement office");
        assert_eq!(
            resting.operators_in(&RoomId::from("office_any"))[0].name,
            "办公室替补"
        );
    }

    #[test]
    fn control_coverage_does_not_count_production_or_shared_members() {
        let by_team = HashMap::from([
            (
                TeamLabel::Alpha,
                vec![
                    "可用甲".to_string(),
                    "生产占用".to_string(),
                    "共享占用".to_string(),
                ],
            ),
            (
                TeamLabel::Beta,
                vec!["可用乙".to_string(), "绑定办公室".to_string()],
            ),
        ]);
        let unavailable = HashSet::from([
            "生产占用".to_string(),
            "共享占用".to_string(),
            "绑定办公室".to_string(),
        ]);

        assert_eq!(
            active_control_candidate_count(
                &by_team,
                &[TeamLabel::Alpha, TeamLabel::Beta],
                &unavailable,
            ),
            2
        );
    }

    #[test]
    fn control_filler_targets_least_available_not_shortest_raw_roster() {
        let by_team = HashMap::from([
            (
                TeamLabel::Alpha,
                vec![
                    "占用甲".to_string(),
                    "占用乙".to_string(),
                    "占用丙".to_string(),
                ],
            ),
            (TeamLabel::Beta, vec!["可用乙".to_string()]),
            (TeamLabel::Gamma, vec!["可用丙".to_string()]),
        ]);
        let unavailable = HashSet::from([
            "占用甲".to_string(),
            "占用乙".to_string(),
            "占用丙".to_string(),
        ]);

        assert_eq!(
            least_available_control_team(&by_team, &unavailable),
            TeamLabel::Alpha,
            "A raw roster 最长但真实可用为0，filler必须优先投A"
        );
    }

    use crate::skill_table::default_skill_table_path;

    fn fixtures() -> (BaseBlueprint, OperBox, OperatorInstances, SkillTable) {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let operbox = OperBox::load(&default_operbox_full_e2_path().unwrap())
            .or_else(|_| OperBox::load(&default_operbox_gongsun_path().unwrap()))
            .unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        (blueprint, operbox, instances, table)
    }

    fn assert_pinus_rotation(
        report: &TeamRotationReport,
        blueprint: &BaseBlueprint,
        manufacturers: &[&str],
    ) {
        use crate::schedule::shift_bind::team_of_operator;

        let mut bound = vec!["焰尾", "薇薇安娜"];
        bound.extend_from_slice(manufacturers);
        let team = team_of_operator(report, "焰尾").expect("焰尾应进入轮换队");
        for name in &bound {
            assert_eq!(
                team_of_operator(report, name),
                Some(team),
                "红松核心应同队上二休一: {name}"
            );
        }

        let mut active_shifts = 0;
        for shift in &report.shifts {
            let active = shift.resting_team != team;
            active_shifts += usize::from(active);
            for name in &bound {
                let room = shift
                    .assignment
                    .rooms
                    .iter()
                    .find(|room| room.operators.iter().any(|op| op.name == *name));
                if !active {
                    assert!(room.is_none(), "休息班不应安排红松核心: {name}");
                    continue;
                }
                let room = room.unwrap_or_else(|| panic!("工作班缺少红松核心: {name}"));
                let room_blueprint = blueprint.room(&room.room_id).expect("room exists");
                if manufacturers.contains(name) {
                    assert!(
                        matches!(
                            room_blueprint.product,
                            Some(crate::layout::RoomProduct::Factory {
                                recipe: crate::types::RecipeKind::BattleRecord
                            })
                        ),
                        "红松制造成员只能进入作战记录站: {name}"
                    );
                } else {
                    assert_eq!(
                        room_blueprint.kind,
                        FacilityKind::ControlCenter,
                        "中枢双核心必须实际进入中枢: {name}"
                    );
                }
            }
        }
        assert_eq!(active_shifts, 2, "红松核心应工作两班、休息一班");
    }

    fn build_test_manu_ctx(
        blueprint: &BaseBlueprint,
        operbox: &OperBox,
        instances: &OperatorInstances,
        table: &SkillTable,
    ) -> (LayoutContext, ManuPool, AssignBaseOptions) {
        let layout = resolve_base(
            blueprint,
            &BaseAssignment::default(),
            Some(instances),
            Some(table),
            24.0,
            None,
        )
        .unwrap()
        .layout_snapshot();
        let manu_pool =
            build_manufacture_pool(&operbox.manufacture_roster(instances), instances, table)
                .unwrap();
        (layout, manu_pool, AssignBaseOptions::default())
    }

    fn trade_room_contains(
        assignment: &BaseAssignment,
        blueprint: &BaseBlueprint,
        names: &[&str],
    ) -> bool {
        assignment.rooms.iter().any(|room| {
            blueprint
                .room(&room.room_id)
                .is_some_and(|bp| bp.kind == FacilityKind::TradePost)
                && names
                    .iter()
                    .all(|name| room.operators.iter().any(|op| op.name == *name))
        })
    }

    #[test]
    fn abyssal_candidate_does_not_materialize_empty_factory_rooms() {
        let (blueprint, operbox, instances, table) = fixtures();
        if !["歌蕾蒂娅", "乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"]
            .iter()
            .all(|name| operbox.owns(name))
        {
            return;
        }
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);

        let shared = BaseAssignment::default();
        let beta = BaseAssignment::default();
        let mut gamma_h1 = BaseAssignment::default();
        gamma_h1.set_room(
            "manu_1",
            vec![
                AssignedOperator::new("芬", 0),
                AssignedOperator::new("克洛丝", 0),
                AssignedOperator::new("泡普卡", 0),
            ],
        );

        let blocked_ops = HashSet::new();
        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1"), RoomId::from("manu_3")],
        });

        assert!(!candidates.is_empty());
        for candidate in candidates {
            for room in &candidate.assignment.rooms {
                let is_empty_factory = blueprint
                    .room(&room.room_id)
                    .is_some_and(|bp| bp.kind == FacilityKind::Factory)
                    && room.operators.is_empty();
                assert!(
                    !is_empty_factory,
                    "深海候选不应把未改动制造站写成显式空房: {:?}",
                    room
                );
            }
        }
    }

    #[test]
    fn abyssal_candidate_only_replaces_mutable_gamma_rooms() {
        let (blueprint, operbox, instances, table) = fixtures();
        if !["歌蕾蒂娅", "乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"]
            .iter()
            .all(|name| operbox.owns(name))
        {
            return;
        }
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);

        let shared = BaseAssignment::default();
        let mut beta = BaseAssignment::default();
        beta.set_room(
            "manu_2",
            vec![
                AssignedOperator::new("清流", 2),
                AssignedOperator::new("温蒂", 2),
                AssignedOperator::new("森蚺", 2),
            ],
        );
        let mut gamma_h1 = BaseAssignment::default();
        gamma_h1.set_room(
            "manu_1",
            vec![
                AssignedOperator::new("芬", 0),
                AssignedOperator::new("克洛丝", 0),
                AssignedOperator::new("泡普卡", 0),
            ],
        );
        gamma_h1.set_room(
            "manu_3",
            vec![
                AssignedOperator::new("斑点", 0),
                AssignedOperator::new("米格鲁", 0),
                AssignedOperator::new("玫兰莎", 0),
            ],
        );

        let blocked_ops = HashSet::new();
        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1"), RoomId::from("manu_3")],
        });

        assert!(!candidates.is_empty());
        for candidate in candidates {
            let beta_names: Vec<_> = candidate
                .assignment
                .operators_in(&RoomId::from("manu_2"))
                .iter()
                .map(|op| op.name.as_str())
                .collect();
            assert_eq!(
                beta_names,
                vec!["清流", "温蒂", "森蚺"],
                "深海候选不应替换 β/活跃体系制造房"
            );
        }
    }

    #[test]
    fn abyssal_candidate_does_not_fill_with_blocked_resting_ops() {
        let (blueprint, operbox, instances, table) = fixtures();
        if !["歌蕾蒂娅", "乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"]
            .iter()
            .all(|name| operbox.owns(name))
        {
            return;
        }
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);

        let shared = BaseAssignment::default();
        let beta = BaseAssignment::default();
        let gamma_h1 = BaseAssignment::default();
        let blocked_ops: HashSet<String> = ["芬"].into_iter().map(str::to_string).collect();

        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1"), RoomId::from("manu_3")],
        });

        assert!(!candidates.is_empty());
        for candidate in candidates {
            assert!(
                !operators_of(&candidate.assignment).contains(&"芬".to_string()),
                "深海候选补位不应使用 S2 休息队员"
            );
        }
    }

    #[test]
    fn abyssal_candidate_accepts_original_hunters_at_any_tier_but_not_alternates() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let operbox = OperBox::from_entries(vec![
            OperBoxEntry {
                id: "gladiia".into(),
                name: "歌蕾蒂娅".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ulpian".into(),
                name: "乌尔比安".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "skadi".into(),
                name: "斯卡蒂".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ghost".into(),
                name: "幽灵鲨".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "angel".into(),
                name: "安哲拉".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ghost2".into(),
                name: "归溟幽灵鲨".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "fen".into(),
                name: "芬".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 3,
            },
            OperBoxEntry {
                id: "kroos".into(),
                name: "克洛丝".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 3,
            },
        ]);
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);
        let shared = BaseAssignment::default();
        let beta = BaseAssignment::default();
        let gamma_h1 = BaseAssignment::default();
        let blocked_ops = HashSet::new();

        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1"), RoomId::from("manu_3")],
        });

        assert!(
            !candidates.is_empty(),
            "原阵营四名深海猎人齐备时应进入 S2 深海候选"
        );
    }

    #[test]
    fn abyssal_candidate_runs_with_three_original_hunters() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let operbox = OperBox::from_entries(vec![
            OperBoxEntry {
                id: "gladiia".into(),
                name: "歌蕾蒂娅".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ulpian".into(),
                name: "乌尔比安".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "skadi".into(),
                name: "斯卡蒂".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "angel".into(),
                name: "安哲拉".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ghost2".into(),
                name: "归溟幽灵鲨".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
        ]);
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);
        let shared = BaseAssignment::default();
        let beta = BaseAssignment::default();
        let gamma_h1 = BaseAssignment::default();
        let blocked_ops = HashSet::new();

        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1")],
        });

        assert!(
            !candidates.is_empty(),
            "缺一名原阵营深海猎人时应进入降级 S2 深海候选"
        );
        for candidate in candidates {
            assert!(
                !operators_of(&candidate.assignment).contains(&"归溟幽灵鲨".to_string()),
                "归溟幽灵鲨不能代替本体幽灵鲨进入深海候选"
            );
        }
    }

    #[test]
    fn abyssal_candidate_requires_at_least_three_original_hunters() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let operbox = OperBox::from_entries(vec![
            OperBoxEntry {
                id: "gladiia".into(),
                name: "歌蕾蒂娅".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "skadi".into(),
                name: "斯卡蒂".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "angel".into(),
                name: "安哲拉".into(),
                elite: 0,
                level: 1,
                own: true,
                potential: 1,
                rarity: 6,
            },
            OperBoxEntry {
                id: "ghost2".into(),
                name: "归溟幽灵鲨".into(),
                elite: 2,
                level: 60,
                own: true,
                potential: 1,
                rarity: 6,
            },
        ]);
        let (layout, manu_pool, options) =
            build_test_manu_ctx(&blueprint, &operbox, &instances, &table);
        let shared = BaseAssignment::default();
        let beta = BaseAssignment::default();
        let gamma_h1 = BaseAssignment::default();
        let blocked_ops = HashSet::new();

        let candidates = build_abyssal_s2_candidates(&AbyssalBuildCtx {
            operbox: &operbox,
            instances: &instances,
            table: &table,
            blueprint: &blueprint,
            layout: &layout,
            options: &options,
            manu_pool: &manu_pool,
            used_ab: &HashSet::new(),
            blocked_ops: &blocked_ops,
            shared: &shared,
            beta: &beta,
            gamma_h1: &gamma_h1,
            mutable_manu_rooms: &[RoomId::from("manu_1"), RoomId::from("manu_3")],
        });

        assert!(
            candidates.is_empty(),
            "两名原阵营深海猎人收益不足，不应进入 S2 深海候选"
        );
    }

    #[test]
    fn team_rotation_fills_every_facility_each_shift() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        assert_eq!(report.shifts.len(), 3);
        assert_eq!(report.teams.len(), 3);

        let production_rooms: Vec<&RoomId> = blueprint
            .rooms
            .iter()
            .filter(|r| {
                matches!(
                    r.kind,
                    FacilityKind::TradePost | FacilityKind::Factory | FacilityKind::PowerPlant
                )
            })
            .map(|r| &r.id)
            .collect();

        // 关键：每班每个生产设施都满编，绝不空转。
        for shift in &report.shifts {
            for room_id in &production_rooms {
                let ops = shift.assignment.operators_in(room_id);
                assert!(
                    !ops.is_empty(),
                    "shift {} 设施 {} 空转",
                    shift.index + 1,
                    room_id.0
                );
            }
            // 每班内部无重复干员。
            let mut seen = HashSet::new();
            for room in &shift.assignment.rooms {
                for op in &room.operators {
                    assert!(
                        seen.insert(op.name.clone()),
                        "shift {} dup {}",
                        shift.index,
                        op.name
                    );
                }
            }
        }

        // 三队两两互斥。
        for i in 0..report.teams.len() {
            for j in (i + 1)..report.teams.len() {
                let a: HashSet<_> = report.teams[i].operators.iter().collect();
                let b: HashSet<_> = report.teams[j].operators.iter().collect();
                assert!(a.is_disjoint(&b), "teams {i} & {j} overlap");
            }
        }

        assert!((report.shifts[0].duration_hours - 12.0).abs() < f64::EPSILON);
        assert!(!report.daily.trade.is_zero());
        assert!(!report.daily.manufacture.is_zero());
    }

    #[test]
    fn team_rotation_control_center_operator_does_not_work_all_three_shifts() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let mut shifts_by_name: HashMap<String, HashSet<usize>> = HashMap::new();
        for shift in &report.shifts {
            for op in shift.assignment.control_operators() {
                shifts_by_name
                    .entry(op.name)
                    .or_default()
                    .insert(shift.index);
            }
        }

        for (name, shifts) in shifts_by_name {
            assert!(
                shifts.len() < 3,
                "中枢干员 {name} 不应三班连续上岗: shifts={shifts:?}"
            );
        }
    }

    #[test]
    fn team_rotation_control_center_respects_resting_team() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let team_by_name = operator_team_map(&report);

        for shift in &report.shifts {
            for op in shift.assignment.control_operators() {
                let team = team_by_name
                    .get(&op.name)
                    .copied()
                    .unwrap_or_else(|| panic!("中枢干员 {} 缺少 α/β/γ 归属", op.name));
                assert_ne!(
                    team,
                    shift.resting_team,
                    "shift {} 中枢干员 {} 属于休息队 {:?}",
                    shift.index + 1,
                    op.name,
                    shift.resting_team
                );
            }
        }
    }

    #[test]
    fn team_rotation_control_prefers_trade_manu_inject_over_resource_only_fillers() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let mut control_by_shift: Vec<Vec<String>> = Vec::new();
        for shift in &report.shifts {
            control_by_shift.push(
                shift
                    .assignment
                    .control_operators()
                    .into_iter()
                    .map(|op| op.name)
                    .collect(),
            );
        }

        let trade_injectors = ["阿米娅", "诗怀雅", "明椒", "阿斯卡纶", "望", "火龙S黑角"];
        let manu_injectors = ["斩业星熊", "Mon3tr", "凯尔希", "布丁", "麒麟R夜刀"];
        for names in &control_by_shift {
            assert_eq!(
                names.len(),
                5,
                "每班中枢仍应满编 5 人: {:?}",
                control_by_shift
            );
            assert!(
                names
                    .iter()
                    .any(|name| trade_injectors.contains(&name.as_str())),
                "每班中枢应优先包含全贸易订单效率注入: {:?}",
                control_by_shift
            );
            assert!(
                names
                    .iter()
                    .any(|name| manu_injectors.contains(&name.as_str())),
                "每班中枢应优先包含全制造生产力注入: {:?}",
                control_by_shift
            );
        }
        for resource_only in ["三角初华", "战车", "闪击"] {
            assert!(
                control_by_shift
                    .iter()
                    .all(|names| names.iter().all(|name| name != resource_only)),
                "{resource_only} 不应作为无当前消费方的中枢补位进入班次: {:?}",
                control_by_shift
            );
        }
    }

    #[test]
    fn team_rotation_abyssal_only_runs_in_s2_short_shift() {
        let (blueprint, operbox, instances, table) = fixtures();
        if !operbox.owns("歌蕾蒂娅") || !operbox.owns("乌尔比安") {
            return;
        }
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let abyssal_names: HashSet<&str> = ["乌尔比安", "斯卡蒂", "幽灵鲨", "安哲拉"]
            .into_iter()
            .collect();

        let mut shifts_with_abyssal_manu = Vec::new();
        for shift in &report.shifts {
            let has_abyssal_manu = shift.assignment.rooms.iter().any(|room| {
                blueprint
                    .room(&room.room_id)
                    .is_some_and(|bp| bp.kind == FacilityKind::Factory)
                    && room
                        .operators
                        .iter()
                        .any(|op| abyssal_names.contains(op.name.as_str()))
            });
            if has_abyssal_manu {
                shifts_with_abyssal_manu.push(shift.index);
                assert_eq!(shift.index, 1, "深海链只允许 S2 6h 短班");
                assert!(
                    shift.efficiencies.manufacture_efficiency.as_f64() > 4.500,
                    "S2 深海候选应使用带歌蕾蒂娅和深海 tag 的最终布局重算制造分，got {}",
                    shift.efficiencies.manufacture_efficiency
                );
                assert!(
                    shift
                        .assignment
                        .control_operators()
                        .iter()
                        .any(|op| op.name == "歌蕾蒂娅"),
                    "S2 深海候选必须保留歌蕾蒂娅中枢"
                );
                let resting = report
                    .teams
                    .iter()
                    .find(|team| team.label == shift.resting_team)
                    .unwrap();
                for room in &shift.assignment.rooms {
                    let is_factory = blueprint
                        .room(&room.room_id)
                        .is_some_and(|bp| bp.kind == FacilityKind::Factory);
                    if is_factory
                        && room
                            .operators
                            .iter()
                            .any(|op| abyssal_names.contains(op.name.as_str()))
                    {
                        assert!(
                            room.operators
                                .iter()
                                .all(|op| op.name != "冬时" && op.name != "温蒂"),
                            "自动化组体系专用干员不应作为深海制造散件: {:?}",
                            room.operators
                        );
                    }
                    for op in &room.operators {
                        assert!(
                            !resting.operators.contains(&op.name),
                            "S2 上岗干员 {} 不应属于休息队 {:?}",
                            op.name,
                            shift.resting_team
                        );
                    }
                }
            }
        }

        assert!(
            shifts_with_abyssal_manu == vec![1],
            "深海制造只应出现在 S2: {shifts_with_abyssal_manu:?}"
        );
    }

    #[test]
    fn team_rotation_assignments_do_not_use_resting_team_members() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let team_by_name = operator_team_map(&report);

        for shift in &report.shifts {
            for room in &shift.assignment.rooms {
                if blueprint.room(&room.room_id).is_some_and(|bp| {
                    matches!(bp.kind, FacilityKind::Dormitory | FacilityKind::Office)
                }) {
                    continue;
                }
                for op in &room.operators {
                    let team = team_by_name
                        .get(&op.name)
                        .copied()
                        .unwrap_or_else(|| panic!("上岗干员 {} 缺少 α/β/γ 归属", op.name));
                    if team == shift.resting_team {
                        assert_eq!(
                            shift
                                .fiammetta
                                .as_ref()
                                .map(|action| action.target.as_str()),
                            Some(op.name.as_str()),
                            "shift {} 房间 {} 非菲亚目标的休息队干员 {} 被错误排回",
                            shift.index + 1,
                            room.room_id.0,
                            op.name
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn team_rotation_scores_match_current_shift_context() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let durin_plan = operbox.durin_dorm_planning_count(&instances);

        for shift in &report.shifts {
            let mut assignment = shift.assignment.clone();
            clear_production_efficiencies(&blueprint, &mut assignment);
            let recomputed = evaluate_base_assignment_efficiencies(
                &blueprint,
                &assignment,
                &instances,
                &table,
                shift.duration_hours,
                Some(durin_plan),
            )
            .unwrap();

            assert!(
                recomputed.trade_efficiency == shift.efficiencies.trade_efficiency,
                "shift {} trade score used a stale room snapshot: recomputed={} stored={}",
                shift.index + 1,
                recomputed.trade_efficiency,
                shift.efficiencies.trade_efficiency
            );
            assert!(
                recomputed.manufacture_efficiency == shift.efficiencies.manufacture_efficiency,
                "shift {} manufacture score used a stale room snapshot: recomputed={} stored={}",
                shift.index + 1,
                recomputed.manufacture_efficiency,
                shift.efficiencies.manufacture_efficiency
            );
            assert!(
                recomputed.power_efficiency == shift.efficiencies.power_efficiency,
                "shift {} power score used a stale room snapshot: recomputed={} stored={}",
                shift.index + 1,
                recomputed.power_efficiency,
                shift.efficiencies.power_efficiency
            );
        }
    }

    #[test]
    fn team_rotation_fiammetta_returns_peak_core_and_rests_replacement() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let actions: Vec<_> = report
            .shifts
            .iter()
            .filter_map(|shift| shift.fiammetta.as_ref().map(|action| (shift, action)))
            .collect();
        assert_eq!(actions.len(), 1, "每个 24h 周期只应安排一次菲亚回岗");

        let (shift, action) = actions[0];
        assert_eq!(action.target, "但书");
        assert!(
            shift
                .assignment
                .operators_in(&action.room_id)
                .iter()
                .any(|op| op.name == action.target),
            "菲亚目标必须回到 peak 原房间"
        );
        assert!(
            !shift
                .assignment
                .operator_names()
                .contains(&action.displaced),
            "被替换者必须离开当班 assignment"
        );
        assert_eq!(
            crate::schedule::team_of_operator(&report, &action.target),
            Some(shift.resting_team),
            "菲亚回岗应是休息队主力的显式例外"
        );
    }

    #[test]
    fn team_rotation_without_fiammetta_does_not_create_return_action() {
        let (blueprint, operbox, instances, table) = fixtures();
        let no_fiammetta = operbox.excluding(&HashSet::from(["菲亚梅塔".to_string()]));
        let report = schedule_team_rotation(
            &blueprint,
            &no_fiammetta,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report.shifts.iter().all(|shift| shift.fiammetta.is_none()));
    }

    #[test]
    fn team_rotation_carries_peak_plan() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(report.peak_plan.mode, AssignShiftMode::Peak);
        assert!(
            !report
                .peak_plan
                .registry_system_ids()
                .contains(&"syracusa_pair"),
            "叙拉古贸易队友不应再由 peak registry 认领"
        );
        if ["八幡海铃", "伺夜", "贝洛内"]
            .iter()
            .all(|name| operbox.elite_of(name).is_some_and(|elite| elite >= 2))
        {
            assert!(
                report
                    .peak_plan
                    .registry_system_ids()
                    .contains(&"syracusa_cross_station"),
                "peak plan 应包含叙拉古跨站体系"
            );
        }
    }

    #[test]
    fn team_rotation_reports_peak_mood_eta() {
        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let eta = report
            .peak_mood_eta
            .as_ref()
            .expect("peak 主力班必须输出 mood ETA");
        assert!(!eta.per_op.is_empty());
        assert!(eta.bottleneck.is_some());
        assert!(eta.eta_hours.is_some_and(|hours| hours > 0.0));
    }

    #[test]
    fn team_rotation_carries_peak_blackkey_trade_station() {
        let (blueprint, operbox, instances, table) = fixtures();
        if !operbox.owns("黑键") {
            return;
        }
        let peak = assign_shift(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &BaseAssignment::default(),
        )
        .unwrap();
        let peak_has_blackkey = peak.rooms.iter().any(|r| {
            blueprint
                .rooms
                .iter()
                .any(|b| b.id == r.room_id && b.kind == FacilityKind::TradePost)
                && r.operators.iter().any(|o| o.name == "黑键")
        });
        if !peak_has_blackkey {
            return;
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let blackkey_in_rotation = report.shifts.iter().any(|shift| {
            shift.assignment.rooms.iter().any(|room| {
                blueprint
                    .rooms
                    .iter()
                    .any(|b| b.id == room.room_id && b.kind == FacilityKind::TradePost)
                    && room.operators.iter().any(|o| o.name == "黑键")
            })
        });
        assert!(
            blackkey_in_rotation,
            "peak 已认领黑键贸站时 team-rotation 应保留"
        );
        assert!(
            report.teams[0]
                .operators
                .iter()
                .chain(report.teams[1].operators.iter())
                .any(|n| n == "黑键"),
            "黑键应在 α 或 β 队: alpha={:?} beta={:?}",
            report.teams[0].operators,
            report.teams[1].operators
        );
        for shift in &report.shifts {
            assert!(
                !blackkey_witch_same_trade_room(&shift.assignment, &blueprint),
                "shift {} 黑键与巫恋不得同房",
                shift.index + 1
            );
        }
    }

    #[test]
    fn team_rotation_rosemary_blackkey_shift_bind() {
        use crate::layout::build_plan;
        use crate::schedule::shift_bind::{
            shift_binds_from_plan, team_of_operator, verify_shift_binds,
        };

        let (blueprint, operbox, instances, table) = fixtures();
        if !operbox.owns("迷迭香") || !operbox.owns("黑键") {
            return;
        }
        let peak = assign_shift(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &BaseAssignment::default(),
        )
        .unwrap();
        assert!(
            peak.rooms
                .iter()
                .any(|r| r.operators.iter().any(|o| o.name == "迷迭香")),
            "迷迭香链激活时 peak 应含迷迭香"
        );
        assert!(
            peak.rooms
                .iter()
                .any(|r| r.operators.iter().any(|o| o.name == "黑键")),
            "迷迭香链激活时 peak 应含黑键"
        );

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let binds = shift_binds_from_plan(
            &build_plan(
                &blueprint,
                &operbox,
                AssignShiftMode::Peak,
                &BaseAssignment::default(),
                &std::collections::HashSet::new(),
            )
            .unwrap(),
        );
        verify_shift_binds(&report, &binds, &peak).expect("迷迭香+黑键 应同上同下、上2休1");
        let team = team_of_operator(&report, "迷迭香").unwrap();
        assert_eq!(
            team_of_operator(&report, "黑键"),
            Some(team),
            "迷迭香与黑键应同队"
        );
    }

    #[test]
    fn team_rotation_pure_fireworks_rotates_office_trade_and_control_together() {
        use crate::schedule::shift_bind::team_of_operator;

        let (blueprint, full_operbox, instances, table) = fixtures();
        let operbox = OperBox::from_entries(
            full_operbox
                .entries
                .into_iter()
                .filter(|entry| entry.name != "迷迭香")
                .collect(),
        );
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report
            .peak_plan
            .activated
            .iter()
            .any(|system| system.system_id == "human_fireworks_pure"));
        let bind = report
            .peak_plan
            .shift_binds
            .iter()
            .find(|bind| {
                bind.operators.iter().any(|name| name == "桑葚")
                    && bind.operators.iter().any(|name| name == "乌有")
            })
            .expect("纯人间烟火应产生包含桑葚和乌有的实际 shift bind");
        let control_name = ["重岳", "令"]
            .into_iter()
            .find(|name| bind.operators.iter().any(|bound| bound == name))
            .expect("纯人间烟火 bind 应包含实际入驻中枢的重岳或令");
        let team = team_of_operator(&report, "桑葚").expect("桑葚应归入轮换队伍");
        assert_eq!(
            report
                .teams
                .iter()
                .flat_map(|cohort| &cohort.operators)
                .filter(|name| name.as_str() == "桑葚")
                .count(),
            1,
            "绑定办公室成员只能归属一个 cohort"
        );
        for name in ["乌有", control_name] {
            assert_eq!(
                team_of_operator(&report, name),
                Some(team),
                "纯烟火成员应与桑葚同队: {name}"
            );
        }

        let in_facility = |shift: &TeamShiftResult, facility: FacilityKind, name: &str| {
            shift.assignment.rooms.iter().any(|room| {
                blueprint
                    .room(&room.room_id)
                    .is_some_and(|bp| bp.kind == facility)
                    && room.operators.iter().any(|op| op.name == name)
            })
        };
        let mut active_count = 0;
        for shift in &report.shifts {
            assert_eq!(shift.assignment.control_operators().len(), 5);
            let mangberry_active = in_facility(shift, FacilityKind::Office, "桑葚");
            let wuyou_active = in_facility(shift, FacilityKind::TradePost, "乌有");
            let control_active = in_facility(shift, FacilityKind::ControlCenter, control_name);
            let resolve_fireworks = |assignment: &BaseAssignment| {
                resolve_base(
                    &blueprint,
                    assignment,
                    Some(&instances),
                    Some(&table),
                    24.0,
                    None,
                )
                .unwrap()
                .layout
                .global
                .get(crate::global_resource::GlobalResourceKey::HumanFireworks)
            };
            let actual_fireworks = resolve_fireworks(&shift.assignment);
            let office_id = blueprint
                .rooms
                .iter()
                .find(|room| room.kind == FacilityKind::Office)
                .unwrap()
                .id
                .clone();
            let mut counterfactual = shift.assignment.clone();
            counterfactual.set_room(
                office_id,
                if mangberry_active {
                    Vec::new()
                } else {
                    vec![AssignedOperator::new("桑葚", 2)]
                },
            );
            let counterfactual_fireworks = resolve_fireworks(&counterfactual);
            assert_eq!(
                (wuyou_active, control_active),
                (mangberry_active, mangberry_active),
                "纯烟火办公室、贸易和中枢成员必须同上同下"
            );

            if shift.resting_team == team {
                assert!(!mangberry_active, "桑葚休息班不得继续占用办公室");
                let offices: Vec<_> = shift
                    .assignment
                    .rooms
                    .iter()
                    .filter(|room| {
                        blueprint
                            .room(&room.room_id)
                            .is_some_and(|bp| bp.kind == FacilityKind::Office)
                    })
                    .collect();
                assert!(!offices.is_empty(), "布局应包含办公室");
                assert!(
                    offices.iter().all(|room| !room.operators.is_empty()),
                    "桑葚休息时办公室必须由其他干员补位"
                );
                assert!(offices.iter().all(|room| {
                    room.operators
                        .iter()
                        .all(|operator| operator.name != "桑葚")
                }));
                assert!(
                    (counterfactual_fireworks - actual_fireworks - 20.0).abs() < 0.001,
                    "休班替补不得暗含桑葚的 +20 烟火"
                );
            } else {
                active_count += 1;
                assert!(mangberry_active, "桑葚所属队当班时应进入办公室");
                assert!(
                    (actual_fireworks - counterfactual_fireworks - 20.0).abs() < 0.001,
                    "最终班次 resolve 必须看到桑葚办公室的 +20 烟火"
                );
            }
        }
        assert_eq!(active_count, 2, "纯烟火绑定组应上二休一");
    }

    #[test]
    fn team_rotation_perception_fireworks_keeps_full_core_together() {
        use crate::schedule::shift_bind::team_of_operator;

        let (blueprint, operbox, instances, table) = fixtures();
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report
            .peak_plan
            .registry_system_ids()
            .contains(&"human_fireworks_perception"));
        assert!(!report
            .peak_plan
            .registry_system_ids()
            .contains(&"human_fireworks_pure"));
        let team = team_of_operator(&report, "乌有").expect("乌有应归入感知烟火 cohort");
        for name in ["重岳", "令"] {
            assert_eq!(team_of_operator(&report, name), Some(team));
        }

        let in_facility = |shift: &TeamShiftResult, facility: FacilityKind, name: &str| {
            shift.assignment.rooms.iter().any(|room| {
                blueprint
                    .room(&room.room_id)
                    .is_some_and(|bp| bp.kind == facility)
                    && room.operators.iter().any(|op| op.name == name)
            })
        };
        let mut active_count = 0;
        for shift in &report.shifts {
            assert_eq!(shift.assignment.control_operators().len(), 5);
            let wuyou = in_facility(shift, FacilityKind::TradePost, "乌有");
            let zhongyue = in_facility(shift, FacilityKind::ControlCenter, "重岳");
            let ling = in_facility(shift, FacilityKind::ControlCenter, "令");
            assert_eq!((zhongyue, ling), (wuyou, wuyou));
            assert!(
                !in_facility(shift, FacilityKind::Office, "桑葚"),
                "感知分支与纯烟火桑葚办公室分支必须互斥"
            );
            assert!(blueprint
                .rooms
                .iter()
                .filter(|room| room.kind == FacilityKind::Office)
                .all(|room| !shift.assignment.operators_in(&room.id).is_empty()));
            active_count += usize::from(wuyou);
        }
        assert_eq!(active_count, 2);
    }

    #[test]
    fn team_rotation_pinus_two_manufacturers_end_to_end() {
        let (blueprint, full_operbox, instances, table) = fixtures();
        let operbox = OperBox::from_entries(
            full_operbox
                .entries
                .into_iter()
                .filter(|entry| entry.name != "野鬃")
                .collect(),
        );
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        assert!(report
            .peak_plan
            .activated
            .iter()
            .any(|system| system.system_id == "pinus_sylvestris"));
        assert_pinus_rotation(&report, &blueprint, &["灰毫", "远牙"]);
        assert!(
            report
                .teams
                .iter()
                .all(|team| !team.operators.iter().any(|name| name == "野鬃")),
            "未拥有的野鬃不得进入轮换"
        );
    }

    #[test]
    fn team_rotation_pinus_three_manufacturers_span_battle_record_rooms() {
        let (mut blueprint, operbox, instances, table) = fixtures();
        for room in &mut blueprint.rooms {
            if matches!(
                room.product,
                Some(crate::layout::RoomProduct::Factory {
                    recipe: crate::types::RecipeKind::BattleRecord
                })
            ) {
                room.level = 2;
            }
        }
        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let manufacturers = ["灰毫", "远牙", "野鬃"];
        assert_pinus_rotation(&report, &blueprint, &manufacturers);
        for shift in &report.shifts {
            if shift.assignment.rooms.iter().any(|room| {
                room.operators
                    .iter()
                    .any(|op| manufacturers.contains(&op.name.as_str()))
            }) {
                let occupied_rooms = shift
                    .assignment
                    .rooms
                    .iter()
                    .filter(|room| {
                        room.operators
                            .iter()
                            .any(|op| manufacturers.contains(&op.name.as_str()))
                    })
                    .count();
                assert!(
                    occupied_rooms >= 2,
                    "标准 243 满配应证明红松成员允许跨作战记录站分布"
                );
            }
        }
    }

    #[test]
    fn team_rotation_keeps_docus_and_syracusa_cross_station_members() {
        use crate::operbox::default_operbox_full_e2_path;

        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let operbox = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        for name in ["但书", "伺夜", "贝洛内", "可露希尔", "黑键", "吉星"] {
            if !operbox.owns(name) {
                return;
            }
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();
        let shift1 = &report.shifts[0].assignment;
        let trade_rooms: Vec<_> = shift1
            .rooms
            .iter()
            .filter(|r| {
                blueprint
                    .rooms
                    .iter()
                    .any(|b| b.id == r.room_id && b.kind == FacilityKind::TradePost)
            })
            .collect();
        assert!(
            trade_rooms
                .iter()
                .any(|r| r.operators.iter().any(|o| o.name == "但书")),
            "12h 班应包含但书站: {:?}",
            trade_rooms
        );
        let teams = operator_team_map(&report);
        for name in ["伺夜", "贝洛内"] {
            if let Some(team) = teams.get(name) {
                assert_eq!(
                    Some(team),
                    teams.get("八幡海铃"),
                    "自然入选的叙拉古贸易成员必须与八幡海铃同上同下: {name}"
                );
            }
        }
        assert!(
            trade_rooms
                .iter()
                .any(|r| r.operators.iter().any(|o| o.name == "可露希尔")),
            "12h 班应保留可露希尔核心: {:?}",
            trade_rooms
        );
    }

    #[test]
    fn team_rotation_vina_trade_shift_pins_daifeen_control() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let base = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let operbox = base.excluding(&HashSet::from([
            "龙舌兰".to_string(),
            "可露希尔".to_string(),
            "但书".to_string(),
        ]));
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        for name in [DAIFEEN, "推进之王", "摩根", "维娜·维多利亚"] {
            if !operbox.owns(name) {
                return;
            }
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
        )
        .unwrap();

        let mut checked = 0;
        for shift in &report.shifts {
            let has_vina_trade = trade_room_contains(
                &shift.assignment,
                &blueprint,
                &["推进之王", "摩根", "维娜·维多利亚"],
            );
            if has_vina_trade {
                checked += 1;
                assert!(
                    shift
                        .assignment
                        .control_operators()
                        .iter()
                        .any(|op| op.name == DAIFEEN),
                    "shift {} 推王组上站时中枢必须同步戴菲恩: {:?}",
                    shift.index + 1,
                    shift.assignment.control_operators()
                );
            }
        }
        let _ = checked;
    }

    #[test]
    fn team_rotation_feedback_000142_docus_lv2_prefers_higher_tool_over_vina() {
        use crate::operbox::default_operbox_full_e2_path;

        let blueprint =
            BaseBlueprint::load(&crate::skill_table::data_path("layout/252.json").unwrap())
                .unwrap();
        let operbox = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
        )
        .unwrap();

        for shift in &report.shifts {
            for room in &shift.assignment.rooms {
                let Some(bp_room) = blueprint.room(&room.room_id) else {
                    continue;
                };
                if bp_room.kind != FacilityKind::TradePost {
                    continue;
                }
                let names: Vec<_> = room.operators.iter().map(|op| op.name.as_str()).collect();
                assert!(
                    !(names.contains(&"但书") && names.contains(&"维娜·维多利亚")),
                    "shift {} 但书二级站不应选择退化态维娜: {:?}",
                    shift.index + 1,
                    names
                );
            }
        }
    }

    #[test]
    fn team_rotation_does_not_invent_karlan_control_producer() {
        let mut blueprint = BaseBlueprint::template_snhunt().unwrap();
        blueprint.scenario.initial_global.clear();
        let base = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let operbox = base.excluding(&HashSet::from(["八幡海铃".to_string(), "黑键".to_string()]));
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
        )
        .unwrap();

        let peak_has_karlan_consumer = report.peak_plan.shift_binds.iter().any(|bind| {
            ["灵知", "孑", "银灰"]
                .iter()
                .all(|name| bind.operators.iter().any(|op| op == name))
        });
        assert!(
            !peak_has_karlan_consumer,
            "本 fixture 的 peak 未形成喀兰消费链"
        );
        assert!(
            report.shifts.iter().all(|shift| !shift
                .assignment
                .control_operators()
                .iter()
                .any(|op| op.name == "灵知")),
            "未形成实际消费链时不得仅因 operbox 持有灵知而强塞中枢"
        );
    }

    #[test]
    fn team_rotation_feedback_010457_witch_group_keeps_trade_room() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let operbox = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        for name in ["巫恋", "龙舌兰"] {
            if !operbox.owns(name) {
                return;
            }
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
        )
        .unwrap();

        let mut witch_rooms = Vec::new();
        for shift in &report.shifts {
            if let Some(room_id) = trade_room_containing_group(
                &shift.assignment,
                &blueprint,
                &WARMUP_STICKY_TRADE_OPERATORS,
            ) {
                witch_rooms.push((shift.index, room_id));
            }
        }
        assert!(
            witch_rooms.len() >= 2,
            "feedback operbox should schedule witch group in short shifts: {witch_rooms:?}"
        );
        let first_room = witch_rooms[0].1.clone();
        assert!(
            witch_rooms.iter().all(|(_, room)| *room == first_room),
            "巫恋/龙舌兰组跨班不应变更贸易站: {witch_rooms:?}"
        );
    }

    #[test]
    fn team_rotation_warmup_manu_operator_keeps_factory_room() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let operbox = OperBox::load(&default_operbox_full_e2_path().unwrap()).unwrap();
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        if !operbox.owns("阿罗玛") {
            return;
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
        )
        .unwrap();

        let mut aroma_rooms = Vec::new();
        for shift in &report.shifts {
            if let Some(room_id) = room_containing_operator(
                &shift.assignment,
                &blueprint,
                "阿罗玛",
                FacilityKind::Factory,
            ) {
                aroma_rooms.push((shift.index, room_id));
            }
        }
        assert!(
            aroma_rooms.len() >= 2,
            "full E2 rotation should schedule 阿罗玛 in multiple shifts: {aroma_rooms:?}"
        );
        let first_room = aroma_rooms[0].1.clone();
        assert!(
            aroma_rooms.iter().all(|(_, room)| *room == first_room),
            "阿罗玛例行清扫跨班不应变更制造站: {aroma_rooms:?}"
        );
    }

    /// γ 队贸易站不得抢占 peak α/β 已认领的 meta 干员（如但书、巫恋）。
    #[test]
    fn team_rotation_gamma_trade_disjoint_from_peak_meta() {
        const META_TRADE_OPS: &[&str] = &["但书", "巫恋", "龙舌兰", "可露希尔"];

        let (blueprint, operbox, instances, table) = fixtures();
        let peak = assign_shift(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &BaseAssignment::default(),
        )
        .unwrap();

        let peak_meta: HashSet<String> = peak
            .rooms
            .iter()
            .filter(|r| {
                blueprint
                    .rooms
                    .iter()
                    .any(|b| b.id == r.room_id && b.kind == FacilityKind::TradePost)
            })
            .flat_map(|r| r.operators.iter().map(|o| o.name.clone()))
            .filter(|n| META_TRADE_OPS.contains(&n.as_str()))
            .collect();
        if peak_meta.is_empty() {
            return;
        }

        let report = schedule_team_rotation(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 5,
                ..Default::default()
            },
        )
        .unwrap();

        let gamma_ops: HashSet<_> = report.teams[2].operators.iter().cloned().collect();
        for name in peak_meta {
            assert!(
                !gamma_ops.contains(&name),
                "γ 队不应含 peak meta 干员 {name}"
            );
        }
    }

    #[test]
    fn split_keeps_multi_room_production_bind_as_one_component() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let mut peak = BaseAssignment::default();
        peak.set_room("manu_1", vec![AssignedOperator::new("生产甲", 2)]);
        peak.set_room("manu_2", vec![AssignedOperator::new("生产乙", 2)]);
        peak.set_room("manu_3", vec![AssignedOperator::new("生产丙", 2)]);
        peak.set_room("control", vec![AssignedOperator::new("中枢甲", 2)]);
        let bind = RuntimeShiftBind {
            operators: ["生产甲", "生产乙", "生产丙", "中枢甲"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            on_shifts: 2,
            off_shifts: 1,
        };

        let [h1, h2] = split_production_facilities(&blueprint, &peak, &[bind]).unwrap();
        let bound = [
            RoomId::from("manu_1"),
            RoomId::from("manu_2"),
            RoomId::from("manu_3"),
        ];
        assert!(
            bound.iter().all(|room| h1.manu.contains(room))
                || bound.iter().all(|room| h2.manu.contains(room)),
            "multi-room bind was split: h1={:?}, h2={:?}",
            h1.manu,
            h2.manu
        );
    }

    #[test]
    fn split_rejects_bind_member_missing_from_peak() {
        let blueprint = BaseBlueprint::template_243_use_this().unwrap();
        let peak = BaseAssignment::default();
        let bind = RuntimeShiftBind {
            operators: vec!["不存在".into(), "也不存在".into()],
            on_shifts: 2,
            off_shifts: 1,
        };
        let error = split_production_facilities(&blueprint, &peak, &[bind]).unwrap_err();
        assert!(error.to_string().contains("missing from peak"));
    }
}
