use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::skill_table::data_path;

use super::types::TrainingRecommendationRules;

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
    serde_json::from_str(&raw).map_err(|e| {
        Error::msg(format!(
            "training recommendations parse {}: {e}",
            path.display()
        ))
    })
}
