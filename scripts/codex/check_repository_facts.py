#!/usr/bin/env python3
"""Check repository-wide stable documentation, lifecycle, and CLI facts."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from urllib.parse import unquote

import docs_inventory
from check_feedback_evidence import check_tracking


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
            if not (path.parent / local).resolve(strict=False).exists():
                errors.append(f"broken Markdown link in {document}: {raw_target}")
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
    source_commands = set(
        re.findall(r'^\s*"([a-z][a-z-]*)"\s*=>', main_text[match_start:match_end], re.MULTILINE)
    )

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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--allow-in-progress", action="store_true")
    args = parser.parse_args()
    repo = args.repo_root.resolve()

    documents, errors = docs_inventory.load_inventory(repo)
    errors.extend(docs_inventory.validate_inventory(repo, documents, final=not args.allow_in_progress))
    errors.extend(docs_inventory.check_generated_sections(repo, documents))
    errors.extend(check_markdown_links(repo, [document.path for document in documents]))
    errors.extend(check_tracking(repo, verify_local=False))
    errors.extend(check_cli_help_map(repo))
    if errors:
        for error in sorted(set(errors)):
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "PASS repository_facts "
        f"documents={len(documents)} domains={sum(len(document.domains) for document in documents)} "
        "lifecycle=ok links=ok feedback=ok cli_help_map=ok"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
