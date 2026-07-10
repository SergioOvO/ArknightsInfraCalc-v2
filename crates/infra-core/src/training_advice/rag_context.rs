use std::collections::BTreeSet;

use super::types::{RagContextItem, TrainingRecommendation, TrainingSystemReport};

pub fn build_rag_context(
    recommendations: &[TrainingRecommendation],
    systems: &[TrainingSystemReport],
) -> Vec<RagContextItem> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();

    for rec in recommendations {
        let key = format!("operator:{}:{}", rec.operator, rec.reason_code);
        if seen.insert(key.clone()) {
            out.push(RagContextItem {
                key,
                kind: "recommendation".to_string(),
                operator: Some(rec.operator.clone()),
                system_id: rec.system_id.clone(),
                label: None,
                reason_code: Some(rec.reason_code.clone()),
                source_paths: rec.source_paths.clone(),
                needs_review: rec.needs_review,
            });
        }
    }

    for system in systems {
        let key = format!("system:{}", system.id);
        if seen.insert(key.clone()) {
            out.push(RagContextItem {
                key,
                kind: "system".to_string(),
                operator: None,
                system_id: Some(system.id.clone()),
                label: Some(system.label.clone()),
                reason_code: None,
                source_paths: system.source_paths.clone(),
                needs_review: system.needs_review,
            });
        }
    }

    out
}
