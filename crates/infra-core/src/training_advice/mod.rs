mod evaluate;
mod rag;
mod rules;
mod types;

pub use evaluate::build_training_advice;
pub use rag::{build_training_advice_rag_input, render_training_advice_answer};
pub use rules::{default_training_recommendations_path, load_training_recommendation_rules};
pub use types::{
    AcquisitionMode, AcquisitionPolicy, BlockedRuleReport, EvidenceRef, EvidenceSnippet,
    MemberBenefit, MemberRole, MissingCoreGroupReport, OperatorAdviceItem, OperatorTrainingState,
    PickOneCoreSlot, RecommendationAction, RecommendationPriority, RequiredCoreGroup, ReviewStatus,
    RuleAdmission, RuleKind, RuleMatch, RuleMember, RuleReview, RuleScope, TrainingAdviceBundle,
    TrainingAdviceFact, TrainingAdviceOptions, TrainingAdviceRagInput, TrainingAdviceReport,
    TrainingAdviceSummary, TrainingRecommendationRules, TrainingRule, TrainingTarget,
};
