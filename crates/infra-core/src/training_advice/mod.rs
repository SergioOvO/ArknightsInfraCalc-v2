mod evaluate;
mod rules;
mod types;

pub use evaluate::build_training_advice;
pub use rules::{default_training_recommendations_path, load_training_recommendation_rules};
pub use types::{
    AcquisitionMode, AcquisitionPolicy, BlockedRuleReport, EvidenceRef, MemberBenefit, MemberRole,
    OperatorAdviceItem, OperatorTrainingState, PickOneCoreSlot, RecommendationAction,
    RecommendationPriority, ReviewStatus, RuleAdmission, RuleKind, RuleMatch, RuleMember,
    RuleReview, RuleScope, TrainingAdviceOptions, TrainingAdviceReport, TrainingAdviceSummary,
    TrainingRecommendationRules, TrainingRule, TrainingTarget,
};
