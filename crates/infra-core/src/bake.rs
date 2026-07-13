use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use std::time::Instant as StdInstant;

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::efficiency::Efficiency;
use crate::error::{Error, Result};
use crate::instances::{default_instances_path, OperatorInstances};
use crate::layout::LayoutContext;
use crate::manufacture::input::ManuRoomInput;
use crate::manufacture::solver::solve_manufacture;
use crate::pool::{
    build_manufacture_pool, build_trade_combo_operators_vec, build_trade_pool,
    combinations_indices, n_choose_k_u64,
};
use crate::pool::{standalone_names_for, StandaloneFilter};
use crate::pool::{HasName, ManuPool, PoolCore, TradePool};
use crate::roster::{OperatorProgress, Roster};
use crate::search::{
    ManuEfficiencyBreakdown, ManuSearchHit, ManuSearchOptions, ManuSearchReport,
    SearchTripleFilter, TradeEfficiencyBreakdown, TradeSearchHit, TradeSearchOptions,
    TradeSearchReport,
};
use crate::skill_table::{data_path, default_skill_table_path, SkillTable};
use crate::trade::input::{TradeOrderKind, TradeRoomInput, TradeSearchOrderMode};
use crate::trade::shortcut::trade_station_exclusive_violation;
use crate::trade::solver::solve_trade_with_shift_prevalidated;
use crate::types::RecipeKind;
use crate::FacilityKind;

pub const BAKE_SCHEMA_VERSION: u32 = 10;

pub type BakeProgressCallback = Arc<dyn Fn(BakeProgressEvent) + Send + Sync>;

#[derive(Clone)]
pub struct BakeOptions {
    pub out_dir: PathBuf,
    pub include_trade: bool,
    pub include_manufacture: bool,
    pub limit_per_signature: Option<usize>,
    pub generator: Option<BakeGeneratorFingerprint>,
    pub progress: Option<BakeProgressCallback>,
}

impl Default for BakeOptions {
    fn default() -> Self {
        Self {
            out_dir: PathBuf::from("data/baked"),
            include_trade: true,
            include_manufacture: true,
            limit_per_signature: None,
            generator: None,
            progress: None,
        }
    }
}

impl std::fmt::Debug for BakeOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BakeOptions")
            .field("out_dir", &self.out_dir)
            .field("include_trade", &self.include_trade)
            .field("include_manufacture", &self.include_manufacture)
            .field("limit_per_signature", &self.limit_per_signature)
            .field("generator", &self.generator)
            .field("progress", &self.progress.as_ref().map(|_| "<callback>"))
            .finish()
    }
}

pub fn default_baked_out_dir() -> Result<PathBuf> {
    let roots = default_baked_data_roots()?;
    for root in &roots {
        let out_dir = root.join("baked");
        if out_dir.join("manifest.json").exists() {
            return Ok(out_dir);
        }
    }
    Ok(roots
        .into_iter()
        .next()
        .unwrap_or_else(|| PathBuf::from("data"))
        .join("baked"))
}

fn default_baked_data_roots() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("ARKNIGHTS_INFRA_DATA_DIR") {
        push_unique_path(&mut roots, PathBuf::from(root));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            push_unique_path(&mut roots, exe_dir.join("data"));
            if let Some(bundle_parent) = exe_dir.parent() {
                push_unique_path(&mut roots, bundle_parent.join("data"));
            }
        }
    }
    push_unique_path(
        &mut roots,
        std::env::current_dir().map_err(Error::from)?.join("data"),
    );
    Ok(roots)
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|candidate| candidate == &path) {
        paths.push(path);
    }
}

#[derive(Debug, Clone)]
pub enum BakeProgressEvent {
    Started {
        out_dir: PathBuf,
        operator_count: usize,
        signature_count: usize,
        worker_count: usize,
    },
    SignatureStarted {
        facility: &'static str,
        signature_key: String,
        combo_count: u64,
    },
    SignatureFinished {
        facility: &'static str,
        signature_key: String,
        rows: usize,
        elapsed_ms: u128,
    },
    Writing {
        path: PathBuf,
        rows: Option<usize>,
    },
    Finished {
        combo_table_rows: usize,
        elapsed_ms: u128,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BakeGeneratorFingerprint {
    pub kind: String,
    pub path: String,
    pub bytes: u64,
    pub hash64: String,
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
    pub combo_table_rows: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generator: Option<BakeGeneratorFingerprint>,
    pub elapsed_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
struct BakeManifest {
    schema_version: u32,
    generated_by: String,
    model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    generator: Option<BakeGeneratorFingerprint>,
    inputs: Vec<BakeInputFingerprint>,
    options: BakeManifestOptions,
}

#[derive(Debug, Serialize, Deserialize)]
struct BakeManifestOptions {
    include_trade: bool,
    include_manufacture: bool,
    limit_per_signature: Option<usize>,
    roster_model: String,
    layout_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
struct BakedComboTable {
    schema_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    generator: Option<BakeGeneratorFingerprint>,
    operator_count: usize,
    #[serde(default)]
    operator_names: Vec<String>,
    mask_words: usize,
    indexes: Vec<BakedComboIndex>,
    rows: Vec<BakedComboRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BakedComboTableDisk {
    schema_version: u32,
    generator: Option<BakeGeneratorFingerprint>,
    operator_count: usize,
    operator_names: Vec<String>,
    mask_words: usize,
    indexes: Vec<BakedComboIndex>,
    rows: Vec<BakedComboRowDisk>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BakedComboRowDisk {
    room_level: u8,
    operator_capacity: usize,
    operator_indices: Vec<usize>,
    sort_efficiency_millis: i32,
    order_kind: Option<TradeOrderKind>,
    recipe: Option<RecipeKind>,
    trade_base_efficiency_millis: Option<i32>,
    trade_occupancy_efficiency_millis: Option<i32>,
    trade_skill_efficiency_millis: Option<i32>,
    trade_control_efficiency_millis: Option<i32>,
    trade_paper_efficiency_millis: Option<i32>,
    trade_mechanic_equivalent_efficiency_millis: Option<i32>,
    trade_unit_output_multiplier_millis: Option<i32>,
    trade_final_efficiency_millis: Option<i32>,
    trade_equivalent_skill_efficiency_millis: Option<i32>,
    rule_id: Option<String>,
    unit_trade_per_day: Option<f64>,
    unit_gold_per_day: Option<f64>,
    unit_originium_per_day: Option<f64>,
    manufacture_base_efficiency_millis: Option<i32>,
    manufacture_occupancy_efficiency_millis: Option<i32>,
    manufacture_skill_efficiency_millis: Option<i32>,
    manufacture_global_efficiency_millis: Option<i32>,
    manufacture_final_efficiency_millis: Option<i32>,
    manufacture_storage_limit: Option<i32>,
}

impl From<&BakedComboTable> for BakedComboTableDisk {
    fn from(value: &BakedComboTable) -> Self {
        Self {
            schema_version: value.schema_version,
            generator: value.generator.clone(),
            operator_count: value.operator_count,
            operator_names: value.operator_names.clone(),
            mask_words: value.mask_words,
            indexes: value.indexes.clone(),
            rows: value.rows.iter().map(BakedComboRowDisk::from).collect(),
        }
    }
}

impl From<&BakedComboRow> for BakedComboRowDisk {
    fn from(value: &BakedComboRow) -> Self {
        Self {
            room_level: value.room_level,
            operator_capacity: value.operator_capacity,
            operator_indices: value.operator_indices.clone(),
            sort_efficiency_millis: value.sort_efficiency.millis(),
            order_kind: value.order_kind,
            recipe: value.recipe,
            trade_base_efficiency_millis: value.trade_base_efficiency.map(Efficiency::millis),
            trade_occupancy_efficiency_millis: value
                .trade_occupancy_efficiency
                .map(Efficiency::millis),
            trade_skill_efficiency_millis: value.trade_skill_efficiency.map(Efficiency::millis),
            trade_control_efficiency_millis: value.trade_control_efficiency.map(Efficiency::millis),
            trade_paper_efficiency_millis: value.trade_paper_efficiency.map(Efficiency::millis),
            trade_mechanic_equivalent_efficiency_millis: value
                .trade_mechanic_equivalent_efficiency
                .map(Efficiency::millis),
            trade_unit_output_multiplier_millis: value
                .trade_unit_output_multiplier
                .map(Efficiency::millis),
            trade_final_efficiency_millis: value.trade_final_efficiency.map(Efficiency::millis),
            trade_equivalent_skill_efficiency_millis: value
                .trade_equivalent_skill_efficiency
                .map(Efficiency::millis),
            rule_id: value.rule_id.clone(),
            unit_trade_per_day: value.unit_trade_per_day,
            unit_gold_per_day: value.unit_gold_per_day,
            unit_originium_per_day: value.unit_originium_per_day,
            manufacture_base_efficiency_millis: value
                .manufacture_base_efficiency
                .map(Efficiency::millis),
            manufacture_occupancy_efficiency_millis: value
                .manufacture_occupancy_efficiency
                .map(Efficiency::millis),
            manufacture_skill_efficiency_millis: value
                .manufacture_skill_efficiency
                .map(Efficiency::millis),
            manufacture_global_efficiency_millis: value
                .manufacture_global_efficiency
                .map(Efficiency::millis),
            manufacture_final_efficiency_millis: value
                .manufacture_final_efficiency
                .map(Efficiency::millis),
            manufacture_storage_limit: value.manufacture_storage_limit,
        }
    }
}

impl From<BakedComboTableDisk> for BakedComboTable {
    fn from(value: BakedComboTableDisk) -> Self {
        Self {
            schema_version: value.schema_version,
            generator: value.generator,
            operator_count: value.operator_count,
            operator_names: value.operator_names,
            mask_words: value.mask_words,
            indexes: value.indexes,
            rows: value.rows.into_iter().map(BakedComboRow::from).collect(),
        }
    }
}

impl From<BakedComboRowDisk> for BakedComboRow {
    fn from(value: BakedComboRowDisk) -> Self {
        Self {
            row_id: 0,
            facility: String::new(),
            signature_key: String::new(),
            room_level: value.room_level,
            operator_capacity: value.operator_capacity,
            names: Vec::new(),
            operator_indices: value.operator_indices,
            operator_mask: Vec::new(),
            sort_efficiency: Efficiency::from_millis(value.sort_efficiency_millis),
            order_kind: value.order_kind,
            recipe: value.recipe,
            trade_base_efficiency: value
                .trade_base_efficiency_millis
                .map(Efficiency::from_millis),
            trade_occupancy_efficiency: value
                .trade_occupancy_efficiency_millis
                .map(Efficiency::from_millis),
            trade_skill_efficiency: value
                .trade_skill_efficiency_millis
                .map(Efficiency::from_millis),
            trade_control_efficiency: value
                .trade_control_efficiency_millis
                .map(Efficiency::from_millis),
            trade_paper_efficiency: value
                .trade_paper_efficiency_millis
                .map(Efficiency::from_millis),
            trade_mechanic_equivalent_efficiency: value
                .trade_mechanic_equivalent_efficiency_millis
                .map(Efficiency::from_millis),
            trade_unit_output_multiplier: value
                .trade_unit_output_multiplier_millis
                .map(Efficiency::from_millis),
            trade_final_efficiency: value
                .trade_final_efficiency_millis
                .map(Efficiency::from_millis),
            trade_equivalent_skill_efficiency: value
                .trade_equivalent_skill_efficiency_millis
                .map(Efficiency::from_millis),
            rule_id: value.rule_id,
            unit_trade_per_day: value.unit_trade_per_day,
            unit_gold_per_day: value.unit_gold_per_day,
            unit_originium_per_day: value.unit_originium_per_day,
            manufacture_base_efficiency: value
                .manufacture_base_efficiency_millis
                .map(Efficiency::from_millis),
            manufacture_occupancy_efficiency: value
                .manufacture_occupancy_efficiency_millis
                .map(Efficiency::from_millis),
            manufacture_skill_efficiency: value
                .manufacture_skill_efficiency_millis
                .map(Efficiency::from_millis),
            manufacture_global_efficiency: value
                .manufacture_global_efficiency_millis
                .map(Efficiency::from_millis),
            manufacture_final_efficiency: value
                .manufacture_final_efficiency_millis
                .map(Efficiency::from_millis),
            manufacture_storage_limit: value.manufacture_storage_limit,
        }
    }
}

#[derive(Debug)]
struct RuntimeBakedComboTable {
    table: BakedComboTable,
    index_by_key: HashMap<String, (usize, usize)>,
    operator_index_by_name: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BakedComboIndex {
    signature_key: String,
    start: usize,
    len: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct BakedComboRow {
    #[serde(default, skip_serializing)]
    row_id: usize,
    #[serde(rename = "f", default, skip_serializing)]
    facility: String,
    #[serde(default, skip_serializing)]
    signature_key: String,
    #[serde(rename = "l")]
    room_level: u8,
    #[serde(rename = "c")]
    operator_capacity: usize,
    #[serde(default, skip_serializing)]
    names: Vec<String>,
    #[serde(rename = "oi")]
    operator_indices: Vec<usize>,
    #[serde(default, skip_serializing)]
    operator_mask: Vec<u64>,
    #[serde(rename = "s")]
    sort_efficiency: Efficiency,
    #[serde(rename = "ok", skip_serializing_if = "Option::is_none")]
    order_kind: Option<TradeOrderKind>,
    #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
    recipe: Option<RecipeKind>,
    #[serde(rename = "tb", skip_serializing_if = "Option::is_none")]
    trade_base_efficiency: Option<Efficiency>,
    #[serde(rename = "to", skip_serializing_if = "Option::is_none")]
    trade_occupancy_efficiency: Option<Efficiency>,
    #[serde(rename = "ts", skip_serializing_if = "Option::is_none")]
    trade_skill_efficiency: Option<Efficiency>,
    #[serde(rename = "tc", skip_serializing_if = "Option::is_none")]
    trade_control_efficiency: Option<Efficiency>,
    #[serde(rename = "tp", skip_serializing_if = "Option::is_none")]
    trade_paper_efficiency: Option<Efficiency>,
    #[serde(rename = "tm", skip_serializing_if = "Option::is_none")]
    trade_mechanic_equivalent_efficiency: Option<Efficiency>,
    #[serde(rename = "tu", skip_serializing_if = "Option::is_none")]
    trade_unit_output_multiplier: Option<Efficiency>,
    #[serde(rename = "tf", skip_serializing_if = "Option::is_none")]
    trade_final_efficiency: Option<Efficiency>,
    #[serde(rename = "te", skip_serializing_if = "Option::is_none")]
    trade_equivalent_skill_efficiency: Option<Efficiency>,
    #[serde(rename = "ri", skip_serializing_if = "Option::is_none")]
    rule_id: Option<String>,
    #[serde(rename = "utd", skip_serializing_if = "Option::is_none")]
    unit_trade_per_day: Option<f64>,
    #[serde(rename = "ugd", skip_serializing_if = "Option::is_none")]
    unit_gold_per_day: Option<f64>,
    #[serde(rename = "uod", default, skip_serializing_if = "Option::is_none")]
    unit_originium_per_day: Option<f64>,
    #[serde(rename = "mb", skip_serializing_if = "Option::is_none")]
    manufacture_base_efficiency: Option<Efficiency>,
    #[serde(rename = "mo", skip_serializing_if = "Option::is_none")]
    manufacture_occupancy_efficiency: Option<Efficiency>,
    #[serde(rename = "ms", skip_serializing_if = "Option::is_none")]
    manufacture_skill_efficiency: Option<Efficiency>,
    #[serde(rename = "mg", skip_serializing_if = "Option::is_none")]
    manufacture_global_efficiency: Option<Efficiency>,
    #[serde(rename = "mf", skip_serializing_if = "Option::is_none")]
    manufacture_final_efficiency: Option<Efficiency>,
    #[serde(rename = "msl", default, skip_serializing_if = "Option::is_none")]
    manufacture_storage_limit: Option<i32>,
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

pub fn bake_catalogs(options: &BakeOptions) -> Result<BakeReport> {
    let start = Instant::now();
    fs::create_dir_all(&options.out_dir)?;

    let instances_path = default_instances_path()?;
    let skill_table_path = default_skill_table_path()?;
    let instances = OperatorInstances::load(&instances_path)?;
    let table = SkillTable::load(&skill_table_path)?;
    let roster = bake_roster(&instances);
    let operators = baked_operators(&instances, &roster);
    let signature_count = expected_signature_count(options);

    emit_progress(
        options,
        BakeProgressEvent::Started {
            out_dir: options.out_dir.clone(),
            operator_count: operators.len(),
            signature_count,
            worker_count: rayon::current_num_threads(),
        },
    );

    emit_progress(
        options,
        BakeProgressEvent::Writing {
            path: options.out_dir.join("operators.json"),
            rows: Some(operators.len()),
        },
    );
    write_json(options.out_dir.join("operators.json"), &operators)?;
    remove_stale_catalogs(&options.out_dir)?;

    let operator_index: HashMap<&str, usize> = operators
        .iter()
        .enumerate()
        .map(|(idx, op)| (op.name.as_str(), idx))
        .collect();
    let mask_words = operators.len().div_ceil(64).max(1);

    let mut trade_signatures = 0usize;
    let mut trade_hits = 0usize;
    let mut rows = Vec::new();
    if options.include_trade {
        let baked = bake_trade_rows(
            &roster,
            &instances,
            &table,
            options,
            &operator_index,
            mask_words,
        )?;
        trade_signatures = baked.signatures;
        trade_hits = baked.rows.len();
        rows.extend(baked.rows);
    }

    let mut manufacture_signatures = 0usize;
    let mut manufacture_hits = 0usize;
    if options.include_manufacture {
        let baked = bake_manufacture_rows(
            &roster,
            &instances,
            &table,
            options,
            &operator_index,
            mask_words,
        )?;
        manufacture_signatures = baked.signatures;
        manufacture_hits = baked.rows.len();
        rows.extend(baked.rows);
    }

    let operator_names = operators.iter().map(|op| op.name.clone()).collect();
    let combo_table =
        build_combo_table_from_rows(operator_names, mask_words, rows, options.generator.clone());
    let combo_table_rows = combo_table.rows.len();
    emit_progress(
        options,
        BakeProgressEvent::Writing {
            path: options.out_dir.join("combo_table.bin"),
            rows: Some(combo_table_rows),
        },
    );
    write_binary(
        options.out_dir.join("combo_table.bin"),
        &BakedComboTableDisk::from(&combo_table),
    )?;

    let manifest = BakeManifest {
        schema_version: BAKE_SCHEMA_VERSION,
        generated_by: "infra-core::bake".to_string(),
        model: "binary_single_room_combo_table".to_string(),
        generator: options.generator.clone(),
        inputs: bake_input_fingerprints()?,
        options: BakeManifestOptions {
            include_trade: options.include_trade,
            include_manufacture: options.include_manufacture,
            limit_per_signature: options.limit_per_signature,
            roster_model:
                "standalone_roster.json trade/manufacture entries at elite2 level1 rarity6"
                    .to_string(),
            layout_model: "single room signatures; gold trade keeps gold line count in key"
                .to_string(),
        },
    };
    emit_progress(
        options,
        BakeProgressEvent::Writing {
            path: options.out_dir.join("manifest.json"),
            rows: None,
        },
    );
    write_json(options.out_dir.join("manifest.json"), &manifest)?;

    let report = BakeReport {
        out_dir: options.out_dir.clone(),
        schema_version: BAKE_SCHEMA_VERSION,
        operator_count: operators.len(),
        trade_signatures,
        trade_hits,
        manufacture_signatures,
        manufacture_hits,
        combo_table_rows,
        generator: options.generator.clone(),
        elapsed_ms: start.elapsed().as_millis(),
    };
    emit_progress(
        options,
        BakeProgressEvent::Writing {
            path: options.out_dir.join("summary.json"),
            rows: None,
        },
    );
    write_json(options.out_dir.join("summary.json"), &report)?;
    emit_progress(
        options,
        BakeProgressEvent::Finished {
            combo_table_rows,
            elapsed_ms: start.elapsed().as_millis(),
        },
    );
    Ok(report)
}

fn emit_progress(options: &BakeOptions, event: BakeProgressEvent) {
    if let Some(callback) = &options.progress {
        callback(event);
    }
}

fn expected_signature_count(options: &BakeOptions) -> usize {
    let trade = if options.include_trade { 3 * 2 } else { 0 };
    let manufacture = if options.include_manufacture {
        3 * 3
    } else {
        0
    };
    trade + manufacture
}

fn remove_stale_catalogs(out_dir: &Path) -> Result<()> {
    for name in [
        "trade_combos.json",
        "manufacture_combos.json",
        "combo_table.json",
    ] {
        let path = out_dir.join(name);
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

struct BakedRows {
    signatures: usize,
    rows: Vec<BakedComboRow>,
}

fn filter_pool_to_names<T: HasName + Clone>(
    pool: &PoolCore<T>,
    names: Option<BTreeSet<String>>,
) -> PoolCore<T> {
    let Some(names) = names else {
        return PoolCore {
            entries: Vec::new(),
            skipped: pool.skipped.clone(),
        };
    };
    PoolCore {
        entries: pool
            .entries
            .iter()
            .filter(|entry| names.contains(entry.pool_name()))
            .cloned()
            .collect(),
        skipped: pool.skipped.clone(),
    }
}

fn bake_trade_rows(
    roster: &Roster,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &BakeOptions,
    operator_index: &HashMap<&str, usize>,
    mask_words: usize,
) -> Result<BakedRows> {
    let pool = build_trade_pool(roster, instances, table)?;
    let layout = Arc::new(LayoutContext::search_baseline());
    let gold_lines = if layout.gold_manu_line_count > 0 {
        layout.gold_manu_line_count
    } else {
        TradeSearchOptions::default().gold_production_lines
    };
    let mut all_rows = Vec::new();
    let mut signatures = 0usize;

    for room_level in 1..=3 {
        for order_kind in [TradeOrderKind::Gold, TradeOrderKind::Originium] {
            let operator_capacity = station_operator_capacity(room_level);
            let signature_key =
                trade_lookup_key(room_level, operator_capacity, order_kind, gold_lines);
            let combo_count = n_choose_k_u64(pool.entries.len(), operator_capacity);
            let sig_start = Instant::now();
            emit_progress(
                options,
                BakeProgressEvent::SignatureStarted {
                    facility: "trade",
                    signature_key: signature_key.clone(),
                    combo_count,
                },
            );

            let mut rows: Vec<_> = combinations_indices(pool.entries.len(), operator_capacity)
                .collect::<Vec<_>>()
                .par_iter()
                .filter_map(|combo| {
                    trade_combo_row(
                        &pool,
                        table,
                        room_level,
                        operator_capacity,
                        order_kind,
                        gold_lines,
                        &layout,
                        combo,
                        &signature_key,
                        operator_index,
                        mask_words,
                    )
                })
                .collect();
            sort_signature_rows(&mut rows);
            if let Some(limit) = options.limit_per_signature {
                rows.truncate(limit);
            }
            emit_progress(
                options,
                BakeProgressEvent::SignatureFinished {
                    facility: "trade",
                    signature_key,
                    rows: rows.len(),
                    elapsed_ms: sig_start.elapsed().as_millis(),
                },
            );
            signatures += 1;
            all_rows.extend(rows);
        }
    }

    Ok(BakedRows {
        signatures,
        rows: all_rows,
    })
}

#[allow(clippy::too_many_arguments)]
fn trade_combo_row(
    pool: &TradePool,
    table: &SkillTable,
    room_level: u8,
    operator_capacity: usize,
    order_kind: TradeOrderKind,
    gold_lines: u32,
    layout: &Arc<LayoutContext>,
    combo: &[usize],
    signature_key: &str,
    operator_index: &HashMap<&str, usize>,
    mask_words: usize,
) -> Option<BakedComboRow> {
    let ops = build_trade_combo_operators_vec(pool, combo, None, None);
    if trade_station_exclusive_violation(&ops, table) {
        return None;
    }
    let input = TradeRoomInput {
        level: room_level,
        operators: ops,
        order_count: None,
        mood: 24.0,
        gold_production_lines: Some(gold_lines),
        durin_virtual_lines: None,
        human_fireworks: None,
        layout: Arc::clone(layout),
        active_order_kind: order_kind,
    };
    let result = solve_trade_with_shift_prevalidated(&input, table, 24.0).ok()?;
    let names: Vec<String> = input.operators.iter().map(|op| op.name.clone()).collect();
    let (operator_indices, operator_mask) =
        operator_index_and_mask(&names, operator_index, mask_words);
    Some(BakedComboRow {
        row_id: 0,
        facility: "trade".to_string(),
        signature_key: signature_key.to_string(),
        room_level,
        operator_capacity,
        names,
        operator_indices,
        operator_mask,
        sort_efficiency: result.efficiency.final_efficiency,
        order_kind: Some(order_kind),
        recipe: None,
        trade_base_efficiency: Some(result.efficiency.paper.base_efficiency),
        trade_occupancy_efficiency: Some(result.efficiency.paper.occupancy_efficiency),
        trade_skill_efficiency: Some(result.efficiency.paper.skill_efficiency),
        trade_control_efficiency: Some(result.efficiency.paper.control_efficiency),
        trade_paper_efficiency: Some(result.efficiency.paper.paper_efficiency),
        trade_mechanic_equivalent_efficiency: Some(
            result.order_mechanic.mechanic_equivalent_efficiency,
        ),
        trade_unit_output_multiplier: Some(
            result.efficiency.production_basis.unit_output_multiplier,
        ),
        trade_final_efficiency: Some(result.efficiency.final_efficiency),
        trade_equivalent_skill_efficiency: Some(result.efficiency.equivalent_skill_efficiency),
        rule_id: result.rule_id.clone(),
        unit_trade_per_day: Some(result.production.unit.unit_trade_per_day),
        unit_gold_per_day: Some(result.production.unit.unit_gold_per_day),
        unit_originium_per_day: Some(result.production.unit.unit_originium_per_day),
        manufacture_base_efficiency: None,
        manufacture_occupancy_efficiency: None,
        manufacture_skill_efficiency: None,
        manufacture_global_efficiency: None,
        manufacture_final_efficiency: None,
        manufacture_storage_limit: None,
    })
}

fn bake_manufacture_rows(
    roster: &Roster,
    instances: &OperatorInstances,
    table: &SkillTable,
    options: &BakeOptions,
    operator_index: &HashMap<&str, usize>,
    mask_words: usize,
) -> Result<BakedRows> {
    let full_pool = build_manufacture_pool(roster, instances, table)?;
    let layout = Arc::new(LayoutContext::search_baseline());
    let mut all_rows = Vec::new();
    let mut signatures = 0usize;

    for room_level in 1..=3 {
        for recipe in [
            RecipeKind::Gold,
            RecipeKind::BattleRecord,
            RecipeKind::Originium,
        ] {
            let pool = filter_pool_to_names(
                &full_pool,
                standalone_names_for(FacilityKind::Factory, StandaloneFilter::for_recipe(recipe)),
            );
            let operator_capacity = station_operator_capacity(room_level);
            let signature_key = manufacture_lookup_key(room_level, operator_capacity, recipe);
            let combo_count = n_choose_k_u64(pool.entries.len(), operator_capacity);
            let sig_start = Instant::now();
            emit_progress(
                options,
                BakeProgressEvent::SignatureStarted {
                    facility: "manufacture",
                    signature_key: signature_key.clone(),
                    combo_count,
                },
            );

            let mut rows: Vec<_> = combinations_indices(pool.entries.len(), operator_capacity)
                .collect::<Vec<_>>()
                .par_iter()
                .filter_map(|combo| {
                    manufacture_combo_row(
                        &pool,
                        table,
                        room_level,
                        operator_capacity,
                        recipe,
                        &layout,
                        combo,
                        &signature_key,
                        operator_index,
                        mask_words,
                    )
                })
                .collect();
            sort_signature_rows(&mut rows);
            if let Some(limit) = options.limit_per_signature {
                rows.truncate(limit);
            }
            emit_progress(
                options,
                BakeProgressEvent::SignatureFinished {
                    facility: "manufacture",
                    signature_key,
                    rows: rows.len(),
                    elapsed_ms: sig_start.elapsed().as_millis(),
                },
            );
            signatures += 1;
            all_rows.extend(rows);
        }
    }

    Ok(BakedRows {
        signatures,
        rows: all_rows,
    })
}

#[allow(clippy::too_many_arguments)]
fn manufacture_combo_row(
    pool: &ManuPool,
    table: &SkillTable,
    room_level: u8,
    operator_capacity: usize,
    recipe: RecipeKind,
    layout: &Arc<LayoutContext>,
    combo: &[usize],
    signature_key: &str,
    operator_index: &HashMap<&str, usize>,
    mask_words: usize,
) -> Option<BakedComboRow> {
    let operators: Vec<_> = combo
        .iter()
        .map(|idx| pool.entries[*idx].to_manu_operator())
        .collect();
    let names: Vec<String> = operators.iter().map(|op| op.name.clone()).collect();
    let input = ManuRoomInput {
        level: room_level,
        operators,
        active_recipe: recipe,
        mood: 24.0,
        layout: Arc::clone(layout),
    };
    let result = solve_manufacture(&input, table).ok()?;
    let (operator_indices, operator_mask) =
        operator_index_and_mask(&names, operator_index, mask_words);
    Some(BakedComboRow {
        row_id: 0,
        facility: "manufacture".to_string(),
        signature_key: signature_key.to_string(),
        room_level,
        operator_capacity,
        names,
        operator_indices,
        operator_mask,
        sort_efficiency: result.final_efficiency,
        order_kind: None,
        recipe: Some(recipe),
        trade_base_efficiency: None,
        trade_occupancy_efficiency: None,
        trade_skill_efficiency: None,
        trade_control_efficiency: None,
        trade_paper_efficiency: None,
        trade_mechanic_equivalent_efficiency: None,
        trade_unit_output_multiplier: None,
        trade_final_efficiency: None,
        trade_equivalent_skill_efficiency: None,
        rule_id: None,
        unit_trade_per_day: None,
        unit_gold_per_day: None,
        unit_originium_per_day: None,
        manufacture_base_efficiency: Some(result.base_efficiency),
        manufacture_occupancy_efficiency: Some(result.occupancy_efficiency),
        manufacture_skill_efficiency: Some(result.skill_efficiency),
        manufacture_global_efficiency: Some(result.global_efficiency),
        manufacture_final_efficiency: Some(result.final_efficiency),
        manufacture_storage_limit: Some(result.storage_limit),
    })
}

fn sort_signature_rows(rows: &mut [BakedComboRow]) {
    rows.sort_by(|a, b| {
        b.sort_efficiency
            .cmp(&a.sort_efficiency)
            .then_with(|| a.names.cmp(&b.names))
    });
}

pub fn validate_baked_catalog(out_dir: &Path, generator: &BakeGeneratorFingerprint) -> Result<()> {
    let manifest_path = out_dir.join("manifest.json");
    let raw = fs::read_to_string(&manifest_path)?;
    let manifest: BakeManifest = serde_json::from_str(&raw)?;
    if manifest.schema_version != BAKE_SCHEMA_VERSION {
        return Err(Error::msg(format!(
            "baked schema mismatch: found {}, expected {}; rerun `infra-cli bake --out {}`",
            manifest.schema_version,
            BAKE_SCHEMA_VERSION,
            out_dir.display()
        )));
    }
    let Some(baked_generator) = manifest.generator.as_ref() else {
        return Err(Error::msg(format!(
            "baked manifest has no generator fingerprint; rerun `infra-cli bake --out {}`",
            out_dir.display()
        )));
    };
    if baked_generator.hash64 != generator.hash64
        || baked_generator.bytes != generator.bytes
        || baked_generator.kind != generator.kind
    {
        return Err(Error::msg(format!(
            "baked generator mismatch: baked hash={} bytes={}, current hash={} bytes={}; rerun `infra-cli bake --out {}`",
            baked_generator.hash64,
            baked_generator.bytes,
            generator.hash64,
            generator.bytes,
            out_dir.display()
        )));
    }
    validate_baked_input_fingerprints(&manifest, out_dir)?;
    Ok(())
}

fn validate_baked_input_fingerprints(manifest: &BakeManifest, out_dir: &Path) -> Result<()> {
    let expected = bake_input_fingerprints()?;
    for current in expected {
        let Some(name) = Path::new(&current.path).file_name() else {
            continue;
        };
        let Some(baked) = manifest
            .inputs
            .iter()
            .find(|input| Path::new(&input.path).file_name() == Some(name))
        else {
            return Err(Error::msg(format!(
                "baked input mismatch: {} missing from manifest; rerun `infra-cli bake --out {}`",
                name.to_string_lossy(),
                out_dir.display()
            )));
        };
        if baked.bytes != current.bytes || baked.hash64 != current.hash64 {
            return Err(Error::msg(format!(
                "baked input mismatch: {} changed; rerun `infra-cli bake --out {}`",
                name.to_string_lossy(),
                out_dir.display()
            )));
        }
    }
    Ok(())
}

pub fn try_baked_trade_search(
    pool: &TradePool,
    _skill_table: &SkillTable,
    options: &TradeSearchOptions,
    order_kind: TradeOrderKind,
    filter: &SearchTripleFilter,
    combinations: u64,
    start: StdInstant,
) -> Result<Option<TradeSearchReport>> {
    if !baked_trade_compatible(pool, options, filter) {
        return Ok(None);
    }
    let Some(table) = load_runtime_baked_table()? else {
        return Ok(None);
    };
    if !baked_table_covers_pool_names(
        &table.operator_index_by_name,
        pool.entries.iter().map(|entry| entry.name.as_str()),
    ) {
        return Ok(None);
    }
    let gold_lines = if options.layout.gold_manu_line_count > 0 {
        options.layout.gold_manu_line_count
    } else {
        options.gold_production_lines
    };
    let key = trade_lookup_key(
        options.trade_level,
        options.operator_capacity,
        order_kind,
        gold_lines,
    );
    let Some((start_idx, len)) = table.index_by_key.get(&key).copied() else {
        return Ok(None);
    };
    let available_mask = available_operator_mask(
        &table.operator_index_by_name,
        pool.entries.iter().map(|entry| entry.name.as_str()),
        table.table.mask_words,
    );
    let must_names = filter.must_include_names();
    let mut hits = Vec::new();
    for row in &table.table.rows[start_idx..start_idx + len] {
        if !mask_subset(&row.operator_mask, &available_mask) {
            continue;
        }
        if must_names
            .iter()
            .any(|name| !row.names.iter().any(|row_name| row_name == name))
        {
            continue;
        }
        let hit = row_to_trade_hit(row);
        let Some(hit) = hit else { continue };
        if filter.hit_filter.is_some_and(|f| !f(&hit)) {
            continue;
        }
        hits.push(hit);
        if hits.len() >= options.top_k.max(1) {
            break;
        }
    }
    if hits.is_empty() {
        return Ok(None);
    }
    let best = hits[0].clone();
    let evaluated = hits.len() as u64;
    Ok(Some(TradeSearchReport {
        order_mode: TradeSearchOrderMode::Single(order_kind),
        best,
        top: hits,
        combinations,
        evaluated,
        elapsed: start.elapsed(),
        gold_order_line: None,
        originium_order_line: None,
    }))
}

pub fn try_baked_manufacture_search(
    pool: &ManuPool,
    _skill_table: &SkillTable,
    options: &ManuSearchOptions,
    recipe: RecipeKind,
    combinations: u64,
    start: StdInstant,
) -> Result<Option<ManuSearchReport>> {
    if !baked_manufacture_compatible(pool, options) {
        return Ok(None);
    }
    let Some(table) = load_runtime_baked_table()? else {
        return Ok(None);
    };
    if !baked_table_covers_pool_names(
        &table.operator_index_by_name,
        pool.entries.iter().map(|entry| entry.name.as_str()),
    ) {
        return Ok(None);
    }
    let key = manufacture_lookup_key(options.level, options.operator_capacity, recipe);
    let Some((start_idx, len)) = table.index_by_key.get(&key).copied() else {
        return Ok(None);
    };
    let available_mask = available_operator_mask(
        &table.operator_index_by_name,
        pool.entries.iter().map(|entry| entry.name.as_str()),
        table.table.mask_words,
    );
    let mut hits = Vec::new();
    for row in &table.table.rows[start_idx..start_idx + len] {
        if !mask_subset(&row.operator_mask, &available_mask) {
            continue;
        }
        if options
            .must_include_name
            .as_ref()
            .is_some_and(|name| !row.names.iter().any(|row_name| row_name == name))
        {
            continue;
        }
        let hit = row_to_manu_hit(row);
        let Some(hit) = hit else { continue };
        hits.push(hit);
        if hits.len() >= options.top_k.max(1) {
            break;
        }
    }
    if hits.is_empty() {
        return Ok(None);
    }
    let best = hits[0].clone();
    let evaluated = hits.len() as u64;
    Ok(Some(ManuSearchReport {
        recipe_mode: options.recipe_mode,
        best,
        top: hits,
        combinations,
        evaluated,
        elapsed: start.elapsed(),
        gold_line: None,
        battle_record_line: None,
    }))
}

pub fn warm_runtime_baked_table() -> Result<bool> {
    Ok(load_runtime_baked_table()?.is_some())
}

static RUNTIME_BAKE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub fn set_runtime_bake_in_progress(in_progress: bool) {
    RUNTIME_BAKE_IN_PROGRESS.store(in_progress, Ordering::Release);
}

fn load_runtime_baked_table() -> Result<Option<&'static RuntimeBakedComboTable>> {
    // The background baker writes several files into the catalog directory. Do
    // not cache or consume a catalog until that write set is complete.
    if RUNTIME_BAKE_IN_PROGRESS.load(Ordering::Acquire) {
        return Ok(None);
    }
    static CACHE: OnceLock<Option<RuntimeBakedComboTable>> = OnceLock::new();
    if let Some(table) = CACHE.get() {
        return Ok(table.as_ref());
    }
    match load_runtime_baked_table_inner() {
        Ok(table) => {
            let _ = CACHE.set(Some(table));
            Ok(CACHE.get().and_then(|t| t.as_ref()))
        }
        Err(err) => {
            let msg = err.to_string();
            if msg.contains("data/baked/manifest.json not found")
                || msg.contains("file not found")
                || msg.contains("No such file or directory")
                || msg.contains("baked schema mismatch")
                || msg.contains("baked generator mismatch")
                || msg.contains("baked input mismatch")
            {
                Ok(None)
            } else {
                Err(err)
            }
        }
    }
}

fn load_runtime_baked_table_inner() -> Result<RuntimeBakedComboTable> {
    let Ok(manifest_path) = data_path("baked/manifest.json") else {
        return Err(Error::msg("data/baked/manifest.json not found"));
    };
    let out_dir = manifest_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("data/baked"));
    let generator = current_exe_generator_fingerprint()?;
    validate_baked_catalog(&out_dir, &generator)?;
    let combo_path = out_dir.join("combo_table.bin");
    let raw = fs::read(&combo_path)?;
    let disk: BakedComboTableDisk = bincode::deserialize(&raw)
        .map_err(|e| Error::msg(format!("read {}: {e}", combo_path.display())))?;
    let mut table = BakedComboTable::from(disk);
    let mut index_by_key = HashMap::new();
    for index in &table.indexes {
        index_by_key.insert(index.signature_key.clone(), (index.start, index.len));
        for row in &mut table.rows[index.start..index.start + index.len] {
            row.signature_key = index.signature_key.clone();
            row.facility = row_facility_from_signature(&index.signature_key).to_string();
        }
    }
    for (idx, row) in table.rows.iter_mut().enumerate() {
        row.row_id = idx;
        if row.names.is_empty() && !table.operator_names.is_empty() {
            row.names = row
                .operator_indices
                .iter()
                .filter_map(|idx| table.operator_names.get(*idx).cloned())
                .collect();
        }
        if row.operator_mask.is_empty() {
            row.operator_mask = operator_mask_from_indices(&row.operator_indices, table.mask_words);
        }
    }
    let operator_index_by_name = table
        .operator_names
        .iter()
        .enumerate()
        .map(|(idx, name)| (name.clone(), idx))
        .collect();
    Ok(RuntimeBakedComboTable {
        table,
        index_by_key,
        operator_index_by_name,
    })
}

fn trade_lookup_key(
    room_level: u8,
    operator_capacity: usize,
    order_kind: TradeOrderKind,
    gold_lines: u32,
) -> String {
    match order_kind {
        TradeOrderKind::Gold => format!(
            "trade:level{}:cap{}:order_{:?}:gold_lines{}",
            room_level, operator_capacity, order_kind, gold_lines
        ),
        TradeOrderKind::Originium => format!(
            "trade:level{}:cap{}:order_{:?}",
            room_level, operator_capacity, order_kind
        ),
    }
    .to_ascii_lowercase()
}

fn manufacture_lookup_key(room_level: u8, operator_capacity: usize, recipe: RecipeKind) -> String {
    format!(
        "manufacture:level{}:cap{}:recipe_{:?}",
        room_level, operator_capacity, recipe
    )
    .to_ascii_lowercase()
}

fn current_exe_generator_fingerprint() -> Result<BakeGeneratorFingerprint> {
    let path = std::env::current_exe()?;
    let bytes = fs::read(&path)?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    Ok(BakeGeneratorFingerprint {
        kind: "infra-cli-exe".to_string(),
        path: path.to_string_lossy().to_string(),
        bytes: bytes.len() as u64,
        hash64: format!("{:016x}", hasher.finish()),
    })
}

fn baked_trade_compatible(
    pool: &TradePool,
    options: &TradeSearchOptions,
    filter: &SearchTripleFilter,
) -> bool {
    filter.must_operator_override.is_none()
        && (options.mood - 24.0).abs() < f64::EPSILON
        && (options.shift_hours - 24.0).abs() < f64::EPSILON
        && baked_layout_search_compatible(&options.layout)
        && pool.entries.iter().all(|entry| entry.progress.elite >= 2)
}

fn baked_manufacture_compatible(pool: &ManuPool, options: &ManuSearchOptions) -> bool {
    !options.full_pool
        && (options.mood - 24.0).abs() < f64::EPSILON
        && baked_layout_search_compatible(&options.layout)
        && pool.entries.iter().all(|entry| entry.progress.elite >= 2)
}

fn baked_layout_search_compatible(layout: &LayoutContext) -> bool {
    let _ = layout;
    true
}

fn baked_table_covers_pool_names<'a>(
    operator_index_by_name: &HashMap<String, usize>,
    mut names: impl Iterator<Item = &'a str>,
) -> bool {
    names.all(|name| operator_index_by_name.contains_key(name))
}

fn available_operator_mask<'a>(
    operator_index_by_name: &HashMap<String, usize>,
    names: impl Iterator<Item = &'a str>,
    mask_words: usize,
) -> Vec<u64> {
    let mut mask = vec![0u64; mask_words];
    for name in names {
        let Some(idx) = operator_index_by_name.get(name).copied() else {
            continue;
        };
        let word = idx / 64;
        let bit = idx % 64;
        if let Some(slot) = mask.get_mut(word) {
            *slot |= 1u64 << bit;
        }
    }
    mask
}

fn mask_subset(row_mask: &[u64], available_mask: &[u64]) -> bool {
    row_mask.iter().enumerate().all(|(idx, word)| {
        let available = available_mask.get(idx).copied().unwrap_or(0);
        word & !available == 0
    })
}

fn row_to_trade_hit(row: &BakedComboRow) -> Option<TradeSearchHit> {
    if row.facility != "trade" {
        return None;
    }
    let final_efficiency = row.trade_final_efficiency?;
    let mechanic_equivalent_efficiency = row.trade_mechanic_equivalent_efficiency?;
    let unit_trade_per_day = row.unit_trade_per_day.unwrap_or(0.0);
    let unit_gold_per_day = row.unit_gold_per_day.unwrap_or(0.0);
    Some(TradeSearchHit {
        names: row.names.clone(),
        gold_names: vec![],
        originium_names: vec![],
        final_efficiency,
        mechanic_equivalent_efficiency,
        rule_id: row.rule_id.clone(),
        unit_trade_per_day,
        unit_gold_per_day,
        unit_originium_per_day: row.unit_originium_per_day.unwrap_or(0.0),
        breakdown: Some(TradeEfficiencyBreakdown {
            base_efficiency: row.trade_base_efficiency?,
            occupancy_efficiency: row.trade_occupancy_efficiency?,
            skill_efficiency: row.trade_skill_efficiency?,
            control_efficiency: row.trade_control_efficiency?,
            paper_efficiency: row.trade_paper_efficiency?,
            mechanic_equivalent_efficiency,
            unit_output_multiplier: row.trade_unit_output_multiplier?,
            final_efficiency,
            equivalent_skill_efficiency: row.trade_equivalent_skill_efficiency?,
            unit_trade_per_day,
            unit_gold_per_day,
            rule_id: row.rule_id.clone(),
        }),
    })
}

fn row_to_manu_hit(row: &BakedComboRow) -> Option<ManuSearchHit> {
    if row.facility != "manufacture" {
        return None;
    }
    let recipe = row.recipe?;
    let final_efficiency = row.manufacture_final_efficiency?;
    let storage_limit = row.manufacture_storage_limit.unwrap_or(0);
    let mut per_station = crate::manufacture::ManuProdBreakdown::default();
    let mut storage = crate::manufacture::ManuStorageBreakdown::default();
    let recipe_label = match recipe {
        RecipeKind::Gold => {
            per_station.gold = final_efficiency;
            storage.gold = storage_limit;
            "gold"
        }
        RecipeKind::BattleRecord => {
            per_station.battle_record = final_efficiency;
            storage.battle_record = storage_limit;
            "battle_record"
        }
        RecipeKind::Originium => {
            per_station.originium = final_efficiency;
            storage.originium = storage_limit;
            "originium"
        }
        RecipeKind::All => "all",
    };
    Some(ManuSearchHit {
        names: row.names.clone(),
        gold_names: vec![],
        battle_record_names: vec![],
        final_efficiency,
        per_station,
        storage,
        breakdown: ManuEfficiencyBreakdown {
            base_efficiency: row.manufacture_base_efficiency?,
            occupancy_efficiency: row.manufacture_occupancy_efficiency?,
            skill_efficiency: row.manufacture_skill_efficiency?,
            global_efficiency: row.manufacture_global_efficiency?,
            final_efficiency,
            storage_limit,
            recipe: recipe_label.to_string(),
        },
    })
}

fn build_combo_table_from_rows(
    operator_names: Vec<String>,
    mask_words: usize,
    mut rows: Vec<BakedComboRow>,
    generator: Option<BakeGeneratorFingerprint>,
) -> BakedComboTable {
    rows.sort_by(|a, b| {
        a.facility
            .cmp(&b.facility)
            .then_with(|| a.signature_key.cmp(&b.signature_key))
            .then_with(|| b.sort_efficiency.cmp(&a.sort_efficiency))
            .then_with(|| a.names.cmp(&b.names))
    });
    for (idx, row) in rows.iter_mut().enumerate() {
        row.row_id = idx;
    }

    let indexes = build_combo_indexes(&rows);
    BakedComboTable {
        schema_version: BAKE_SCHEMA_VERSION,
        generator,
        operator_count: operator_names.len(),
        operator_names,
        mask_words,
        indexes,
        rows,
    }
}

fn build_combo_indexes(rows: &[BakedComboRow]) -> Vec<BakedComboIndex> {
    let mut indexes = Vec::new();
    let mut start = 0usize;
    while start < rows.len() {
        let key = rows[start].signature_key.clone();
        let mut end = start + 1;
        while end < rows.len() && rows[end].signature_key == key {
            end += 1;
        }
        indexes.push(BakedComboIndex {
            signature_key: key,
            start,
            len: end - start,
        });
        start = end;
    }
    indexes
}

fn operator_index_and_mask(
    names: &[String],
    operator_index: &HashMap<&str, usize>,
    mask_words: usize,
) -> (Vec<usize>, Vec<u64>) {
    let mut indices: Vec<_> = names
        .iter()
        .filter_map(|name| operator_index.get(name.as_str()).copied())
        .collect();
    indices.sort_unstable();

    let mut mask = vec![0u64; mask_words];
    for idx in &indices {
        let word = idx / 64;
        let bit = idx % 64;
        if let Some(slot) = mask.get_mut(word) {
            *slot |= 1u64 << bit;
        }
    }
    (indices, mask)
}

fn operator_mask_from_indices(indices: &[usize], mask_words: usize) -> Vec<u64> {
    let mut mask = vec![0u64; mask_words];
    for idx in indices {
        let word = idx / 64;
        let bit = idx % 64;
        if let Some(slot) = mask.get_mut(word) {
            *slot |= 1u64 << bit;
        }
    }
    mask
}

fn row_facility_from_signature(signature_key: &str) -> &'static str {
    if signature_key.starts_with("trade:") {
        "trade"
    } else if signature_key.starts_with("manufacture:") {
        "manufacture"
    } else {
        ""
    }
}

fn bake_roster(instances: &OperatorInstances) -> Roster {
    let mut roster = Roster::default();
    for name in bake_operator_names(instances) {
        let has_production_binding = instances
            .get(&name, crate::tier::PromotionTier::TierUp)
            .is_some_and(|i| {
                i.facilities.contains_key("trade") || i.facilities.contains_key("manufacture")
            })
            || instances
                .get(&name, crate::tier::PromotionTier::Tier0)
                .is_some_and(|i| {
                    i.facilities.contains_key("trade") || i.facilities.contains_key("manufacture")
                });
        if !has_production_binding {
            continue;
        }
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

fn bake_operator_names(instances: &OperatorInstances) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for (_, instance) in instances.iter() {
        if instance.facilities.contains_key("trade") {
            names.insert(instance.name.clone());
        }
    }
    for recipe in [
        RecipeKind::Gold,
        RecipeKind::BattleRecord,
        RecipeKind::Originium,
    ] {
        if let Some(set) =
            standalone_names_for(FacilityKind::Factory, StandaloneFilter::for_recipe(recipe))
        {
            names.extend(set);
        }
    }
    names
}

fn bake_input_fingerprints() -> Result<Vec<BakeInputFingerprint>> {
    let mut paths = BTreeMap::new();
    for name in [
        "operator_instances.json",
        "skill_table.json",
        "standalone_roster.json",
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
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, value)
        .map_err(|e| Error::msg(format!("write {}: {e}", path.display())))
}

fn write_binary(path: PathBuf, value: &impl Serialize) -> Result<()> {
    let bytes = bincode::serialize(value)
        .map_err(|e| Error::msg(format!("encode {}: {e}", path.display())))?;
    fs::write(&path, bytes).map_err(|e| Error::msg(format!("write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baked_table_must_cover_every_candidate_name() {
        let operator_index_by_name = HashMap::from([
            ("Miss.Christine".to_string(), 0usize),
            ("酒神".to_string(), 1usize),
            ("弑君者".to_string(), 2usize),
        ]);

        assert!(baked_table_covers_pool_names(
            &operator_index_by_name,
            ["Miss.Christine", "酒神"].into_iter()
        ));
        assert!(
            !baked_table_covers_pool_names(
                &operator_index_by_name,
                ["Miss.Christine", "酒神", "红云"].into_iter()
            ),
            "baked search must fall back when a stale catalog omits a live candidate"
        );
    }

    #[test]
    fn assignment_full_manufacture_pool_structurally_rejects_standalone_bake() {
        let pool = ManuPool {
            entries: vec![],
            skipped: vec![],
        };
        assert!(baked_manufacture_compatible(
            &pool,
            &ManuSearchOptions::default()
        ));
        assert!(!baked_manufacture_compatible(
            &pool,
            &ManuSearchOptions {
                full_pool: true,
                ..Default::default()
            }
        ));
    }
}
