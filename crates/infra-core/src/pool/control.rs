use std::collections::HashSet;

use crate::control::ControlOperator;
use crate::error::Result;
use crate::instances::OperatorInstances;
use crate::roster::{OperatorProgress, Roster};
use crate::skill_table::SkillTable;
use crate::tier::PromotionTier;

use crate::layout::tier::OperatorTier;
use crate::operbox::OperBox;

use super::base::{build_roster_pool, filter_pool, HasName, HasProgress, PoolCore, TierTagged};
pub use super::trade::PoolSkip;

#[derive(Debug, Clone)]
pub struct ControlPoolEntry {
    pub name: String,
    pub elite: u8,
    pub progress: OperatorProgress,
    pub buff_ids: Vec<String>,
    pub tags: Vec<String>,
    pub tier: OperatorTier,
}

impl HasName for ControlPoolEntry {
    fn pool_name(&self) -> &str {
        &self.name
    }
}

impl HasProgress for ControlPoolEntry {
    fn progress(&self) -> OperatorProgress {
        self.progress
    }
}

impl TierTagged for ControlPoolEntry {
    fn tier(&self) -> OperatorTier {
        self.tier
    }

    fn set_tier(&mut self, tier: OperatorTier) {
        self.tier = tier;
    }
}

impl ControlPoolEntry {
    pub fn to_control_operator(&self) -> ControlOperator {
        ControlOperator {
            name: self.name.clone(),
            elite: self.elite,
            buff_ids: self.buff_ids.clone(),
            tags: self.tags.clone(),
        }
    }
}

/// 向后兼容别名
pub type ControlPool = PoolCore<ControlPoolEntry>;

pub fn build_control_pool(
    roster: &Roster,
    instances: &OperatorInstances,
    table: &SkillTable,
) -> Result<ControlPool> {
    // control 按 name 排序（无 eff hint），用常数 0.0 作为 sort_key 统一排序逻辑
    build_roster_pool(roster, instances, table, |_| 0.0, try_entry)
}

/// Build the modeled control pool plus a bounded set of skill-less legal fillers.
pub fn build_control_pool_with_fillers(
    operbox: &OperBox,
    instances: &OperatorInstances,
    table: &SkillTable,
) -> Result<ControlPool> {
    let mut pool = build_control_pool(&operbox.control_roster(instances), instances, table)?;
    let existing: HashSet<String> = pool
        .entries
        .iter()
        .map(|entry| entry.name.clone())
        .collect();
    let roster = operbox.roster();
    for name in roster
        .names()
        .filter(|name| !existing.contains(*name))
        .take(5)
    {
        let Some(progress) = roster.progress(name) else {
            continue;
        };
        let tier = PromotionTier::from_progress(progress);
        let tags = instances
            .get(name, tier)
            .map(|instance| instance.tags.clone())
            .unwrap_or_default();
        pool.entries.push(ControlPoolEntry {
            name: name.to_string(),
            elite: progress.elite,
            progress,
            buff_ids: Vec::new(),
            tags,
            tier: OperatorTier::Standalone,
        });
    }
    Ok(pool)
}

pub fn filter_control_pool(pool: &ControlPool, exclude: &HashSet<String>) -> ControlPool {
    filter_pool(pool, exclude)
}

fn try_entry(
    name: &str,
    progress: crate::roster::OperatorProgress,
    instances: &OperatorInstances,
    table: &SkillTable,
) -> std::result::Result<ControlPoolEntry, PoolSkip> {
    let tier = PromotionTier::from_progress(progress);
    let inst = instances.get(name, tier);
    let buff_ids = instances.resolve_control_buff_ids(name, tier);
    for bid in &buff_ids {
        let Some(skill) = table.get(bid) else {
            return Err(PoolSkip::UnmodeledBuff(bid.clone()));
        };
        if skill.facility != "control" {
            return Err(PoolSkip::UnmodeledBuff(bid.clone()));
        }
    }

    let tags = inst.map(|i| i.tags.clone()).unwrap_or_default();

    Ok(ControlPoolEntry {
        name: name.to_string(),
        elite: progress.elite,
        progress,
        buff_ids,
        tags,
        tier: OperatorTier::Standalone,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_control_pool_adds_bounded_skillless_fillers() {
        let operbox =
            OperBox::load(&crate::operbox::default_operbox_full_e2_path().unwrap()).unwrap();
        let instances =
            OperatorInstances::load(&crate::instances::default_instances_path().unwrap()).unwrap();
        let table =
            SkillTable::load(&crate::skill_table::default_skill_table_path().unwrap()).unwrap();
        let skilled =
            build_control_pool(&operbox.control_roster(&instances), &instances, &table).unwrap();
        let with_fillers = build_control_pool_with_fillers(&operbox, &instances, &table).unwrap();

        assert!(with_fillers.entries.len() <= skilled.entries.len() + 5);
        assert!(with_fillers
            .entries
            .iter()
            .any(|entry| entry.buff_ids.is_empty()));
    }
}
