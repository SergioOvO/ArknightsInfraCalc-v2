use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::layout::{
    assignment_operator_names, AssignedOperator, BaseAssignment, BaseBlueprint, FacilityKind,
    RoomProduct,
};
use crate::operbox::OperBox;
use crate::schedule::{BaseRotationReport, BaseShiftRole, TeamLabel, TeamRotationReport};
use crate::trade::input::TradeOrderKind;
use crate::types::RecipeKind;

#[derive(Debug, Clone)]
pub struct MaaExportOptions {
    pub title: String,
    pub description: Option<String>,
    /// 菲亚梅塔换班优先级清单（按优先级从高到低排列的干员名列表）。
    ///
    /// 每个 plan 生成时，从当班 assignment 中找清单里第一个在岗干员作为换班目标；
    /// 找到则 `enable: true`，找不到或清单为空则 `enable: false`。
    pub fiammetta_priority: Vec<String>,
}

impl MaaExportOptions {
    pub fn for_blueprint(blueprint: &BaseBlueprint) -> Self {
        let template = blueprint
            .template
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("custom");
        Self {
            title: format!("{template} 基建排班"),
            description: Some("由 ArknightsInfraCalc 生成；可导入 MAA 自定义基建换班。".into()),
            fiammetta_priority: Vec::new(),
        }
    }

    /// 启用公孙长乐确认的菲亚梅塔常规目标顺序。
    pub fn enable_gongsun_fiammetta_priority(&mut self) {
        self.fiammetta_priority = ["但书", "巫恋", "龙舌兰", "清流", "可露希尔"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MaaSchedule {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub plans: Vec<MaaPlan>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaaPlan {
    pub name: String,
    pub description: String,
    #[serde(rename = "Fiammetta")]
    pub fiammetta: MaaFiammetta,
    pub drones: MaaDrones,
    pub rooms: MaaRooms,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaaFiammetta {
    pub enable: bool,
    pub target: String,
    pub order: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaaDrones {
    pub enable: bool,
    pub room: &'static str,
    pub index: u8,
    pub order: &'static str,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MaaRooms {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trading: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub manufacture: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub power: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dormitory: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub control: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub meeting: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hire: Vec<MaaRoomSlot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processing: Vec<MaaRoomSlot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MaaRoomSlot {
    pub skip: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub product: Option<&'static str>,
    pub operators: Vec<String>,
    pub sort: bool,
    pub autofill: bool,
}

struct PlanInput<'a> {
    index: usize,
    duration_hours: f64,
    assignment: &'a BaseAssignment,
    name: String,
    description: String,
    resting: Vec<String>,
    fiammetta_priority: &'a [String],
}

impl MaaSchedule {
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| Error::msg(format!("maa schedule serialize: {e}")))?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    Error::msg(format!("maa schedule mkdir {}: {e}", parent.display()))
                })?;
            }
        }
        std::fs::write(path, json)
            .map_err(|e| Error::msg(format!("maa schedule write {}: {e}", path.display())))
    }
}

pub fn build_from_team_rotation(
    blueprint: &BaseBlueprint,
    report: &TeamRotationReport,
    opts: &MaaExportOptions,
) -> Result<MaaSchedule> {
    let plans = report
        .shifts
        .iter()
        .map(|shift| {
            let active: Vec<&str> = shift
                .active_teams
                .iter()
                .map(|team| team_label_zh(*team))
                .collect();
            let resting = resting_team_operators(report, shift.resting_team, &shift.assignment);
            PlanInput {
                index: shift.index,
                duration_hours: shift.duration_hours,
                assignment: &shift.assignment,
                name: format!(
                    "Shift {} · {:.0}h · {}",
                    shift.index + 1,
                    shift.duration_hours,
                    active.join("+")
                ),
                description: format!(
                    "本班 {:.0} 小时；休息 {} 队",
                    shift.duration_hours,
                    team_label_zh(shift.resting_team)
                ),
                resting,
                fiammetta_priority: &opts.fiammetta_priority,
            }
        })
        .map(|input| build_plan(blueprint, &input))
        .collect();
    Ok(wrap_schedule(opts, plans))
}

pub fn build_from_base_rotation(
    blueprint: &BaseBlueprint,
    report: &BaseRotationReport,
    opts: &MaaExportOptions,
) -> Result<MaaSchedule> {
    let plans = report
        .shifts
        .iter()
        .map(|shift| {
            let role = match shift.role {
                BaseShiftRole::Peak => "高峰",
                BaseShiftRole::Recovery => "恢复",
            };
            let reuse = shift
                .reused_from_shift
                .map(|i| format!(" · 复用 shift{}", i + 1))
                .unwrap_or_default();
            PlanInput {
                index: shift.index,
                duration_hours: 12.0,
                assignment: &shift.assignment,
                name: format!("Shift {} · {role}{reuse}", shift.index + 1),
                description: format!("ABA {role} 班；下次约 12 小时后换班"),
                resting: shift.rotating_workers.clone(),
                fiammetta_priority: &opts.fiammetta_priority,
            }
        })
        .map(|input| build_plan(blueprint, &input))
        .collect();
    Ok(wrap_schedule(opts, plans))
}

fn wrap_schedule(opts: &MaaExportOptions, plans: Vec<MaaPlan>) -> MaaSchedule {
    MaaSchedule {
        title: Some(opts.title.clone()),
        description: opts.description.clone(),
        plans,
    }
}

fn build_plan(blueprint: &BaseBlueprint, input: &PlanInput) -> MaaPlan {
    MaaPlan {
        name: input.name.clone(),
        description: input.description.clone(),
        fiammetta: resolve_fiammetta(input.fiammetta_priority, input.assignment),
        drones: drone_defaults(blueprint),
        rooms: build_rooms(blueprint, input.assignment, &input.resting),
    }
}

fn build_rooms(
    blueprint: &BaseBlueprint,
    assignment: &BaseAssignment,
    resting: &[String],
) -> MaaRooms {
    MaaRooms {
        trading: blueprint
            .rooms_of(FacilityKind::TradePost)
            .into_iter()
            .map(|room| {
                let product = room.product.as_ref().and_then(|p| match p {
                    RoomProduct::Trade { order } => Some(maa_trade_product(*order)),
                    _ => None,
                });
                production_slot(assignment, &room.id.0, product, true)
            })
            .collect(),
        manufacture: blueprint
            .rooms_of(FacilityKind::Factory)
            .into_iter()
            .map(|room| {
                let product = room.product.as_ref().and_then(|p| match p {
                    RoomProduct::Factory { recipe } => Some(maa_factory_product(*recipe)),
                    _ => None,
                });
                production_slot(assignment, &room.id.0, product, true)
            })
            .collect(),
        power: blueprint
            .rooms_of(FacilityKind::PowerPlant)
            .into_iter()
            .map(|room| production_slot(assignment, &room.id.0, None, false))
            .collect(),
        dormitory: build_dorm_slots(blueprint, assignment, resting),
        control: if blueprint.count_facility(FacilityKind::ControlCenter) > 0 {
            vec![shared_slot(assignment, "control", false)]
        } else {
            Vec::new()
        },
        meeting: blueprint
            .rooms_of(FacilityKind::MeetingRoom)
            .into_iter()
            .map(|room| shared_slot(assignment, &room.id.0, true))
            .collect(),
        hire: blueprint
            .rooms_of(FacilityKind::Office)
            .into_iter()
            .map(|room| shared_slot(assignment, &room.id.0, true))
            .collect(),
        processing: blueprint
            .rooms_of(FacilityKind::Workshop)
            .into_iter()
            .map(|room| shared_slot(assignment, &room.id.0, false))
            .collect(),
    }
}

fn production_slot(
    assignment: &BaseAssignment,
    room_id: &str,
    product: Option<&'static str>,
    sort: bool,
) -> MaaRoomSlot {
    let operators = operator_names(assignment, room_id);
    MaaRoomSlot {
        skip: false,
        product,
        operators,
        sort,
        autofill: false,
    }
}

fn shared_slot(assignment: &BaseAssignment, room_id: &str, autofill_if_empty: bool) -> MaaRoomSlot {
    let operators = operator_names(assignment, room_id);
    if operators.is_empty() && autofill_if_empty {
        return MaaRoomSlot {
            skip: false,
            product: None,
            operators,
            sort: false,
            autofill: true,
        };
    }
    MaaRoomSlot {
        skip: false,
        product: None,
        operators,
        sort: false,
        autofill: false,
    }
}

fn build_dorm_slots(
    blueprint: &BaseBlueprint,
    assignment: &BaseAssignment,
    resting: &[String],
) -> Vec<MaaRoomSlot> {
    let mut resting: Vec<String> = resting.to_vec();
    blueprint
        .rooms_of(FacilityKind::Dormitory)
        .into_iter()
        .map(|room| {
            let assigned = operator_names(assignment, &room.id.0);
            if !assigned.is_empty() {
                return MaaRoomSlot {
                    skip: false,
                    product: None,
                    operators: assigned,
                    sort: true,
                    autofill: false,
                };
            }
            if !resting.is_empty() {
                let beds = room.dorm_beds.unwrap_or(5).max(1) as usize;
                let take = resting.len().min(beds);
                let ops: Vec<String> = resting.drain(..take).collect();
                return MaaRoomSlot {
                    skip: false,
                    product: None,
                    operators: ops,
                    sort: true,
                    autofill: false,
                };
            }
            MaaRoomSlot {
                skip: false,
                product: None,
                operators: Vec::new(),
                sort: false,
                autofill: true,
            }
        })
        .collect()
}

fn resting_team_operators(
    report: &TeamRotationReport,
    resting_team: TeamLabel,
    assignment: &BaseAssignment,
) -> Vec<String> {
    let assigned = assignment_operator_names(assignment);
    report
        .teams
        .iter()
        .find(|team| team.label == resting_team)
        .map(|team| {
            team.operators
                .iter()
                .filter(|name| !assigned.contains(*name))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn operator_names(assignment: &BaseAssignment, room_id: &str) -> Vec<String> {
    assignment
        .operators_in(&room_id.into())
        .iter()
        .map(|op| op.name.clone())
        .collect()
}

/// 菲亚梅塔换班目标解析。
///
/// 从 `priority`（公孙长乐提供的优先级清单，从高到低）里，找第一个
/// 出现在当班 `assignment` 里的干员作为换班目标；找到则 `enable: true`。
/// 优先级清单为空时退化为旧行为（`enable: false`，不换班）。
fn resolve_fiammetta(priority: &[String], assignment: &BaseAssignment) -> MaaFiammetta {
    if priority.is_empty() {
        return MaaFiammetta {
            enable: false,
            target: String::new(),
            order: "pre",
        };
    }
    let assigned: HashSet<_> = assignment_operator_names(assignment);
    let target = priority
        .iter()
        .find(|name| assigned.contains(name.as_str()))
        .cloned()
        .unwrap_or_default();
    MaaFiammetta {
        enable: !target.is_empty(),
        target,
        order: "pre",
    }
}

fn drone_defaults(blueprint: &BaseBlueprint) -> MaaDrones {
    let index = blueprint
        .rooms
        .iter()
        .filter(|room| room.kind == FacilityKind::Factory)
        .enumerate()
        .find_map(|(i, room)| match room.product.as_ref() {
            Some(RoomProduct::Factory {
                recipe: RecipeKind::Gold,
            }) => Some((i + 1) as u8),
            _ => None,
        })
        .unwrap_or(1);
    MaaDrones {
        enable: true,
        room: "manufacture",
        index,
        order: "pre",
    }
}

fn maa_trade_product(order: TradeOrderKind) -> &'static str {
    match order {
        TradeOrderKind::Gold => "LMD",
        TradeOrderKind::Originium => "Orundum",
    }
}

fn maa_factory_product(recipe: RecipeKind) -> &'static str {
    match recipe {
        RecipeKind::BattleRecord => "Battle Record",
        RecipeKind::Gold => "Pure Gold",
        RecipeKind::Originium => "Originium Shard",
        RecipeKind::All => "Pure Gold",
    }
}

fn team_label_zh(label: TeamLabel) -> &'static str {
    match label {
        TeamLabel::Alpha => "α",
        TeamLabel::Beta => "β",
        TeamLabel::Gamma => "γ",
    }
}

// ── 一图流 / MAA 排班 JSON 导入 ─────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MaaScheduleImport {
    #[serde(default)]
    pub title: Option<String>,
    pub plans: Vec<MaaPlanImport>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaaPlanImport {
    #[serde(default)]
    pub name: String,
    pub rooms: MaaRoomsImport,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MaaRoomsImport {
    #[serde(default)]
    pub trading: Vec<MaaRoomSlotImport>,
    #[serde(default)]
    pub manufacture: Vec<MaaRoomSlotImport>,
    #[serde(default)]
    pub power: Vec<MaaRoomSlotImport>,
    #[serde(default)]
    pub dormitory: Vec<MaaRoomSlotImport>,
    #[serde(default)]
    pub control: Vec<MaaRoomSlotImport>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaaRoomSlotImport {
    #[serde(default)]
    pub skip: bool,
    #[serde(default)]
    pub operators: Vec<String>,
}

pub fn load_maa_schedule(path: &Path) -> Result<MaaScheduleImport> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| Error::msg(format!("maa schedule read {}: {e}", path.display())))?;
    let mut raw = raw;
    if raw.starts_with('\u{feff}') {
        raw = raw.trim_start_matches('\u{feff}').to_string();
    }
    serde_json::from_str(&raw)
        .map_err(|e| Error::msg(format!("maa schedule parse {}: {e}", path.display())))
}

/// 将一图流 plan 转为 `BaseAssignment`；elite 取自 operbox 练度。
pub fn assignment_from_maa_plan(plan: &MaaPlanImport, operbox: &OperBox) -> BaseAssignment {
    let mut assignment = BaseAssignment::default();
    let rooms = &plan.rooms;

    for (i, slot) in rooms.trading.iter().enumerate() {
        if slot.skip {
            continue;
        }
        push_room_ops(&mut assignment, &format!("trade_{}", i + 1), slot, operbox);
    }
    for (i, slot) in rooms.manufacture.iter().enumerate() {
        if slot.skip {
            continue;
        }
        push_room_ops(&mut assignment, &format!("manu_{}", i + 1), slot, operbox);
    }
    for (i, slot) in rooms.power.iter().enumerate() {
        if slot.skip {
            continue;
        }
        push_room_ops(&mut assignment, &format!("power_{}", i + 1), slot, operbox);
    }
    for (i, slot) in rooms.dormitory.iter().enumerate() {
        if slot.skip {
            continue;
        }
        push_room_ops(&mut assignment, &format!("dorm_{}", i + 1), slot, operbox);
    }
    for slot in &rooms.control {
        if slot.skip {
            continue;
        }
        push_room_ops(&mut assignment, "control", slot, operbox);
    }

    assignment
}

fn push_room_ops(
    assignment: &mut BaseAssignment,
    room_id: &str,
    slot: &MaaRoomSlotImport,
    operbox: &OperBox,
) {
    let ops: Vec<AssignedOperator> = slot
        .operators
        .iter()
        .filter(|n| !n.is_empty())
        .map(|name| {
            let elite = operbox.elite_of(name).unwrap_or(0);
            AssignedOperator::new(name, elite)
        })
        .collect();
    if !ops.is_empty() {
        assignment.set_room(room_id, ops);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{AssignedOperator, RoomBlueprint, RoomId};
    use crate::schedule::{ShiftScores, TeamAssignment, TeamShiftResult};
    use std::time::Duration;

    fn sample_blueprint() -> BaseBlueprint {
        BaseBlueprint {
            template: Some("243".into()),
            drone_cap: 135,
            scenario: Default::default(),
            rooms: vec![
                RoomBlueprint {
                    id: RoomId::new("trade_1"),
                    kind: FacilityKind::TradePost,
                    level: 3,
                    product: Some(RoomProduct::Trade {
                        order: TradeOrderKind::Gold,
                    }),
                    dorm_beds: None,
                    dorm_ambience_level: None,
                },
                RoomBlueprint {
                    id: RoomId::new("power_1"),
                    kind: FacilityKind::PowerPlant,
                    level: 3,
                    product: None,
                    dorm_beds: None,
                    dorm_ambience_level: None,
                },
                RoomBlueprint {
                    id: RoomId::new("dorm_1"),
                    kind: FacilityKind::Dormitory,
                    level: 3,
                    product: None,
                    dorm_beds: Some(5),
                    dorm_ambience_level: Some(5),
                },
            ],
        }
    }

    #[test]
    fn build_team_rotation_emits_maa_products() {
        let blueprint = sample_blueprint();
        let mut assignment = BaseAssignment::default();
        assignment.set_room(
            "trade_1",
            vec![
                AssignedOperator::new("但书", 2),
                AssignedOperator::new("龙舌兰", 2),
                AssignedOperator::new("卡夫卡", 2),
            ],
        );
        assignment.set_power_operator("power_1", AssignedOperator::new("格雷伊", 2));

        let report = TeamRotationReport {
            peak_plan: crate::layout::AssignmentPlan::recovery(
                crate::layout::AssignShiftMode::Peak,
            ),
            teams: vec![TeamAssignment {
                label: TeamLabel::Gamma,
                operators: vec!["休息干员".into()],
            }],
            shifts: vec![TeamShiftResult {
                index: 0,
                duration_hours: 12.0,
                active_teams: vec![TeamLabel::Alpha, TeamLabel::Beta],
                resting_team: TeamLabel::Gamma,
                assignment,
                scores: ShiftScores::default(),
                weighted_trade: 0.0,
                weighted_manu: 0.0,
                weighted_power: 0.0,
            }],
            daily: Default::default(),
            elapsed: Duration::from_millis(1),
        };

        let mut opts = MaaExportOptions::for_blueprint(&blueprint);
        opts.enable_gongsun_fiammetta_priority();
        let schedule = build_from_team_rotation(&blueprint, &report, &opts).unwrap();

        assert_eq!(schedule.plans.len(), 1);
        assert_eq!(schedule.plans[0].rooms.trading[0].product, Some("LMD"));
        assert_eq!(
            schedule.plans[0].rooms.trading[0].operators,
            vec!["但书", "龙舌兰", "卡夫卡"]
        );
        assert_eq!(
            schedule.plans[0].rooms.dormitory[0].operators,
            vec!["休息干员"]
        );
        assert_eq!(schedule.plans[0].drones.index, 1);
        assert!(schedule.plans[0].fiammetta.enable);
        assert_eq!(schedule.plans[0].fiammetta.target, "但书");
        assert_eq!(schedule.plans[0].fiammetta.order, "pre");
    }

    #[test]
    fn resolve_fiammetta_uses_confirmed_priority_order() {
        let blueprint = sample_blueprint();
        let mut assignment = BaseAssignment::default();
        assignment.set_room(
            "trade_1",
            vec![
                AssignedOperator::new("龙舌兰", 2),
                AssignedOperator::new("但书", 2),
                AssignedOperator::new("可露希尔", 2),
            ],
        );
        let mut opts = MaaExportOptions::for_blueprint(&blueprint);
        opts.enable_gongsun_fiammetta_priority();

        let resolved = resolve_fiammetta(&opts.fiammetta_priority, &assignment);

        assert!(resolved.enable);
        assert_eq!(resolved.target, "但书");
        assert_eq!(resolved.order, "pre");
    }

    #[test]
    fn resolve_fiammetta_falls_back_and_empty_priority_disables_it() {
        let blueprint = sample_blueprint();
        let mut assignment = BaseAssignment::default();
        assignment.set_room(
            "trade_1",
            vec![
                AssignedOperator::new("清流", 2),
                AssignedOperator::new("可露希尔", 2),
            ],
        );
        let mut opts = MaaExportOptions::for_blueprint(&blueprint);
        opts.enable_gongsun_fiammetta_priority();

        let fallback = resolve_fiammetta(&opts.fiammetta_priority, &assignment);
        assert!(fallback.enable);
        assert_eq!(fallback.target, "清流");

        let disabled = resolve_fiammetta(&[], &assignment);
        assert!(!disabled.enable);
        assert!(disabled.target.is_empty());
        assert_eq!(disabled.order, "pre");
    }
}
