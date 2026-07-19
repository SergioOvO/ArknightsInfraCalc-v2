use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::skill_table::data_path;

use super::types::{MemberRole, ReviewStatus, RuleKind, TrainingRecommendationRules, TrainingRule};

pub fn default_training_recommendations_path() -> Result<PathBuf> {
    data_path("training_recommendations.json")
}

pub fn load_training_recommendation_rules(path: &Path) -> Result<TrainingRecommendationRules> {
    let raw = std::fs::read_to_string(path).map_err(|e| {
        Error::msg(format!(
            "training recommendations read {}: {e}",
            path.display()
        ))
    })?;
    let rules: TrainingRecommendationRules = serde_json::from_str(&raw).map_err(|e| {
        Error::msg(format!(
            "training recommendations parse {}: {e}",
            path.display()
        ))
    })?;
    validate_rules(&rules)?;
    Ok(rules)
}

pub fn validate_rules(rules: &TrainingRecommendationRules) -> Result<()> {
    if rules.version != 2 {
        return Err(Error::msg(format!(
            "training recommendations version must be 2, got {}",
            rules.version
        )));
    }

    let mut ids = HashSet::new();
    for rule in &rules.rules {
        if !ids.insert(rule.id.clone()) {
            return Err(Error::msg(format!("duplicate rule id: {}", rule.id)));
        }
        validate_rule(rule)?;
    }
    Ok(())
}

fn validate_rule(rule: &TrainingRule) -> Result<()> {
    if rule.members.is_empty() {
        return Err(Error::msg(format!("rule {} has no members", rule.id)));
    }

    let hard = matches!(rule.kind, RuleKind::System | RuleKind::Combo);
    if matches!(rule.kind, RuleKind::Standalone | RuleKind::SoftCombo) {
        if !rule.admission.required_core.is_empty() || !rule.admission.pick_one_core.is_empty() {
            return Err(Error::msg(format!(
                "rule {} kind {:?} must not declare hard core admission",
                rule.id, rule.kind
            )));
        }
    }

    if hard && rule.admission.required_core.is_empty() && rule.admission.pick_one_core.is_empty() {
        return Err(Error::msg(format!(
            "rule {} kind {:?} requires admission core",
            rule.id, rule.kind
        )));
    }

    for name in &rule.admission.required_core {
        let member = rule.members.iter().find(|m| m.operator == *name);
        match member {
            Some(m) if matches!(m.role, MemberRole::Core) => {}
            Some(m) => {
                return Err(Error::msg(format!(
                    "rule {} required_core {} has role {:?}",
                    rule.id, name, m.role
                )));
            }
            None => {
                return Err(Error::msg(format!(
                    "rule {} required_core {} missing from members",
                    rule.id, name
                )));
            }
        }
    }

    for slot in &rule.admission.pick_one_core {
        if slot.candidates.is_empty() {
            return Err(Error::msg(format!(
                "rule {} pick_one slot {} has no candidates",
                rule.id, slot.label
            )));
        }
        for cand in &slot.candidates {
            let member = rule.members.iter().find(|m| m.operator == *cand);
            match member {
                Some(m) if matches!(m.role, MemberRole::Core) => {}
                Some(m) => {
                    return Err(Error::msg(format!(
                        "rule {} pick_one candidate {} has role {:?}",
                        rule.id, cand, m.role
                    )));
                }
                None => {
                    return Err(Error::msg(format!(
                        "rule {} pick_one candidate {} missing from members",
                        rule.id, cand
                    )));
                }
            }
        }
    }

    if rule.review.status == ReviewStatus::NeedsReview && rule.review.conflicts.is_empty() {
        return Err(Error::msg(format!(
            "rule {} needs_review but conflicts empty",
            rule.id
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::training_advice::types::*;

    #[test]
    fn default_rules_are_v2_and_valid() {
        let rules =
            load_training_recommendation_rules(&default_training_recommendations_path().unwrap())
                .unwrap();
        assert_eq!(rules.version, 2);
        assert!(!rules.rules.is_empty());
    }

    #[test]
    fn natural_syracusa_members_are_not_a_training_core_rule() {
        let rules =
            load_training_recommendation_rules(&default_training_recommendations_path().unwrap())
                .unwrap();
        for forbidden in ["docus_syracusa", "syracusa_pair", "syracusa_cross_station"] {
            assert!(rules.rules.iter().all(|rule| {
                rule.id != forbidden && rule.source_system_id.as_deref() != Some(forbidden)
            }));
        }
    }

    #[test]
    fn standalone_with_hard_core_is_rejected() {
        let rules = TrainingRecommendationRules {
            version: 2,
            acquisition_policy: AcquisitionPolicy::default(),
            rules: vec![TrainingRule {
                id: "bad".to_string(),
                kind: RuleKind::Standalone,
                scope: RuleScope::Independent,
                label: "bad".to_string(),
                source_system_id: None,
                admission: RuleAdmission {
                    required_core: vec!["清流".to_string()],
                    pick_one_core: Vec::new(),
                },
                members: vec![RuleMember {
                    operator: "清流".to_string(),
                    role: MemberRole::Core,
                    target: TrainingTarget {
                        elite: 1,
                        level: None,
                        skill_id: None,
                        skill_name: None,
                    },
                    priority: RecommendationPriority::P0,
                    acquisition: AcquisitionMode::Policy,
                    rarity: Some(4),
                    benefit: None,
                }],
                evidence: Vec::new(),
                review: RuleReview::default(),
            }],
        };
        assert!(validate_rules(&rules).is_err());
    }
}
