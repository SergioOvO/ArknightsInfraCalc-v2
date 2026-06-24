//! 工具人表（散件干员速查）：Phase 3 白名单过滤，缩小 C(n,k) 搜索空间。
//!
//! # 数据来源
//!
//! `data/standalone_roster.json`（维护者 @knightcode，源文档 `散件干员速查.md`）。
//!
//! # 过滤策略
//!
//! - 优先使用白名单内干员搜索
//! - 过滤后可用条目不足 `min_entries`（贸易/制造 = 3，发电/中枢 = 1）时回退全池
//! - `OnceLock` 惰性缓存，首次调用后常驻

use std::collections::HashSet;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::skill_table::data_path;
use crate::FacilityKind;

use super::base::{HasName, PoolCore};

/// JSON 文件顶层结构。
#[derive(Debug, Clone, Deserialize)]
struct StandaloneRosterFile {
    #[serde(default)]
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    #[allow(dead_code)]
    tier: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    source: String,
    operators: StandaloneOperatorsFile,
}

#[derive(Debug, Clone, Deserialize)]
struct StandaloneOperatorsFile {
    #[serde(default)]
    trade_post: Vec<String>,
    #[serde(default)]
    factory: Vec<String>,
    #[serde(default)]
    power_plant: Vec<String>,
    #[serde(default)]
    control_center: Vec<String>,
}

/// 工具人表运行时缓存：按设施类型索引的干员名集合。
#[derive(Debug, Clone)]
struct StandaloneRoster {
    trade_post: HashSet<String>,
    factory: HashSet<String>,
    power_plant: HashSet<String>,
    control_center: HashSet<String>,
}

impl StandaloneRoster {
    fn get(&self, facility: FacilityKind) -> Option<&HashSet<String>> {
        match facility {
            FacilityKind::TradePost => Some(&self.trade_post),
            FacilityKind::Factory => Some(&self.factory),
            FacilityKind::PowerPlant => Some(&self.power_plant),
            FacilityKind::ControlCenter => Some(&self.control_center),
            _ => None,
        }
    }
}

fn facility_label(facility: FacilityKind) -> &'static str {
    match facility {
        FacilityKind::TradePost => "贸易站",
        FacilityKind::Factory => "制造站",
        FacilityKind::PowerPlant => "发电站",
        FacilityKind::ControlCenter => "中枢",
        FacilityKind::Dormitory => "宿舍",
        FacilityKind::Office => "办公室",
        _ => "其他",
    }
}

static ROSTER_CACHE: OnceLock<Option<StandaloneRoster>> = OnceLock::new();

/// 加载并缓存工具人表（惰性，首次调用后常驻）。
fn load_standalone_roster() -> Option<&'static StandaloneRoster> {
    ROSTER_CACHE
        .get_or_init(|| {
            let path = data_path("standalone_roster.json").ok()?;
            let raw = std::fs::read_to_string(&path).ok()?;
            let file: StandaloneRosterFile = serde_json::from_str(&raw).ok()?;
            let ops = file.operators;
            let roster = StandaloneRoster {
                trade_post: ops.trade_post.into_iter().collect(),
                factory: ops.factory.into_iter().collect(),
                power_plant: ops.power_plant.into_iter().collect(),
                control_center: ops.control_center.into_iter().collect(),
            };
            let total = roster.trade_post.len()
                + roster.factory.len()
                + roster.power_plant.len()
                + roster.control_center.len();
            if total == 0 {
                eprintln!("[工具人表] 已加载 {} 但所有设施列表为空", path.display());
                return None;
            }
            eprintln!(
                "[工具人表] 已加载 (v{}) {}: 贸易{}人, 制造{}人, 发电{}人, 中枢{}人",
                file.version,
                path.display(),
                roster.trade_post.len(),
                roster.factory.len(),
                roster.power_plant.len(),
                roster.control_center.len(),
            );
            Some(roster)
        })
        .as_ref()
}

/// 尝试用工具人表白名单过滤池。
///
/// 返回过滤后的池。如果工具人表未加载、该设施无白名单、或过滤后可用条目
/// 不足 `min_entries`，则返回原池（兜底回退）。
///
/// `min_entries`：贸易/制造 = 3（三人组最低要求），发电 = 1，中枢 = 1。
pub fn try_filter_standalone<T: HasName + Clone>(
    pool: &PoolCore<T>,
    facility: FacilityKind,
    min_entries: usize,
) -> PoolCore<T> {
    let Some(roster) = load_standalone_roster() else {
        eprintln!(
            "[工具人表] {}: 未加载，使用全池 (n={})",
            facility_label(facility),
            pool.entries.len(),
        );
        return pool.clone();
    };
    let Some(names) = roster.get(facility) else {
        eprintln!(
            "[工具人表] {}: 无此设施白名单，使用全池 (n={})",
            facility_label(facility),
            pool.entries.len(),
        );
        return pool.clone();
    };
    let before = pool.entries.len();
    let filtered: Vec<T> = pool
        .entries
        .iter()
        .filter(|e| names.contains(e.pool_name()))
        .cloned()
        .collect();
    let after = filtered.len();
    if after < min_entries {
        eprintln!(
            "[工具人表] {}: 过滤后仅 {}/{} 人 (最低需要{}人)，回退全池 (n={})",
            facility_label(facility),
            after,
            before,
            min_entries,
            before,
        );
        return pool.clone();
    }
    eprintln!(
        "[工具人表] {}: 过滤 {before}→{after} 人 (最低需要{min_entries}人)",
        facility_label(facility),
    );
    PoolCore {
        entries: filtered,
        skipped: pool.skipped.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试用最小 `HasName` 实现。
    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestEntry(String);

    impl HasName for TestEntry {
        fn pool_name(&self) -> &str {
            &self.0
        }
    }

    fn make_pool(names: &[&str]) -> PoolCore<TestEntry> {
        PoolCore {
            entries: names.iter().map(|n| TestEntry(n.to_string())).collect(),
            skipped: vec![],
        }
    }

    #[test]
    fn roster_loads_all_facilities() {
        let roster = load_standalone_roster().expect("standalone_roster.json should load");
        assert!(!roster.trade_post.is_empty());
        assert!(!roster.factory.is_empty());
        assert!(!roster.power_plant.is_empty());
        assert!(!roster.control_center.is_empty());
    }

    #[test]
    fn trade_post_has_expected_entries() {
        let roster = load_standalone_roster().unwrap();
        assert!(roster.trade_post.contains("空弦"));
        assert!(roster.trade_post.contains("吉星"));
        assert!(roster.trade_post.contains("石英"));
        assert!(
            !roster.trade_post.contains("但书"),
            "但书是体系核，不应在工具人表"
        );
        assert!(
            !roster.trade_post.contains("巫恋"),
            "巫恋是体系核，不应在工具人表"
        );
    }

    #[test]
    fn filter_preserves_whitelisted_entries() {
        let pool = make_pool(&["空弦", "吉星", "无关干员A", "石英", "无关干员B"]);
        let result = try_filter_standalone(&pool, FacilityKind::TradePost, 3);
        let names: Vec<&str> = result.entries.iter().map(|e| e.pool_name()).collect();
        assert!(names.contains(&"空弦"));
        assert!(names.contains(&"吉星"));
        assert!(names.contains(&"石英"));
        assert!(!names.contains(&"无关干员A"));
        assert!(!names.contains(&"无关干员B"));
    }

    #[test]
    fn filter_falls_back_when_too_few() {
        let pool = make_pool(&["无关干员A", "无关干员B"]);
        // 白名单里没有这两个，过滤后空，不足 3 → 回退原池
        let result = try_filter_standalone(&pool, FacilityKind::TradePost, 3);
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].pool_name(), "无关干员A");
    }

    #[test]
    fn filter_falls_back_when_no_roster_for_facility() {
        let pool = make_pool(&["a", "b", "c"]);
        // Dormitory 不在工具人表中 → 回退
        let result = try_filter_standalone(&pool, FacilityKind::Dormitory, 1);
        assert_eq!(result.entries.len(), 3);
    }
}
