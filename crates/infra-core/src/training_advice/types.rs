use serde::{Deserialize, Serialize};

use crate::roster::OperatorProgress;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecommendationPriority {
    P0,
    P1,
    P2,
    Info,
}

impl RecommendationPriority {
    pub fn rank(self) -> u8 {
        match self {
            Self::P0 => 0,
            Self::P1 => 1,
            Self::P2 => 2,
            Self::Info => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationKind {
    Train,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemStatus {
    Ready,
    ReadyAfterTraining,
    PartialBlocked,
    Missing,
    StandaloneReady,
    StandaloneTrainable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingTarget {
    pub name: String,
    pub elite: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

impl TrainingTarget {
    pub fn is_met_by(&self, progress: OperatorProgress) -> bool {
        if progress.elite > self.elite {
            return true;
        }
        if progress.elite < self.elite {
            return false;
        }
        self.level.is_none_or(|level| progress.level >= level)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorTrainingState {
    pub elite: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

impl From<OperatorProgress> for OperatorTrainingState {
    fn from(value: OperatorProgress) -> Self {
        Self {
            elite: value.elite,
            level: Some(value.level),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PickOneCoreRule {
    pub label: String,
    pub elite: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    pub candidates: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandaloneRecommendationRule {
    pub id: String,
    pub label: String,
    pub priority: RecommendationPriority,
    pub targets: Vec<TrainingTarget>,
    pub reason_code: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub source_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_notes: Option<String>,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemRecommendationRule {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_system_id: Option<String>,
    pub priority_ready_after_training: RecommendationPriority,
    #[serde(default = "default_info_priority")]
    pub priority_blocked: RecommendationPriority,
    pub core: Vec<TrainingTarget>,
    #[serde(default)]
    pub pick_one_core: Vec<PickOneCoreRule>,
    #[serde(default)]
    pub important: Vec<TrainingTarget>,
    #[serde(default)]
    pub hangers: Vec<TrainingTarget>,
    #[serde(default = "default_hanger_priority")]
    pub priority_hangers: RecommendationPriority,
    pub reason_code: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub source_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_notes: Option<String>,
    #[serde(default)]
    pub needs_review: bool,
    #[serde(default)]
    pub conflicts: Vec<String>,
}

fn default_info_priority() -> RecommendationPriority {
    RecommendationPriority::Info
}

fn default_hanger_priority() -> RecommendationPriority {
    RecommendationPriority::P2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRecommendationRules {
    pub version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_repo: Option<String>,
    #[serde(default)]
    pub standalone_rules: Vec<StandaloneRecommendationRule>,
    #[serde(default)]
    pub system_rules: Vec<SystemRecommendationRule>,
}

#[derive(Debug, Clone, Default)]
pub struct TrainingAdviceOptions {
    pub operbox_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingAdviceReport {
    pub schema_version: u32,
    pub operbox_label: String,
    pub summary: TrainingAdviceSummary,
    pub recommendations: Vec<TrainingRecommendation>,
    pub systems: Vec<TrainingSystemReport>,
    pub rag_context: Vec<RagContextItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingAdviceSummary {
    pub owned: usize,
    pub modelled_owned: usize,
    pub ready_systems: usize,
    pub blocked_systems: usize,
    pub trainable_recommendations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingRecommendation {
    pub priority: RecommendationPriority,
    pub kind: RecommendationKind,
    pub operator: String,
    pub target: OperatorTrainingState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<OperatorTrainingState>,
    pub reason_code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_id: Option<String>,
    #[serde(default)]
    pub related_systems: Vec<String>,
    pub message: String,
    #[serde(default)]
    pub source_paths: Vec<String>,
    #[serde(default)]
    pub needs_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSystemReport {
    pub id: String,
    pub label: String,
    pub status: SystemStatus,
    #[serde(default)]
    pub owned_core: Vec<String>,
    #[serde(default)]
    pub missing_core: Vec<String>,
    #[serde(default)]
    pub undertrained_core: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
    #[serde(default)]
    pub source_paths: Vec<String>,
    #[serde(default)]
    pub needs_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagContextItem {
    pub key: String,
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason_code: Option<String>,
    #[serde(default)]
    pub source_paths: Vec<String>,
    #[serde(default)]
    pub needs_review: bool,
}
