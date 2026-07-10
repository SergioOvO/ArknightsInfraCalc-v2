mod evaluate;
mod rag_context;
mod rules;
mod types;

pub use evaluate::build_training_advice;
pub use rules::{default_training_recommendations_path, load_training_recommendation_rules};
pub use types::{
    OperatorTrainingState, PickOneCoreRule, RagContextItem, RecommendationKind,
    RecommendationPriority, StandaloneRecommendationRule, SystemRecommendationRule, SystemStatus,
    TrainingAdviceOptions, TrainingAdviceReport, TrainingAdviceSummary, TrainingRecommendation,
    TrainingRecommendationRules, TrainingSystemReport, TrainingTarget,
};
