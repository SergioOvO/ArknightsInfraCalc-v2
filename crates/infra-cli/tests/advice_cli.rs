use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

fn custom_rules(evidence_path: &str, heading: Option<&str>) -> String {
    let heading = heading
        .map(|value| format!(r#", "heading": {value:?}"#))
        .unwrap_or_default();
    format!(
        r#"{{
  "version": 2,
  "acquisition_policy": {{"default_rarity_le": 4, "named_exceptions": []}},
  "rules": [{{
    "id": "custom_tequila",
    "kind": "standalone",
    "scope": "independent",
    "label": "龙舌兰测试规则",
    "admission": {{"required_core": [], "pick_one_core": []}},
    "members": [{{
      "operator": "龙舌兰",
      "role": "independent",
      "target": {{"elite": 1}},
      "priority": "P0",
      "acquisition": "owned_only",
      "rarity": 5
    }}],
    "evidence": [{{"path": {evidence_path:?}{heading}}}],
    "review": {{"status": "confirmed", "conflicts": []}}
  }}]
}}"#
    )
}

fn run_explain(rules_path: &Path) -> serde_json::Value {
    let root = workspace_root();
    let output = Command::new(env!("CARGO_BIN_EXE_infra-cli"))
        .current_dir(&root)
        .args([
            "advice",
            "--operbox",
            "data/fixtures/training_advice/witch_only_tequila.json",
            "--rules",
        ])
        .arg(rules_path)
        .arg("--explain")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn custom_rules_explain_reads_workspace_markdown_but_rejects_absolute_sources() {
    let directory = std::env::temp_dir().join(format!(
        "infra-cli-advice-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    fs::create_dir_all(&directory).unwrap();

    let valid_rules = directory.join("valid.json");
    fs::write(
        &valid_rules,
        custom_rules("docs/练卡推荐规则.md", Some("RAG 解释层")),
    )
    .unwrap();
    let valid = run_explain(&valid_rules);
    assert_eq!(valid["report"]["now"][0]["operator"], "龙舌兰");
    assert_eq!(
        valid["rag_input"]["evidence_snippets"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(valid["rag_input"]["unavailable_source_refs"]
        .as_array()
        .unwrap()
        .is_empty());

    let absolute_rules = directory.join("absolute.json");
    fs::write(&absolute_rules, custom_rules("/etc/passwd.md", None)).unwrap();
    let absolute = run_explain(&absolute_rules);
    assert!(absolute["rag_input"]["evidence_snippets"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        absolute["rag_input"]["unavailable_source_refs"][0]["path"],
        "/etc/passwd.md"
    );

    fs::remove_dir_all(directory).unwrap();
}
