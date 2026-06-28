use std::path::PathBuf;
use std::{env, fs, hash::Hash, hash::Hasher, sync::Arc};

use infra_core::{
    bake_catalogs, default_baked_out_dir, validate_baked_catalog, BakeGeneratorFingerprint,
    BakeOptions, BakeProgressEvent, Error,
};

pub fn bake_cmd(args: &[String]) -> Result<(), Error> {
    let mut options = BakeOptions::default();
    options.out_dir = default_baked_out_dir()?;
    let mut mode = "all";
    let mut validate_only = false;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "all" | "trade" | "manufacture" => {
                mode = args[i].as_str();
                i += 1;
            }
            "validate" => {
                validate_only = true;
                i += 1;
            }
            "--out" => {
                let Some(path) = args.get(i + 1) else {
                    return Err(Error::msg("bake --out requires a path"));
                };
                options.out_dir = PathBuf::from(path);
                i += 2;
            }
            "--limit-per-signature" => {
                let Some(raw) = args.get(i + 1) else {
                    return Err(Error::msg(
                        "bake --limit-per-signature requires a positive integer",
                    ));
                };
                options.limit_per_signature = Some(
                    raw.parse()
                        .map_err(|_| Error::msg("invalid --limit-per-signature value"))?,
                );
                i += 2;
            }
            "--help" | "-h" => {
                print_bake_usage();
                return Ok(());
            }
            other => {
                return Err(Error::msg(format!("unknown bake argument {other:?}")));
            }
        }
    }

    let generator = current_generator_fingerprint()?;
    if validate_only {
        validate_baked_catalog(&options.out_dir, &generator)?;
        eprintln!(
            "baked catalog is valid for current cli hash={} -> {}",
            generator.hash64,
            options.out_dir.display()
        );
        return Ok(());
    }
    options.generator = Some(generator);
    options.progress = Some(Arc::new(print_bake_progress));

    match mode {
        "all" => {
            options.include_trade = true;
            options.include_manufacture = true;
        }
        "trade" => {
            options.include_trade = true;
            options.include_manufacture = false;
        }
        "manufacture" => {
            options.include_trade = false;
            options.include_manufacture = true;
        }
        _ => unreachable!(),
    }

    let report = bake_catalogs(&options)?;
    eprintln!(
        "baked operators={} trade_signatures={} trade_hits={} manufacture_signatures={} manufacture_hits={} elapsed={}ms -> {}",
        report.operator_count,
        report.trade_signatures,
        report.trade_hits,
        report.manufacture_signatures,
        report.manufacture_hits,
        report.elapsed_ms,
        report.out_dir.display()
    );
    Ok(())
}

fn print_bake_usage() {
    eprintln!("Usage:");
    eprintln!("  infra-cli bake [all|trade|manufacture] [--out <dir>] [--limit-per-signature <n>]");
    eprintln!("  infra-cli bake validate [--out <dir>]");
    eprintln!("      Generates indexed 3/2/1-person single-room combo_table.json by default.");
}

fn print_bake_progress(event: BakeProgressEvent) {
    match event {
        BakeProgressEvent::Started {
            out_dir,
            operator_count,
            signature_count,
            worker_count,
        } => {
            eprintln!(
                "[bake] start operators={} signatures={} rayon_workers={} -> {}",
                operator_count,
                signature_count,
                worker_count,
                out_dir.display()
            );
        }
        BakeProgressEvent::SignatureStarted {
            facility,
            signature_key,
            combo_count,
        } => {
            eprintln!("[bake] {facility} {signature_key}: enumerating {combo_count} combos");
        }
        BakeProgressEvent::SignatureFinished {
            facility,
            signature_key,
            rows,
            elapsed_ms,
        } => {
            eprintln!("[bake] {facility} {signature_key}: rows={rows} elapsed={elapsed_ms}ms");
        }
        BakeProgressEvent::Writing { path, rows } => {
            if let Some(rows) = rows {
                eprintln!("[bake] write {} rows={rows}", path.display());
            } else {
                eprintln!("[bake] write {}", path.display());
            }
        }
        BakeProgressEvent::Finished {
            combo_table_rows,
            elapsed_ms,
        } => {
            eprintln!("[bake] done combo_table_rows={combo_table_rows} elapsed={elapsed_ms}ms");
        }
    }
}

fn current_generator_fingerprint() -> Result<BakeGeneratorFingerprint, Error> {
    let path = env::current_exe()?;
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
