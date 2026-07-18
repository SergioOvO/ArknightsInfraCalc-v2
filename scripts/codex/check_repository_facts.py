#!/usr/bin/env python3
"""Check repository-wide stable documentation, lifecycle, and CLI facts."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import docs_inventory
from check_feedback_evidence import check_tracking
from check_docs_impact import check_cli_help_map, check_markdown_links


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
    for document in documents:
        errors.extend(docs_inventory.review_errors(repo, document))
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
