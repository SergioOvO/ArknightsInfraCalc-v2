use std::path::PathBuf;

use infra_core::{bake_catalogs, BakeOptions, Error};

pub fn bake_cmd(args: &[String]) -> Result<(), Error> {
    let mut options = BakeOptions::default();
    let mut mode = "all";

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "all" | "trade" | "manufacture" => {
                mode = args[i].as_str();
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
    eprintln!("      Generates full baked 3/2/1-person single-room candidate tables by default.");
}
