#!/usr/bin/env python3
"""Check changed paths against file-owned documentation review triggers."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any
from urllib.parse import unquote

import docs_inventory


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
    if value.get("schema_version") != 2 or not isinstance(value.get("ignore_globs"), list):
        raise CheckError("docs impact config must have schema_version=2 and ignore_globs")
    return value


def _git_paths(repo: Path, arguments: list[str]) -> set[str]:
    result = subprocess.run(
        ["git", "-c", "core.quotepath=false", "-C", str(repo), *arguments],
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


def discover_committed_paths(repo: Path, base_sha: str) -> set[str]:
    if not base_sha:
        raise CheckError("--committed-only requires --base-sha")
    return _git_paths(repo, ["diff", "--name-only", "--diff-filter=ACMRD", f"{base_sha}..HEAD"])


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
    errors: list[str] = []
    for document in documents:
        path = repo / document
        if path.suffix.lower() != ".md" or not path.is_file():
            continue
        try:
            docs_inventory.parse_document(repo, path)
        except docs_inventory.InventoryError as error:
            errors.append(str(error))
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
        for value in re.findall(r"`([^`]+)`", line[1:column_end]):
            words = value.split()
            if not words or words[0].startswith("-"):
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


def _entry_map(value: object, errors: list[str]) -> dict[str, dict[str, Any]]:
    if not isinstance(value, list):
        errors.append("docs_impact.entries must be an array")
        return {}
    entries: dict[str, dict[str, Any]] = {}
    for index, item in enumerate(value):
        if not isinstance(item, dict):
            errors.append(f"docs_impact.entries[{index}] must be an object")
            continue
        path = item.get("path")
        if not isinstance(path, str) or not path:
            errors.append(f"docs_impact.entries[{index}].path is required")
            continue
        if path in entries:
            errors.append(f"duplicate docs impact entry: {path}")
        entries[path] = item
        if item.get("disposition") not in {"updated", "unchanged"}:
            errors.append(f"invalid docs impact disposition: {path}")
        for field in ("source_digest", "document_digest", "cause"):
            if not isinstance(item.get(field), str) or not item.get(field):
                errors.append(f"docs impact {field} is required: {path}")
        for field in ("stable_facts", "evidence"):
            values = item.get(field)
            if not isinstance(values, list) or not values or not all(isinstance(value, str) and value for value in values):
                errors.append(f"docs impact {field} must be a non-empty string array: {path}")
    return entries


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
    if status not in {"updated", "not-needed", "blocked"}:
        errors.append("docs_impact.status must be updated, not-needed, or blocked")
    if status == "blocked":
        errors.append("docs_impact is blocked; completion is not allowed")
    if not _specific_reason(impact.get("reason")):
        errors.append("docs_impact.reason must be specific and at least 12 characters")
    entries = _entry_map(impact.get("entries"), errors)
    if status == "updated" and not entries:
        errors.append("docs_impact.status=updated requires entries")
    if status == "not-needed" and entries:
        errors.append("docs_impact.status=not-needed requires no entries")

    documents, inventory_errors = docs_inventory.load_inventory(repo)
    errors.extend(inventory_errors)
    by_path = {document.path: document for document in documents}
    ignore_globs = [str(value) for value in config.get("ignore_globs", [])]
    errors.extend(docs_inventory.source_coverage_errors(changed_paths, documents, ignore_globs))

    triggered = {
        document.path
        for document in documents
        if any(
            docs_inventory.path_matches_pattern(path, trigger)
            for path in changed_paths
            if not path.endswith(".md")
            for trigger in docs_inventory.dependency_patterns(document)
        )
    }
    changed_reviewable = {
        document.path
        for document in documents
        if docs_inventory.is_reviewable(document) and document.path in changed_paths
    }
    expected_entries = triggered | changed_reviewable
    missing = expected_entries - set(entries)
    if missing:
        errors.append(f"docs impact entries missing changed or triggered documents: {sorted(missing)}")
    unexpected = set(entries) - expected_entries
    if unexpected:
        errors.append(f"docs impact entries contain documents outside the exact review set: {sorted(unexpected)}")

    for path, entry in entries.items():
        document = by_path.get(path)
        if document is None:
            errors.append(f"docs impact entry is not a governed current document: {path}")
            continue
        if path not in changed_paths:
            errors.append(f"docs review record was not updated in this change: {path}")
        expected_facts = docs_inventory.split_values(document.metadata.get("稳定事实", ""))
        expected_evidence = docs_inventory.split_values(document.metadata.get("证据引用", ""))
        if entry.get("disposition") != document.metadata.get("复核结论"):
            errors.append(f"docs impact disposition disagrees with file review record: {path}")
        if entry.get("source_digest") != document.metadata.get("源摘要"):
            errors.append(f"docs impact source digest disagrees with file review record: {path}")
        if entry.get("document_digest") != document.metadata.get("文档摘要"):
            errors.append(f"docs impact document digest disagrees with file review record: {path}")
        if entry.get("cause") != document.metadata.get("复核原因"):
            errors.append(f"docs impact cause disagrees with file review record: {path}")
        if entry.get("stable_facts") != expected_facts:
            errors.append(f"docs impact stable facts disagree with file review record: {path}")
        if entry.get("evidence") != expected_evidence:
            errors.append(f"docs impact evidence disagrees with file review record: {path}")

    updated_docs = [path for path in entries if (repo / path).is_file()]
    generated = set(extra_generated)
    if updated_docs:
        generated.update({"markdown-links", "doc-status"})
    for check in sorted(generated):
        if check == "markdown-links":
            errors.extend(check_markdown_links(repo, updated_docs))
        elif check == "doc-status":
            errors.extend(check_doc_status(repo, updated_docs))
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
    parser.add_argument("--committed-only", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    repo = args.repo_root.resolve()
    try:
        manifest = load_json(args.manifest)
        config = load_config(args.config)
        if args.changed_path:
            changed_paths = set(args.changed_path)
        elif args.committed_only:
            base_sha = args.base_sha or str(manifest.get("task", {}).get("base_sha", ""))
            changed_paths = discover_committed_paths(repo, base_sha)
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
