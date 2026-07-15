#!/usr/bin/env python3
"""Check the repository's stable Markdown and CLI facts without a task manifest."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from check_docs_impact import check_cli_help_map, check_doc_status, check_markdown_links


STABLE_DOCUMENTS = [
    "docs/INDEX.md",
    "docs/PROJECT_MAP.md",
    "docs/MAINTENANCE_MODE.md",
    "docs/QUALITY_AND_AUDIT.md",
    "docs/SYSTEM_AUDIT_WORKFLOW.md",
    "docs/BASE_ASSIGNMENT.md",
    "docs/SCHEDULE_ROTATION.md",
    "docs/INFRA_CLI.md",
    "docs/FRONTEND_CLI.md",
    "scripts/codex/README.md",
]

STATUS_DOCUMENTS = [
    "docs/MAINTENANCE_MODE.md",
    "docs/QUALITY_AND_AUDIT.md",
    "docs/SYSTEM_AUDIT_WORKFLOW.md",
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    args = parser.parse_args()
    repo = args.repo_root.resolve()

    errors = [
        *check_markdown_links(repo, STABLE_DOCUMENTS),
        *check_doc_status(repo, STATUS_DOCUMENTS),
        *check_cli_help_map(repo),
    ]
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(
        "PASS repository_facts "
        f"documents={len(STABLE_DOCUMENTS)} status_documents={len(STATUS_DOCUMENTS)} cli_help_map=ok"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
