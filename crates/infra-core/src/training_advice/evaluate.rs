use std::collections::{BTreeSet, HashMap, HashSet};

use crate::instances::OperatorInstances;
use crate::operbox::OperBox;
use crate::tier::PromotionTier;
use crate::Result;

use super::rag_context::build_rag_context;
use super::types::{
    OperatorTrainingState, PickOneCoreRule, RecommendationKind, StandaloneRecommendationRule,
    SystemRecommendationRule, SystemStatus, TrainingAdviceOptions, TrainingAdviceReport,
    TrainingAdviceSummary, TrainingRecommendation, TrainingRecommendationRules,
    TrainingSystemReport, TrainingTarget,
};

pub fn build_training_advice(
    operbox: &OperBox,
    instances: &OperatorInstances,
    rules: &TrainingRecommendationRules,
    options: &TrainingAdviceOptions,
) -> Result<TrainingAdviceReport> {
    let mut collector = RecommendationCollector::default();
    let mut systems = Vec::new();

    for rule in &rules.system_rules {
        let (system, recommendations) = evaluate_system_rule(operbox, rules, rule);
        for rec in recommendations {
            collector.push(rec);
        }
        systems.push(system);
    }

    for rule in &rules.standalone_rules {
        let system_reports = evaluate_standalone_rule(operbox, rules, rule, &mut collector);
        systems.extend(system_reports);
    }

    let recommendations = collector.into_sorted_vec();
    let summary = TrainingAdviceSummary {
        owned: operbox.owned_count(),
        modelled_owned: modelled_owned_count(operbox, instances),
        ready_systems: systems
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    SystemStatus::Ready | SystemStatus::StandaloneReady
                )
            })
            .count(),
        blocked_systems: systems
            .iter()
            .filter(|s| {
                matches!(
                    s.status,
                    SystemStatus::PartialBlocked | SystemStatus::Missing
                )
            })
            .count(),
        trainable_recommendations: recommendations
            .iter()
            .filter(|r| r.kind == RecommendationKind::Train)
            .count(),
    };
    let rag_context = build_rag_context(&recommendations, &systems);

    Ok(TrainingAdviceReport {
        schema_version: 1,
        operbox_label: options
            .operbox_label
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        summary,
        recommendations,
        systems,
        rag_context,
    })
}

fn evaluate_standalone_rule(
    operbox: &OperBox,
    rules: &TrainingRecommendationRules,
    rule: &StandaloneRecommendationRule,
    collector: &mut RecommendationCollector,
) -> Vec<TrainingSystemReport> {
    let mut systems = Vec::new();
    let source_paths = source_paths(&rule.source_paths);
    let needs_review = rule.needs_review;

    for target in &rule.targets {
        match operbox.progress_of(&target.name) {
            Some(progress) if target.is_met_by(progress) => {
                systems.push(TrainingSystemReport {
                    id: format!("standalone:{}", target.name),
                    label: format!("{}：{}", rule.label, target.name),
                    status: SystemStatus::StandaloneReady,
                    owned_core: vec![target.name.clone()],
                    missing_core: Vec::new(),
                    undertrained_core: Vec::new(),
                    blocked_reason: None,
                    source_paths: source_paths.clone(),
                    needs_review,
                });
            }
            Some(progress) => {
                systems.push(TrainingSystemReport {
                    id: format!("standalone:{}", target.name),
                    label: format!("{}：{}", rule.label, target.name),
                    status: SystemStatus::StandaloneTrainable,
                    owned_core: vec![target.name.clone()],
                    missing_core: Vec::new(),
                    undertrained_core: vec![target.name.clone()],
                    blocked_reason: None,
                    source_paths: source_paths.clone(),
                    needs_review,
                });
                collector.push(TrainingRecommendation {
                    priority: rule.priority,
                    kind: RecommendationKind::Train,
                    operator: target.name.clone(),
                    target: target_state(target),
                    current: Some(progress.into()),
                    reason_code: rule.reason_code.clone(),
                    system_id: None,
                    related_systems: Vec::new(),
                    message: recommendation_message(&rule.message, needs_review),
                    source_paths: inherited_source_paths(rules, &rule.source_paths),
                    needs_review,
                });
            }
            None => {}
        }
    }

    systems
}

fn evaluate_system_rule(
    operbox: &OperBox,
    rules: &TrainingRecommendationRules,
    rule: &SystemRecommendationRule,
) -> (TrainingSystemReport, Vec<TrainingRecommendation>) {
    let mut owned_core = BTreeSet::new();
    let mut missing_core = BTreeSet::new();
    let mut undertrained_core = BTreeSet::new();
    let mut recommendations = Vec::new();

    for target in &rule.core {
        evaluate_named_target(
            operbox,
            target,
            &mut owned_core,
            &mut missing_core,
            &mut undertrained_core,
        );
    }

    let mut pick_train_targets = Vec::new();
    for pick in &rule.pick_one_core {
        let slot = evaluate_pick_one(operbox, pick);
        owned_core.extend(slot.owned);
        match slot.status {
            PickOneStatus::Satisfied => {}
            PickOneStatus::Trainable(target) => {
                undertrained_core.insert(target.name.clone());
                pick_train_targets.push(target);
            }
            PickOneStatus::Missing(label) => {
                missing_core.insert(label);
            }
        }
    }

    let missing_core: Vec<String> = missing_core.into_iter().collect();
    let undertrained_core_vec: Vec<String> = undertrained_core.iter().cloned().collect();
    let owned_core: Vec<String> = owned_core.into_iter().collect();
    let status = if missing_core.is_empty() && undertrained_core_vec.is_empty() {
        SystemStatus::Ready
    } else if missing_core.is_empty() {
        SystemStatus::ReadyAfterTraining
    } else if owned_core.is_empty() {
        SystemStatus::Missing
    } else {
        SystemStatus::PartialBlocked
    };

    if status == SystemStatus::ReadyAfterTraining {
        for target in rule
            .core
            .iter()
            .filter(|target| undertrained_core.contains(&target.name))
            .cloned()
            .chain(pick_train_targets)
        {
            if let Some(progress) = operbox.progress_of(&target.name) {
                recommendations.push(TrainingRecommendation {
                    priority: rule.priority_ready_after_training,
                    kind: RecommendationKind::Train,
                    operator: target.name.clone(),
                    target: target_state(&target),
                    current: Some(progress.into()),
                    reason_code: rule.reason_code.clone(),
                    system_id: Some(rule.id.clone()),
                    related_systems: vec![rule.id.clone()],
                    message: recommendation_message(&rule.message, rule.needs_review),
                    source_paths: inherited_source_paths(rules, &rule.source_paths),
                    needs_review: rule.needs_review,
                });
            }
        }
    }

    let blocked_reason = match status {
        SystemStatus::PartialBlocked => Some(format!("missing core: {}", missing_core.join(", "))),
        SystemStatus::Missing => Some("missing core operators".to_string()),
        _ => None,
    };

    (
        TrainingSystemReport {
            id: rule.id.clone(),
            label: rule.label.clone(),
            status,
            owned_core,
            missing_core,
            undertrained_core: undertrained_core_vec,
            blocked_reason,
            source_paths: source_paths(&rule.source_paths),
            needs_review: rule.needs_review,
        },
        recommendations,
    )
}

fn evaluate_named_target(
    operbox: &OperBox,
    target: &TrainingTarget,
    owned_core: &mut BTreeSet<String>,
    missing_core: &mut BTreeSet<String>,
    undertrained_core: &mut BTreeSet<String>,
) {
    match operbox.progress_of(&target.name) {
        Some(progress) => {
            owned_core.insert(target.name.clone());
            if !target.is_met_by(progress) {
                undertrained_core.insert(target.name.clone());
            }
        }
        None => {
            missing_core.insert(target.name.clone());
        }
    }
}

struct PickOneEvaluation {
    status: PickOneStatus,
    owned: Vec<String>,
}

enum PickOneStatus {
    Satisfied,
    Trainable(TrainingTarget),
    Missing(String),
}

fn evaluate_pick_one(operbox: &OperBox, pick: &PickOneCoreRule) -> PickOneEvaluation {
    let mut owned = Vec::new();
    let mut trainable = Vec::new();

    for name in &pick.candidates {
        if let Some(progress) = operbox.progress_of(name) {
            owned.push(name.clone());
            let target = TrainingTarget {
                name: name.clone(),
                elite: pick.elite,
                level: pick.level,
            };
            if target.is_met_by(progress) {
                return PickOneEvaluation {
                    status: PickOneStatus::Satisfied,
                    owned,
                };
            }
            trainable.push(target);
        }
    }

    let status = trainable
        .into_iter()
        .next()
        .map(PickOneStatus::Trainable)
        .unwrap_or_else(|| PickOneStatus::Missing(pick.label.clone()));
    PickOneEvaluation { status, owned }
}

fn modelled_owned_count(operbox: &OperBox, instances: &OperatorInstances) -> usize {
    operbox
        .owned
        .keys()
        .filter(|name| {
            instances.get(name, PromotionTier::Tier0).is_some()
                || instances.get(name, PromotionTier::TierUp).is_some()
        })
        .count()
}

fn target_state(target: &TrainingTarget) -> OperatorTrainingState {
    OperatorTrainingState {
        elite: target.elite,
        level: target.level,
    }
}

fn recommendation_message(message: &str, needs_review: bool) -> String {
    let base = if message.trim().is_empty() {
        "已拥有但未达到推荐练度。"
    } else {
        message.trim()
    };
    if needs_review {
        format!("待复核：{base}")
    } else {
        base.to_string()
    }
}

fn source_paths(paths: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    paths
        .iter()
        .filter(|p| seen.insert((*p).clone()))
        .cloned()
        .collect()
}

fn inherited_source_paths(rules: &TrainingRecommendationRules, paths: &[String]) -> Vec<String> {
    let mut out = BTreeSet::new();
    if let Some(source_repo) = &rules.source_repo {
        out.insert(source_repo.clone());
    }
    out.extend(paths.iter().cloned());
    out.into_iter().collect()
}

#[derive(Default)]
struct RecommendationCollector {
    by_operator: HashMap<String, TrainingRecommendation>,
}

impl RecommendationCollector {
    fn push(&mut self, rec: TrainingRecommendation) {
        self.by_operator
            .entry(rec.operator.clone())
            .and_modify(|existing| merge_recommendation(existing, &rec))
            .or_insert(rec);
    }

    fn into_sorted_vec(self) -> Vec<TrainingRecommendation> {
        let mut out: Vec<_> = self.by_operator.into_values().collect();
        out.sort_by(|a, b| {
            a.priority
                .rank()
                .cmp(&b.priority.rank())
                .then_with(|| a.operator.cmp(&b.operator))
        });
        out
    }
}

fn merge_recommendation(existing: &mut TrainingRecommendation, incoming: &TrainingRecommendation) {
    let incoming_wins = incoming.priority.rank() < existing.priority.rank()
        || (incoming.priority == existing.priority
            && existing.system_id.is_none()
            && incoming.system_id.is_some());

    let related = merged_strings(&existing.related_systems, &incoming.related_systems);
    let source_paths = merged_strings(&existing.source_paths, &incoming.source_paths);
    let needs_review = existing.needs_review || incoming.needs_review;

    if incoming_wins {
        *existing = incoming.clone();
    }

    existing.related_systems = related;
    existing.source_paths = source_paths;
    existing.needs_review = needs_review;
    if needs_review && !existing.message.starts_with("待复核：") {
        existing.message = format!("待复核：{}", existing.message);
    }
}

fn merged_strings(a: &[String], b: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    a.iter()
        .chain(b.iter())
        .filter(|v| seen.insert((*v).clone()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::operbox::{OperBox, OperBoxEntry};
    use crate::{instances::default_instances_path, OperatorInstances};

    use super::super::types::RecommendationPriority;
    use super::*;

    fn entry(name: &str, elite: u8, own: bool) -> OperBoxEntry {
        OperBoxEntry {
            id: name.to_string(),
            name: name.to_string(),
            elite,
            level: 1,
            own,
            potential: 0,
            rarity: 5,
        }
    }

    fn operbox(entries: Vec<OperBoxEntry>) -> OperBox {
        OperBox::from_entries(entries)
    }

    fn rules() -> TrainingRecommendationRules {
        TrainingRecommendationRules {
            version: 1,
            source_repo: Some("vault".to_string()),
            standalone_rules: vec![StandaloneRecommendationRule {
                id: "standalone_clear".to_string(),
                label: "常用精一四星".to_string(),
                priority: RecommendationPriority::P0,
                targets: vec![TrainingTarget {
                    name: "清流".to_string(),
                    elite: 1,
                    level: None,
                }],
                reason_code: "standalone_must_train".to_string(),
                message: "精一即可启用常用基建技能。".to_string(),
                source_paths: vec!["docs/4-散件工具人/散件干员速查.md".to_string()],
                source_repo: None,
                source_notes: None,
                needs_review: false,
                conflicts: Vec::new(),
            }],
            system_rules: vec![
                SystemRecommendationRule {
                    id: "witch_long_beta".to_string(),
                    label: "巫恋组".to_string(),
                    source_system_id: None,
                    priority_ready_after_training: RecommendationPriority::P0,
                    priority_blocked: RecommendationPriority::Info,
                    core: vec![
                        TrainingTarget {
                            name: "巫恋".to_string(),
                            elite: 2,
                            level: None,
                        },
                        TrainingTarget {
                            name: "龙舌兰".to_string(),
                            elite: 2,
                            level: None,
                        },
                    ],
                    pick_one_core: vec![PickOneCoreRule {
                        label: "裁缝β第三人".to_string(),
                        elite: 2,
                        level: None,
                        candidates: vec!["卡夫卡".to_string(), "柏喙".to_string()],
                    }],
                    reason_code: "system_core_undertrained".to_string(),
                    message: "体系核心已齐，但有人未达到体系要求。".to_string(),
                    source_paths: vec!["docs/2-体系/巫恋裁缝核.md".to_string()],
                    source_repo: None,
                    source_notes: None,
                    needs_review: false,
                    conflicts: Vec::new(),
                },
                SystemRecommendationRule {
                    id: "review_system".to_string(),
                    label: "待复核体系".to_string(),
                    source_system_id: None,
                    priority_ready_after_training: RecommendationPriority::P1,
                    priority_blocked: RecommendationPriority::Info,
                    core: vec![TrainingTarget {
                        name: "槐琥".to_string(),
                        elite: 2,
                        level: None,
                    }],
                    pick_one_core: Vec::new(),
                    reason_code: "needs_review_case".to_string(),
                    message: "该规则来源需要人工确认。".to_string(),
                    source_paths: vec!["vault/review.md".to_string()],
                    source_repo: None,
                    source_notes: None,
                    needs_review: true,
                    conflicts: vec!["conflicting source".to_string()],
                },
            ],
        }
    }

    fn report(box_: OperBox, rules: TrainingRecommendationRules) -> TrainingAdviceReport {
        let instances = OperatorInstances::load(&default_instances_path().unwrap()).unwrap();
        build_training_advice(
            &box_,
            &instances,
            &rules,
            &TrainingAdviceOptions {
                operbox_label: Some("test".to_string()),
            },
        )
        .unwrap()
    }

    #[test]
    fn partial_combo_does_not_train_owned_half_core() {
        let report = report(operbox(vec![entry("龙舌兰", 0, true)]), rules());
        assert!(report
            .systems
            .iter()
            .any(|s| s.id == "witch_long_beta" && s.status == SystemStatus::PartialBlocked));
        assert!(report
            .recommendations
            .iter()
            .all(|r| r.operator != "龙舌兰"));
    }

    #[test]
    fn complete_combo_undertrained_emits_p0_train() {
        let report = report(
            operbox(vec![
                entry("巫恋", 1, true),
                entry("龙舌兰", 2, true),
                entry("卡夫卡", 1, true),
            ]),
            rules(),
        );
        assert!(report.systems.iter().any(|s| {
            s.id == "witch_long_beta" && s.status == SystemStatus::ReadyAfterTraining
        }));
        assert!(report.recommendations.iter().any(|r| {
            r.operator == "巫恋"
                && r.priority == RecommendationPriority::P0
                && r.kind == RecommendationKind::Train
        }));
    }

    #[test]
    fn pick_one_satisfied_by_any_ready_candidate() {
        let report = report(
            operbox(vec![
                entry("巫恋", 2, true),
                entry("龙舌兰", 2, true),
                entry("柏喙", 2, true),
            ]),
            rules(),
        );
        assert!(report
            .systems
            .iter()
            .any(|s| s.id == "witch_long_beta" && s.status == SystemStatus::Ready));
        assert!(report.recommendations.is_empty());
    }

    #[test]
    fn pick_one_trainable_when_other_core_complete() {
        let report = report(
            operbox(vec![
                entry("巫恋", 2, true),
                entry("龙舌兰", 2, true),
                entry("卡夫卡", 1, true),
            ]),
            rules(),
        );
        assert!(report
            .recommendations
            .iter()
            .any(|r| r.operator == "卡夫卡"));
    }

    #[test]
    fn duplicate_operator_keeps_highest_priority_and_related_systems() {
        let mut rules = rules();
        rules.system_rules.push(SystemRecommendationRule {
            id: "clearwater_system".to_string(),
            label: "清流体系".to_string(),
            source_system_id: None,
            priority_ready_after_training: RecommendationPriority::P1,
            priority_blocked: RecommendationPriority::Info,
            core: vec![TrainingTarget {
                name: "清流".to_string(),
                elite: 2,
                level: None,
            }],
            pick_one_core: Vec::new(),
            reason_code: "system_core_undertrained".to_string(),
            message: "体系需要更高练度。".to_string(),
            source_paths: vec!["docs/system.md".to_string()],
            source_repo: None,
            source_notes: None,
            needs_review: false,
            conflicts: Vec::new(),
        });
        let report = report(operbox(vec![entry("清流", 0, true)]), rules);
        let recs: Vec<_> = report
            .recommendations
            .iter()
            .filter(|r| r.operator == "清流")
            .collect();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].priority, RecommendationPriority::P0);
        assert!(recs[0]
            .related_systems
            .contains(&"clearwater_system".to_string()));
    }

    #[test]
    fn unowned_operator_does_not_train() {
        let report = report(operbox(vec![entry("清流", 0, false)]), rules());
        assert!(report.recommendations.iter().all(|r| r.operator != "清流"));
    }

    #[test]
    fn needs_review_is_propagated() {
        let report = report(operbox(vec![entry("槐琥", 1, true)]), rules());
        let rec = report
            .recommendations
            .iter()
            .find(|r| r.operator == "槐琥")
            .unwrap();
        assert!(rec.needs_review);
        assert!(rec.message.starts_with("待复核："));
    }

    #[test]
    fn report_schema_version_is_one() {
        let report = report(operbox(vec![entry("清流", 0, true)]), rules());
        assert_eq!(report.schema_version, 1);
    }

    #[test]
    fn default_rules_emit_the_filtered_standalone_training_list() {
        let box_ = OperBox::from_json(include_str!(
            "../../../../data/fixtures/training_advice/standalone_e1_four_star.json"
        ))
        .unwrap();
        let rules = crate::training_advice::load_training_recommendation_rules(
            &crate::training_advice::default_training_recommendations_path().unwrap(),
        )
        .unwrap();
        let report = report(box_, rules);

        let actual: HashMap<_, _> = report
            .recommendations
            .iter()
            .map(|rec| (rec.operator.as_str(), rec.priority))
            .collect();
        // Fixture only owns the original core standalone set; expanded rules
        // must still emit those operators at the same priorities.
        let expected = [
            ("石英", RecommendationPriority::P0),
            ("清流", RecommendationPriority::P0),
            ("砾", RecommendationPriority::P0),
            ("断罪者", RecommendationPriority::P0),
            ("Castle-3", RecommendationPriority::P0),
            ("慕斯", RecommendationPriority::P1),
            ("缠丸", RecommendationPriority::P1),
            ("安比尔", RecommendationPriority::P1),
            ("斑点", RecommendationPriority::P1),
            ("霜叶", RecommendationPriority::P1),
            ("白雪", RecommendationPriority::P1),
            ("红豆", RecommendationPriority::P1),
            ("空弦", RecommendationPriority::P2),
            ("吉星", RecommendationPriority::P2),
            ("槐琥", RecommendationPriority::P2),
        ];

        for (name, priority) in expected {
            assert_eq!(actual.get(name), Some(&priority), "{name}");
        }
        assert_eq!(actual.len(), expected.len());
        let castle = report
            .recommendations
            .iter()
            .find(|rec| rec.operator == "Castle-3")
            .unwrap();
        assert_eq!(castle.target.elite, 0);
        assert_eq!(castle.target.level, Some(30));
    }

    #[test]
    fn castle_three_level_thirty_meets_the_default_target() {
        let mut castle = entry("Castle-3", 0, true);
        castle.level = 30;
        castle.rarity = 1;
        let rules = crate::training_advice::load_training_recommendation_rules(
            &crate::training_advice::default_training_recommendations_path().unwrap(),
        )
        .unwrap();
        let report = report(operbox(vec![castle]), rules);

        assert!(report
            .recommendations
            .iter()
            .all(|rec| rec.operator != "Castle-3"));
        assert!(report.systems.iter().any(|system| {
            system.id == "standalone:Castle-3" && system.status == SystemStatus::StandaloneReady
        }));
    }
}
