use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use serde::Serialize;

use crate::error::{Error, Result};
use crate::instances::{default_instances_path, OperatorInstances};
use crate::layout::LayoutContext;
use crate::manufacture::ManuSearchRecipeMode;
use crate::pool::{build_manufacture_pool, build_trade_pool};
use crate::roster::{OperatorProgress, Roster};
use crate::search::{
    search_manufacture_triples, search_trade_triples, ManuSearchHit, ManuSearchOptions,
    TradeSearchHit, TradeSearchOptions,
};
use crate::skill_table::{data_path, default_skill_table_path, SkillTable};
use crate::trade::input::{TradeOrderKind, TradeSearchOrderMode};
use crate::types::RecipeKind;

pub const BAKE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone)]
pub struct BakeOptions {
    pub out_dir: PathBuf,
    pub include_trade: bool,
    pub include_manufacture: bool,
    pub limit_per_signature: Option<usize>,
}

impl Default for BakeOptions {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("data/baked"),
            include_trade: true,
            include_manufacture: true,
            limit_per_signature: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BakeReport {
    pub out_dir: PathBuf,
    pub schema_version: u32,
    pub operator_count: usize,
    pub trade_signatures: usize,
    pub trade_hits: usize,
    pub manufacture_signatures: usize,
    pub manufacture_hits: usize,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize)]
struct BakeManifest {
    schema_version: u32,
    generated_by: &'static str,
    model: &'static str,
    inputs: Vec<BakeInputFingerprint>,
    options: BakeManifestOptions,
}

#[derive(Debug, Serialize)]
struct BakeManifestOptions {
    include_trade: bool,
    include_manufacture: bool,
    limit_per_signature: Option<usize>,
    roster_model: &'static str,
    layout_model: &'static str,
}

#[derive(Debug, Serialize)]
struct BakeInputFingerprint {
    path: String,
    bytes: u64,
    hash64: String,
}

#[derive(Debug, Clone, Serialize)]
struct BakedOperator {
    name: String,
    progress: OperatorProgressJson,
    facilities: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct OperatorProgressJson {
    elite: u8,
    level: u32,
    rarity: u8,
}

impl From<OperatorProgress> for OperatorProgressJson {
    fn from(value: OperatorProgress) -> Self {
        Self {
            elite: value.elite,
            level: value.level,
            rarity: value.rarity,
        }
    }
}

#[derive(Debug, Serialize)]
struct BakedTradeCatalog {
    schema_version: u32,
    signatures: Vec<BakedTradeSignature>,
}

#[derive(Debug, Serialize)]
struct BakedTradeSignature {
    room_level: u8,
    operator_capacity: usize,
    order_kind: TradeOrderKind,
    gold_production_lines: u32,
    hits: Vec<TradeSearchHit>,
}

#[derive(Debug, Serialize)]
struct BakedManufactureCatalog {
    schema_version: u32,
    signatures: Vec<BakedManufactureSignature>,
}

#[derive(Debug, Serialize)]
struct BakedManufactureSignature {
    room_level: u8,
    operator_capacity: usize,
    recipe: RecipeKind,
    hits: Vec<ManuSearchHit>,
}

pub fn bake_catalogs(options: &BakeOptions) -> Result<BakeReport> {
    let start = Instant::now();
    fs::create_dir_all(&options.out_dir)?;

    let instances_path = default_instances_path()?;
    let skill_table_path = default_skill_table_path()?;
    let instances = OperatorInstances::load(&instances_path)?;
    let table = SkillTable::load(&skill_table_path)?;
    let roster = bake_roster(&instances);
    let operators = baked_operators(&instances, &roster);

    write_json(options.out_dir.join("operators.json"), &operators)?;

    let mut trade_signatures = 0usize;
    let mut trade_hits = 0usize;
    if options.include_trade {
        let catalog = bake_trade_catalog(&roster, &instances, &table, options)?;
        trade_signatures = catalog.signatures.len();
        trade_hits = catalog.signatures.iter().map(|s| s.hits.len()).sum();
        write_json(options.out_dir.join("trade_combos.json"), &catalog)?;
    }

    let mut manufacture_signatures = 0usize;
    let mut manufacture_hits = 0usize;
    if options.include_manufacture {
        let catalog = bake_manufacture_catalog(&roster, &instances, &table, options)?;
        manufacture_signatures = catalog.signatures.len();
        manufacture_hits = catalog.signatures.iter().map(|s| s.hits.len()).sum();
        write_json(options.out_dir.join("manufacture_combos.json"), &catalog)?;
    }

    let manifest = BakeManifest {
        schema_version: BAKE_SCHEMA_VERSION,
        generated_by: "infra-core::bake",
        model: "baseline_tier_up_operator_catalog",
        inputs: bake_input_fingerprints()?,
        options: BakeManifestOptions {
            include_trade: options.include_trade,
            include_manufacture: options.include_manufacture,
            limit_per_signature: options.limit_per_signature,
            roster_model: "all modelled trade/manufacture operators at elite2 level1 rarity6",
            layout_model: "LayoutContext::search_baseline",
        },
    };
    write_json(options.out_dir.join("manifest.json"), &manifest)?;

    let report = BakeReport {
        out_dir: options.out_dir.clone(),
        schema_version: BAKE_SCHEMA_VERSION,
        operator_count: operators.len(),
        trade_signatures,
        trade_hits,
        manufacture_signatures,
        manufacture_hits,
        elapsed_ms: start.elapsed().as_millis(),
    };
    write_json(options.out_dir.join("summary.json"), &report)?;
    Ok(report)
}

fn bake_trade_catalog(
    roster: &Roster,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &BakeOptions,
) -> Result<BakedTradeCatalog> {
    let pool = build_trade_pool(roster, instances, table)?;
    let mut signatures = Vec::new();

    for room_level in 1..=3 {
        for order_kind in [TradeOrderKind::Gold, TradeOrderKind::Originium] {
            let search_opts = TradeSearchOptions {
                trade_level: room_level,
                operator_capacity: station_operator_capacity(room_level),
                top_k: options.limit_per_signature.unwrap_or(usize::MAX),
                layout: std::sync::Arc::new(LayoutContext::search_baseline()),
                order_mode: TradeSearchOrderMode::Single(order_kind),
                ..TradeSearchOptions::default()
            };
            let report = search_trade_triples(&pool, table, &search_opts)?;
            signatures.push(BakedTradeSignature {
                room_level,
                operator_capacity: search_opts.operator_capacity,
                order_kind,
                gold_production_lines: search_opts.gold_production_lines,
                hits: report.top,
            });
        }
    }

    Ok(BakedTradeCatalog {
        schema_version: BAKE_SCHEMA_VERSION,
        signatures,
    })
}

fn bake_manufacture_catalog(
    roster: &Roster,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &BakeOptions,
) -> Result<BakedManufactureCatalog> {
    let pool = build_manufacture_pool(roster, instances, table)?;
    let mut signatures = Vec::new();

    for room_level in 1..=3 {
        for recipe in [
            RecipeKind::Gold,
            RecipeKind::BattleRecord,
            RecipeKind::Originium,
        ] {
            let search_opts = ManuSearchOptions {
                level: room_level,
                operator_capacity: station_operator_capacity(room_level),
                recipe_mode: ManuSearchRecipeMode::Single(recipe),
                top_k: options.limit_per_signature.unwrap_or(usize::MAX),
                layout: std::sync::Arc::new(LayoutContext::search_baseline()),
                ..ManuSearchOptions::default()
            };
            let report = search_manufacture_triples(&pool, table, &search_opts)?;
            signatures.push(BakedManufactureSignature {
                room_level,
                operator_capacity: search_opts.operator_capacity,
                recipe,
                hits: report.top,
            });
        }
    }

    Ok(BakedManufactureCatalog {
        schema_version: BAKE_SCHEMA_VERSION,
        signatures,
    })
}

fn bake_roster(instances: &OperatorInstances) -> Roster {
    let mut roster = Roster::default();
    for name in modelled_production_operator_names(instances) {
        roster.insert(name, OperatorProgress::new(2, 1, 6));
    }
    roster
}

fn station_operator_capacity(level: u8) -> usize {
    level.clamp(1, 3) as usize
}

fn baked_operators(instances: &OperatorInstances, roster: &Roster) -> Vec<BakedOperator> {
    let mut operators: Vec<_> = roster
        .names()
        .map(|name| {
            let mut facilities = Vec::new();
            for facility in ["trade", "manufacture"] {
                if instances
                    .get(name, crate::tier::PromotionTier::TierUp)
                    .and_then(|i| i.facilities.get(facility))
                    .is_some()
                    || instances
                        .get(name, crate::tier::PromotionTier::Tier0)
                        .and_then(|i| i.facilities.get(facility))
                        .is_some()
                {
                    facilities.push(facility.to_string());
                }
            }
            BakedOperator {
                name: name.clone(),
                progress: roster
                    .progress(name)
                    .unwrap_or_else(|| OperatorProgress::new(2, 1, 6))
                    .into(),
                facilities,
            }
        })
        .collect();
    operators.sort_by(|a, b| a.name.cmp(&b.name));
    operators
}

fn modelled_production_operator_names(instances: &OperatorInstances) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (_, instance) in instances.iter() {
        if instance.facilities.contains_key("trade")
            || instance.facilities.contains_key("manufacture")
        {
            names.insert(instance.name.clone());
        }
    }
    names
}

fn bake_input_fingerprints() -> Result<Vec<BakeInputFingerprint>> {
    let mut paths = BTreeMap::new();
    for name in [
        "operator_instances.json",
        "skill_table.json",
        "trade_shortcuts.json",
        "trade_segments.json",
        "base_systems.json",
    ] {
        let path = data_path(name)?;
        paths.insert(name, path);
    }
    paths
        .into_values()
        .map(|path| fingerprint_file(&path))
        .collect()
}

fn fingerprint_file(path: &Path) -> Result<BakeInputFingerprint> {
    let bytes = fs::read(path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(BakeInputFingerprint {
        path: path.to_string_lossy().to_string(),
        bytes: bytes.len() as u64,
        hash64: format!("{:016x}", hasher.finish()),
    })
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let file = File::create(&path)?;
    serde_json::to_writer_pretty(file, value)
        .map_err(|e| Error::msg(format!("write {}: {e}", path.display())))
}
