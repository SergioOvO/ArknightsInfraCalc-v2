use std::path::PathBuf;

use infra_core::instances::{default_instances_path, OperatorInstances};
use infra_core::operbox::OperBox;
use infra_core::training_advice::{
    build_training_advice, default_training_recommendations_path,
    load_training_recommendation_rules, TrainingAdviceOptions,
};
use infra_core::Error;

pub fn advice_cmd(args: &[String]) -> Result<(), Error> {
    let operbox_path = required_path(args, "--operbox")?;
    let rules_path = optional_path(args, "--rules")
        .map(Ok)
        .unwrap_or_else(default_training_recommendations_path)?;
    let pretty = args.iter().any(|a| a == "--pretty");

    let operbox = OperBox::load(&operbox_path)?;
    let instances = OperatorInstances::load(&default_instances_path()?)?;
    let rules = load_training_recommendation_rules(&rules_path)?;
    let report = build_training_advice(
        &operbox,
        &instances,
        &rules,
        &TrainingAdviceOptions {
            operbox_label: Some(operbox_path.to_string_lossy().into_owned()),
        },
    )?;

    let json = if pretty {
        serde_json::to_string_pretty(&report)?
    } else {
        serde_json::to_string(&report)?
    };
    println!("{json}");
    Ok(())
}

fn required_path(args: &[String], flag: &str) -> Result<PathBuf, Error> {
    optional_path(args, flag).ok_or_else(|| Error::msg(format!("missing required {flag} <path>")))
}

fn optional_path(args: &[String], flag: &str) -> Option<PathBuf> {
    args.windows(2)
        .find(|w| w[0] == flag)
        .map(|w| PathBuf::from(&w[1]))
}
