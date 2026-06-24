//! 干员三层分类：跨站体系 / 同站组合 / 效率工具人。
//!
//! 分配优先级：跨站 > 同站 > 工具人。`select_registry_systems` 按此顺序
//! 分两轮贪心认领（工具人不经 registry，由 `try_filter_standalone` 过滤）。

use serde::{Deserialize, Serialize};

/// 三层分配优先级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorTier {
    /// 跨站体系：slot 跨越中枢+贸易/制造等多设施（如叙拉古链、喀兰链）。
    CrossStation,
    /// 同站组合：slot 在同一设施内（如巫恋组、企鹅物流 bond）。
    SameStation,
    /// 效率工具人：无固定 bond 的通用高效率干员（`standalone_roster.json` 白名单）。
    Standalone,
}

impl OperatorTier {
    /// 分配优先级（数值越大越先处理）。
    pub fn priority(self) -> u8 {
        match self {
            Self::CrossStation => 2,
            Self::SameStation => 1,
            Self::Standalone => 0,
        }
    }
}
