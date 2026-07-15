#!/usr/bin/env python3
"""Check changed paths against docs-impact declarations and deterministic docs facts."""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any
from urllib.parse import unquote


VAGUE_REASONS = {"", "none", "n/a", "na", "no impact", "无影响", "不需要", "not needed"}


class CheckError(ValueError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise CheckError(f"cannot read manifest {path}: {error}") from error
    if not isinstance(value, dict):
        raise CheckError("manifest must be a JSON object")
    return value


def load_config(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as handle:
            value = tomllib.load(handle)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise CheckError(f"cannot read config {path}: {error}") from error
    if value.get("schema_version") != 1 or not isinstance(value.get("rules"), list):
        raise CheckError("docs impact config must have schema_version=1 and [[rules]]")
    return value


def _git_paths(repo: Path, arguments: list[str]) -> set[str]:
    result = subprocess.run(
        ["git", "-C", str(repo), *arguments],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return {line for line in result.stdout.splitlines() if line}


def discover_changed_paths(repo: Path, base_sha: str) -> set[str]:
    paths: set[str] = set()
    if base_sha:
        paths |= _git_paths(repo, ["diff", "--name-only", "--diff-filter=ACMRD", f"{base_sha}..HEAD"])
    paths |= _git_paths(repo, ["diff", "--name-only", "--diff-filter=ACMRD"])
    paths |= _git_paths(repo, ["diff", "--cached", "--name-only", "--diff-filter=ACMRD"])
    paths |= _git_paths(repo, ["ls-files", "--others", "--exclude-standard"])
    return paths


def matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def _specific_reason(reason: object) -> bool:
    if not isinstance(reason, str):
        return False
    normalized = reason.strip().lower()
    return len(normalized) >= 12 and normalized not in VAGUE_REASONS


def check_markdown_links(repo: Path, documents: list[str]) -> list[str]:
    errors: list[str] = []
    link_re = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
    for document in documents:
        path = repo / document
        if path.suffix.lower() != ".md" or not path.is_file():
            continue
        text = path.read_text(encoding="utf-8")
        for raw_target in link_re.findall(text):
            target = raw_target.strip()
            if target.startswith("<") and ">" in target:
                target = target[1 : target.index(">")]
            else:
                target = target.split(" ", 1)[0]
            if not target or target.startswith(("#", "http://", "https://", "mailto:")):
                continue
            local = unquote(target.split("#", 1)[0])
            resolved = (path.parent / local).resolve(strict=False)
            if not resolved.exists():
                errors.append(f"broken Markdown link in {document}: {raw_target}")
    return errors


def check_doc_status(repo: Path, documents: list[str]) -> list[str]:
    errors = []
    for document in documents:
        path = repo / document
        if path.suffix.lower() != ".md" or not path.is_file():
            continue
        header = "\n".join(path.read_text(encoding="utf-8").splitlines()[:15])
        if "状态" not in header:
            errors.append(f"updated document lacks a status field near the top: {document}")
    return errors


def check_cli_help_map(repo: Path) -> list[str]:
    errors: list[str] = []
    main_path = repo / "crates/infra-cli/src/main.rs"
    layout_path = repo / "crates/infra-cli/src/commands/layout.rs"
    map_path = repo / "docs/PROJECT_MAP.md"
    for path in (main_path, layout_path, map_path):
        if not path.is_file():
            return [f"cli-help-map input is missing: {path}"]

    main_text = main_path.read_text(encoding="utf-8")
    layout_text = layout_path.read_text(encoding="utf-8")
    map_text = map_path.read_text(encoding="utf-8")
    match_start = main_text.find("match args[1].as_str()")
    match_end = main_text.find("other =>", match_start)
    if match_start < 0 or match_end < 0:
        return ["cannot parse top-level CLI dispatch in main.rs"]
    source_commands = set(re.findall(r'^\s*"([a-z][a-z-]*)"\s*=>', main_text[match_start:match_end], re.MULTILINE))

    section_start = map_text.find("## `infra-cli` 命令")
    if section_start < 0:
        return ["cannot find infra-cli command section in PROJECT_MAP.md"]
    next_heading = re.search(r"\n#{2,3} ", map_text[section_start + 1 :])
    section_end = section_start + 1 + next_heading.start() if next_heading else len(map_text)
    section = map_text[section_start:section_end]
    doc_commands: set[str] = set()
    doc_layout_commands: set[str] = set()
    for line in section.splitlines():
        if not line.startswith("|"):
            continue
        in_code = False
        column_end = None
        for index, character in enumerate(line[1:], start=1):
            if character == "`":
                in_code = not in_code
            elif character == "|" and not in_code:
                column_end = index
                break
        if column_end is None:
            continue
        first_column = line[1:column_end]
        for value in re.findall(r"`([^`]+)`", first_column):
            words = value.split()
            if not words:
                continue
            if words[0].startswith("-"):
                continue
            doc_commands.add(words[0])
            if words[0] == "layout" and len(words) > 1:
                doc_layout_commands.add(words[1])

    source_layout_commands = set(re.findall(r'Some\("([a-z][a-z-]*)"\)\s*=>', layout_text))
    usage_commands = set(re.findall(r"infra-cli ([a-z][a-z-]*)", main_text))
    usage_layout_commands = set(re.findall(r"infra-cli layout ([a-z][a-z-]*)", main_text))
    if source_commands != doc_commands:
        errors.append(
            f"top-level CLI map mismatch: source_only={sorted(source_commands - doc_commands)} "
            f"docs_only={sorted(doc_commands - source_commands)}"
        )
    missing_usage = source_commands - usage_commands
    if missing_usage:
        errors.append(f"top-level usage is missing commands: {sorted(missing_usage)}")
    if source_layout_commands != doc_layout_commands:
        errors.append(
            f"layout CLI map mismatch: source_only={sorted(source_layout_commands - doc_layout_commands)} "
            f"docs_only={sorted(doc_layout_commands - source_layout_commands)}"
        )
    missing_layout_usage = source_layout_commands - usage_layout_commands
    if missing_layout_usage:
        errors.append(f"top-level usage is missing layout subcommands: {sorted(missing_layout_usage)}")
    return errors


def _declared_routes(value: object) -> dict[str, str]:
    routes: dict[str, str] = {}
    if not isinstance(value, list):
        return routes
    for item in value:
        if isinstance(item, dict) and isinstance(item.get("rule"), str) and isinstance(item.get("note"), str):
            routes[item["rule"]] = item["note"].strip()
    return routes


def run_checks(
    repo: Path,
    config: dict[str, Any],
    manifest: dict[str, Any],
    changed_paths: set[str],
    extra_generated: list[str],
) -> list[str]:
    errors: list[str] = []
    impact = manifest.get("docs_impact")
    if not isinstance(impact, dict):
        return ["manifest.docs_impact must be an object"]
    status = impact.get("status")
    checked = impact.get("checked")
    updated = impact.get("updated")
    reason = impact.get("reason")
    if status not in {"updated", "not-needed", "blocked"}:
        errors.append("docs_impact.status must be updated, not-needed, or blocked")
    if not isinstance(checked, list) or not all(isinstance(item, str) for item in checked):
        errors.append("docs_impact.checked must be an array of paths")
        checked = []
    if not isinstance(updated, list) or not all(isinstance(item, str) for item in updated):
        errors.append("docs_impact.updated must be an array of paths")
        updated = []
    if not _specific_reason(reason):
        errors.append("docs_impact.reason must be specific and at least 12 characters")
    if status == "blocked":
        errors.append("docs_impact is blocked; completion is not allowed")
    if status == "updated" and not updated:
        errors.append("docs_impact.status=updated requires at least one updated document")
    if status == "not-needed" and updated:
        errors.append("docs_impact.status=not-needed requires an empty updated list")

    rules = config["rules"]
    matched_rules: dict[str, dict[str, Any]] = {}
    ignore_globs = config.get("ignore_globs", [])
    uncovered: list[str] = []
    for path in sorted(changed_paths):
        matched = [rule for rule in rules if matches(path, rule.get("globs", []))]
        if not matched and not matches(path, ignore_globs):
            uncovered.append(path)
        for rule in matched:
            matched_rules[str(rule["id"])] = rule
    if uncovered:
        errors.append(f"changed paths are not covered by docs_impact.toml: {uncovered}")

    required = {
        document
        for rule in matched_rules.values()
        for document in rule.get("required_check", [])
    }
    missing_checked = required - set(checked)
    if missing_checked:
        errors.append(f"docs_impact.checked is missing required documents: {sorted(missing_checked)}")

    routes = _declared_routes(impact.get("routes"))
    for rule_id, rule in matched_rules.items():
        if rule.get("domain_route") and not routes.get(rule_id):
            errors.append(
                f"docs_impact.routes requires a non-empty note for {rule_id}: {rule['domain_route']}"
            )

    for document in checked:
        if not (repo / document).is_file():
            errors.append(f"checked document does not exist: {document}")
    for document in updated:
        if document not in checked:
            errors.append(f"updated document was not listed in checked: {document}")
        if document not in changed_paths:
            errors.append(f"docs_impact falsely claims an unchanged document was updated: {document}")

    generated = set(extra_generated)
    for rule in matched_rules.values():
        generated.update(rule.get("generated_check", []))
    if updated:
        generated.add("markdown-links")
    for check in sorted(generated):
        if check == "markdown-links":
            errors.extend(check_markdown_links(repo, updated))
        elif check == "doc-status":
            errors.extend(check_doc_status(repo, updated))
        elif check == "cli-help-map":
            errors.extend(check_cli_help_map(repo))
        else:
            errors.append(f"unknown generated check: {check}")
    return errors


def build_parser() -> argparse.ArgumentParser:
    script_dir = Path(__file__).resolve().parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--config", type=Path, default=script_dir / "docs_impact.toml")
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--base-sha")
    parser.add_argument("--changed-path", action="append", default=[])
    parser.add_argument("--generated-check", action="append", default=[])
    return parser


def main() -> int:
    args = build_parser().parse_args()
    repo = args.repo_root.resolve()
    try:
        manifest = load_json(args.manifest)
        config = load_config(args.config)
        if args.changed_path:
            changed_paths = set(args.changed_path)
        else:
            base_sha = args.base_sha
            if base_sha is None:
                base_sha = str(manifest.get("task", {}).get("base_sha", ""))
            changed_paths = discover_changed_paths(repo, base_sha)
        errors = run_checks(repo, config, manifest, changed_paths, args.generated_check)
    except (CheckError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"PASS docs_impact changed_paths={len(changed_paths)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
