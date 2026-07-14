//! 单班填房流水线主体（ADR 0001 决策 A）。
//!
//! 阶段顺序与 resolve 时机的唯一事实源。facade（`assign.rs`）只负责构建
//! `skip_system_ids` 与 `build_plan`，随后委托给本模块。
//! Peak 会从同一 execute seed 比较普通中枢与可选动态贸易 producer 的完整生产前缀；
//! 每个贸易站在前站提交后重新 resolve，制造只消费胜出的前缀。

use std::collections::HashSet;

use crate::error::{Error, Result};
use crate::instances::OperatorInstances;
use crate::layout::assignment::BaseAssignment;
use crate::layout::blueprint::BaseBlueprint;
use crate::layout::orchestrate::{execute_plan, AssignmentPlan};
use crate::layout::shift::AssignShiftMode;
use crate::operbox::OperBox;
use crate::pool::{
    add_jie_market_to_trade_pool, build_control_pool_with_fillers, build_manufacture_pool,
    build_power_pool, build_trade_pool, karlan_precision_active, ControlPool, PowerPool, TradePool,
};
use crate::search::{
    control_entry_optional_dynamic_trade_tags, control_inject_policy_sort_key_for_layout,
};
use crate::skill_table::SkillTable;

use super::control_fill::{assign_control, assign_control_requiring_any};
use super::manufacture_fill::{assign_manufacture_lines, refresh_manufacture_efficiency_snapshots};
use super::power_fill::assign_power_stations;
use super::producer_fill::{
    assign_dorm_producers, assign_sphinx_urrbian_dorm_anchor,
    cleanup_unused_sphinx_urrbian_dorm_anchor, place_system_producers,
};
use super::run::{AssignmentRun, StageTimer};
use super::trade_fill::{
    assign_trade_jie_remainder, assign_trade_remainder, refresh_trade_efficiency_snapshots,
};
use super::{inject_search_anchor_pool_entries, tag_pool_from_plan, AssignBaseOptions};

/// 执行单班填房流水线，返回最终编制。
///
/// `plan` 已由调用方通过 `build_plan` 构造（含 trade core registry skip 规则）。
#[allow(clippy::too_many_arguments)]
pub(super) fn run_shift_pipeline(
    blueprint: &BaseBlueprint,
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &AssignBaseOptions,
    mode: AssignShiftMode,
    seed: &BaseAssignment,
    plan: &AssignmentPlan,
) -> Result<BaseAssignment> {
    let mut timer = StageTimer::new("单班");

    // 编排落位（execute_plan：fixed/anchor core → assignment + used）。
    let executed = execute_plan(blueprint, operbox, table, plan, seed)?;
    let mut run = AssignmentRun::new(
        blueprint,
        operbox,
        instances,
        table,
        options,
        executed.assignment,
        executed.used,
    );
    timer.mark("编排落位");

    // 建池（中枢 / 贸易 / 制造 / 发电）+ tier 标注 + anchor 注入。
    let mut control_pool = build_control_pool_with_fillers(operbox, instances, table)?;
    let mut trade_pool = build_trade_pool(&operbox.trade_roster(instances), instances, table)?;
    let mut manu_pool =
        build_manufacture_pool(&operbox.manufacture_roster(instances), instances, table)?;
    let mut power_pool = build_power_pool(&operbox.power_roster(instances), instances, table)?;
    tag_pool_from_plan(plan, &mut control_pool);
    tag_pool_from_plan(plan, &mut trade_pool);
    tag_pool_from_plan(plan, &mut manu_pool);
    tag_pool_from_plan(plan, &mut power_pool);
    inject_search_anchor_pool_entries(
        plan,
        operbox,
        instances,
        table,
        &mut trade_pool,
        &mut manu_pool,
    );
    let gold_lines = blueprint.gold_manu_line_count();
    // forbid-same-room 约束（迷迭香 ≠ 清流/温蒂同制造站）从统一 plan 提取，供 anchor 房搜索排除。
    let forbid_same_room = forbid_same_room_pairs(plan);
    timer.mark("建池");

    match mode {
        AssignShiftMode::Peak => {
            let dynamic_producers =
                dynamic_trade_producer_candidates(&control_pool, &trade_pool, table);
            let mut best = run_peak_prefix_candidate(
                run.clone(),
                plan,
                &control_pool,
                &trade_pool,
                &power_pool,
                gold_lines,
                None,
            )?
            .ok_or_else(|| Error::msg("ordinary peak prefix unexpectedly infeasible"))?;
            let normal_control: HashSet<String> = best
                .run
                .assignment
                .control_operators()
                .into_iter()
                .map(|op| op.name)
                .collect();
            for producer in dynamic_producers {
                if normal_control.contains(&producer) {
                    continue;
                }
                let required = HashSet::from([producer]);
                let Some(candidate) = run_peak_prefix_candidate(
                    run.clone(),
                    plan,
                    &control_pool,
                    &trade_pool,
                    &power_pool,
                    gold_lines,
                    Some(&required),
                )?
                else {
                    continue;
                };
                if candidate.policy_sort_key > best.policy_sort_key {
                    best = candidate;
                }
            }
            run = best.run;
            timer.mark("中枢-生产前缀候选");

            let manu_layout = run.resolve_snapshot(true)?;
            assign_manufacture_lines(
                blueprint,
                operbox,
                instances,
                &manu_pool,
                table,
                &manu_layout,
                options,
                &forbid_same_room,
                &mut run.assignment,
                &mut run.used,
            )?;
            timer.mark("制造");
        }
        AssignShiftMode::Recovery => {
            let layout = run.resolve_snapshot(true)?;
            if karlan_precision_active(&layout.global_inject) {
                add_jie_market_to_trade_pool(&mut trade_pool, instances, table);
            }
            assign_trade_jie_remainder(
                blueprint,
                &trade_pool,
                table,
                instances,
                gold_lines,
                options,
                Some(run.durin_plan),
                &mut run.assignment,
                &mut run.used,
            )?;
            timer.mark("trade孑余站");
            assign_manufacture_lines(
                blueprint,
                operbox,
                instances,
                &manu_pool,
                table,
                &layout,
                options,
                &forbid_same_room,
                &mut run.assignment,
                &mut run.used,
            )?;
            timer.mark("制造");
            assign_power_stations(
                blueprint,
                &power_pool,
                table,
                &layout,
                options,
                &mut run.assignment,
                &mut run.used,
            )?;
            timer.mark("发电");
        }
    }

    refresh_manufacture_efficiency_snapshots(
        blueprint,
        &mut run.assignment,
        instances,
        table,
        options.mood,
        Some(run.durin_plan),
    )?;
    refresh_trade_efficiency_snapshots(
        blueprint,
        &mut run.assignment,
        instances,
        table,
        options,
        Some(run.durin_plan),
    )?;

    timer.report();
    Ok(run.assignment)
}

struct PeakPrefixCandidate<'a> {
    run: AssignmentRun<'a>,
    policy_sort_key: f64,
}

/// 从同一个 execute/producer seed 运行制造前候选前缀。
/// control 的选择会改变 `used`、producer/dorm、power 与贸易上下文，因此这些阶段
/// 必须作为一个候选整体推进；制造、轮换与导出只消费最终胜者。
#[allow(clippy::too_many_arguments)]
fn run_peak_prefix_candidate<'a>(
    mut run: AssignmentRun<'a>,
    plan: &AssignmentPlan,
    control_pool: &ControlPool,
    base_trade_pool: &TradePool,
    power_pool: &PowerPool,
    gold_lines: u32,
    required_dynamic_producer: Option<&HashSet<String>>,
) -> Result<Option<PeakPrefixCandidate<'a>>> {
    let producer_layout = run.resolve_snapshot(false)?;
    if run.assignment.control_operators().len() < 5 {
        if let Some(required) = required_dynamic_producer {
            if !assign_control_requiring_any(
                &mut run.assignment,
                control_pool,
                run.table,
                &producer_layout,
                run.options,
                &plan.control_candidate_requirements,
                required,
                &mut run.used,
            )? {
                return Ok(None);
            }
        } else {
            assign_control(
                &mut run.assignment,
                control_pool,
                run.table,
                &producer_layout,
                run.options,
                &plan.control_candidate_requirements,
                &mut run.used,
            )?;
        }
    }
    if let Some(required) = required_dynamic_producer {
        let control_names: HashSet<String> = run
            .assignment
            .control_operators()
            .into_iter()
            .map(|op| op.name)
            .collect();
        if control_names.is_disjoint(required) {
            return Ok(None);
        }
    }

    place_system_producers(
        run.blueprint,
        run.operbox,
        &plan.producers,
        &mut run.assignment,
        &mut run.used,
    );
    assign_sphinx_urrbian_dorm_anchor(
        run.blueprint,
        run.operbox,
        &mut run.assignment,
        &mut run.used,
    );
    assign_dorm_producers(
        run.blueprint,
        run.operbox,
        run.instances,
        &mut run.assignment,
        &mut run.used,
    )?;

    let layout = run.resolve_snapshot(true)?;
    let mut trade_pool = base_trade_pool.clone();
    if karlan_precision_active(&layout.global_inject) {
        add_jie_market_to_trade_pool(&mut trade_pool, run.instances, run.table);
    }
    assign_power_stations(
        run.blueprint,
        power_pool,
        run.table,
        &layout,
        run.options,
        &mut run.assignment,
        &mut run.used,
    )?;
    assign_trade_remainder(
        run.blueprint,
        &trade_pool,
        run.table,
        run.instances,
        gold_lines,
        run.options,
        Some(run.durin_plan),
        &forbid_trade_station_pairs(plan),
        &mut run.assignment,
        &mut run.used,
    )?;
    cleanup_unused_sphinx_urrbian_dorm_anchor(run.blueprint, &mut run.assignment, &mut run.used);

    let final_layout = run.resolve_snapshot(true)?;
    Ok(Some(PeakPrefixCandidate {
        policy_sort_key: control_inject_policy_sort_key_for_layout(&final_layout),
        run,
    }))
}

/// 只为拥有实际贸易标签消费者的动态中枢 producer 建候选；按 atom 结构识别，
/// 不枚举任何消费方人员套餐。
fn dynamic_trade_producer_candidates(
    control_pool: &ControlPool,
    trade_pool: &TradePool,
    table: &SkillTable,
) -> Vec<String> {
    let consumer_tags: HashSet<&str> = trade_pool
        .entries
        .iter()
        .flat_map(|entry| entry.tags.iter().map(String::as_str))
        .collect();
    let mut producers: Vec<String> = control_pool
        .entries
        .iter()
        .filter(|entry| {
            control_entry_optional_dynamic_trade_tags(entry, table)
                .iter()
                .any(|tag| consumer_tags.contains(tag.as_str()))
        })
        .map(|entry| entry.name.clone())
        .collect();
    producers.sort();
    producers.dedup();
    producers
}

/// 从统一 plan 的 constraints 提取 forbid-same-room 名对，供制造 anchor 房搜索排除。
fn forbid_same_room_pairs(plan: &AssignmentPlan) -> Vec<(String, String)> {
    plan.constraints
        .iter()
        .filter_map(|c| match c {
            crate::layout::orchestrate::SystemConstraint::ForbidSameRoom { a, b } => {
                Some((a.clone(), b.clone()))
            }
            _ => None,
        })
        .collect()
}

fn forbid_trade_station_pairs(plan: &AssignmentPlan) -> Vec<(String, String)> {
    plan.constraints
        .iter()
        .filter_map(|constraint| match constraint {
            crate::layout::orchestrate::SystemConstraint::ForbidSameStation { a, b } => {
                Some((a.clone(), b.clone()))
            }
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::instances::{default_instances_path, OperatorInstances};
    use crate::layout::blueprint::{
        BlueprintScenario, FacilityKind, RoomBlueprint, RoomId, RoomProduct,
    };
    use crate::operbox::OperBoxEntry;
    use crate::skill_table::{default_skill_table_path, SkillTable};
    use crate::trade::TradeOrderKind;

    fn e2(name: &str) -> OperBoxEntry {
        OperBoxEntry {
            id: name.to_string(),
            name: name.to_string(),
            elite: 2,
            level: 90,
            own: true,
            potential: 0,
            rarity: 6,
        }
    }

    fn control_blueprint(with_trade: bool) -> BaseBlueprint {
        let mut rooms = vec![RoomBlueprint {
            id: RoomId::from("control"),
            kind: FacilityKind::ControlCenter,
            level: 5,
            product: None,
            dorm_beds: None,
            dorm_ambience_level: None,
        }];
        if with_trade {
            rooms.push(RoomBlueprint {
                id: RoomId::from("trade_one"),
                kind: FacilityKind::TradePost,
                level: 1,
                product: Some(RoomProduct::Trade {
                    order: TradeOrderKind::Gold,
                }),
                dorm_beds: None,
                dorm_ambience_level: None,
            });
            rooms.push(RoomBlueprint {
                id: RoomId::from("meeting"),
                kind: FacilityKind::MeetingRoom,
                level: 3,
                product: None,
                dorm_beds: None,
                dorm_ambience_level: None,
            });
        }
        BaseBlueprint {
            template: None,
            drone_cap: 135,
            scenario: BlueprintScenario::default(),
            rooms,
        }
    }

    fn control_candidates(include_vigil: bool) -> OperBox {
        let mut entries: Vec<_> = ["八幡海铃", "阿米娅", "Mon3tr", "焰尾", "薇薇安娜", "玛恩纳"]
            .into_iter()
            .map(e2)
            .collect();
        if include_vigil {
            entries.push(e2("伺夜"));
        }
        OperBox::from_entries(entries)
    }

    #[test]
    fn single_syracusa_trade_consumer_can_make_dynamic_control_prefix_win() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let assignment = run_shift_pipeline(
            &control_blueprint(true),
            &control_candidates(true),
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &BaseAssignment::default(),
            &AssignmentPlan::recovery(AssignShiftMode::Peak),
        )
        .unwrap();

        assert!(assignment
            .control_operators()
            .iter()
            .any(|operator| operator.name == "八幡海铃"));
        assert_eq!(
            assignment.operators_in(&RoomId::from("trade_one"))[0].name,
            "伺夜"
        );
    }

    #[test]
    fn no_syracusa_trade_consumer_does_not_force_dynamic_control_producer() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let assignment = run_shift_pipeline(
            &control_blueprint(false),
            &control_candidates(false),
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &BaseAssignment::default(),
            &AssignmentPlan::recovery(AssignShiftMode::Peak),
        )
        .unwrap();

        assert!(assignment
            .control_operators()
            .iter()
            .all(|operator| operator.name != "八幡海铃"));
    }

    #[test]
    fn single_consumer_does_not_force_haru_when_ordinary_control_is_better() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let operbox = OperBox::from_entries(
            [
                "古米",
                "夜刀",
                "斑点",
                "玫兰莎",
                "阿米娅",
                "八幡海铃",
                "伺夜",
            ]
            .into_iter()
            .map(e2)
            .collect(),
        );
        let mut seed = BaseAssignment::default();
        seed.set_room(
            "control",
            ["古米", "夜刀", "斑点", "玫兰莎"]
                .into_iter()
                .map(|name| crate::layout::AssignedOperator::new(name, 2))
                .collect(),
        );
        let assignment = run_shift_pipeline(
            &control_blueprint(true),
            &operbox,
            &instances,
            &table,
            &AssignBaseOptions {
                top_k: 20,
                ..Default::default()
            },
            AssignShiftMode::Peak,
            &seed,
            &AssignmentPlan::recovery(AssignShiftMode::Peak),
        )
        .unwrap();
        let control: HashSet<_> = assignment
            .control_operators()
            .into_iter()
            .map(|operator| operator.name)
            .collect();
        assert!(control.contains("阿米娅"), "control={control:?}");
        assert!(!control.contains("八幡海铃"), "control={control:?}");
        assert_eq!(
            assignment.operators_in(&RoomId::from("trade_one"))[0].name,
            "伺夜"
        );
    }

    #[test]
    fn dynamic_prefix_propagates_real_resolve_errors_instead_of_falling_back() {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        let table = SkillTable::load(&default_skill_table_path().unwrap()).unwrap();
        let operbox = control_candidates(true);
        let mut blueprint = control_blueprint(true);
        blueprint
            .rooms
            .iter_mut()
            .find(|room| room.kind == FacilityKind::TradePost)
            .unwrap()
            .product = None;
        let options = AssignBaseOptions::default();
        let plan = AssignmentPlan::recovery(AssignShiftMode::Peak);
        let control_pool = build_control_pool_with_fillers(&operbox, &instances, &table).unwrap();
        let trade_pool =
            build_trade_pool(&operbox.trade_roster(&instances), &instances, &table).unwrap();
        let power_pool =
            build_power_pool(&operbox.power_roster(&instances), &instances, &table).unwrap();
        let run = AssignmentRun::new(
            &blueprint,
            &operbox,
            &instances,
            &table,
            &options,
            BaseAssignment::default(),
            HashSet::new(),
        );

        let result = run_peak_prefix_candidate(
            run,
            &plan,
            &control_pool,
            &trade_pool,
            &power_pool,
            0,
            Some(&HashSet::from(["八幡海铃".to_string()])),
        );
        assert!(result.is_err(), "真实 resolve 错误不得降级成不可行候选");
    }
}
