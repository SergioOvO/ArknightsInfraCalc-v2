#!/usr/bin/env python3
"""Mechanically enforce a task manifest's declared change radius."""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


class ScopeError(ValueError):
    pass


def load_manifest(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise ScopeError(f"cannot read manifest {path}: {error}") from error
    if not isinstance(value, dict):
        raise ScopeError("manifest must be a JSON object")
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
        raise ScopeError("--committed-only requires --base-sha")
    return _git_paths(repo, ["diff", "--name-only", "--diff-filter=ACMRD", f"{base_sha}..HEAD"])


def _matches(path: str, patterns: list[str]) -> bool:
    return any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns)


def _path_list(value: object, field: str, errors: list[str]) -> list[str]:
    if not isinstance(value, list) or not all(isinstance(item, str) and item for item in value):
        errors.append(f"{field} must be an array of non-empty path globs")
        return []
    return value


def _specific(value: object, minimum: int = 12) -> bool:
    return isinstance(value, str) and len(value.strip()) >= minimum


def run_checks(manifest: dict[str, Any], changed_paths: set[str]) -> list[str]:
    errors: list[str] = []
    if manifest.get("schema_version") != 3:
        errors.append("manifest must use schema_version=3")
    if "docs_impact" in manifest:
        errors.append("manifest schema v3 does not allow docs_impact")
    scope = manifest.get("change_scope")
    if not isinstance(scope, dict):
        return ["manifest.change_scope must be an object"]
    invariant = scope.get("invariant")
    root_layer = scope.get("root_cause_layer")
    if not _specific(invariant):
        errors.append("change_scope.invariant must be specific and at least 12 characters")
    if not _specific(root_layer, minimum=3):
        errors.append("change_scope.root_cause_layer must identify the responsibility layer")

    required = _path_list(scope.get("required_paths"), "change_scope.required_paths", errors)
    consumers = _path_list(scope.get("allowed_consumers"), "change_scope.allowed_consumers", errors)
    proofs = _path_list(scope.get("proof_paths"), "change_scope.proof_paths", errors)
    deferred = _path_list(scope.get("explicitly_deferred"), "change_scope.explicitly_deferred", errors)

    expansions = manifest.get("scope_expansions")
    if not isinstance(expansions, list):
        errors.append("scope_expansions must be an array")
        expansions = []
    expansion_patterns: list[str] = []
    expansion_ids: list[str] = []
    for index, expansion in enumerate(expansions):
        if not isinstance(expansion, dict):
            errors.append(f"scope_expansions[{index}] must be an object")
            continue
        expansion_id = expansion.get("id")
        paths = _path_list(expansion.get("paths"), f"scope_expansions[{index}].paths", errors)
        if not isinstance(expansion_id, str) or not expansion_id:
            errors.append(f"scope_expansions[{index}].id is required")
        elif expansion_id in expansion_ids:
            errors.append(f"duplicate scope expansion id: {expansion_id}")
        else:
            expansion_ids.append(expansion_id)
        if not _specific(expansion.get("reason")):
            errors.append(f"scope_expansions[{index}].reason must explain necessity")
        if not _specific(expansion.get("evidence"), minimum=3):
            errors.append(f"scope_expansions[{index}].evidence is required")
        expansion_patterns.extend(paths)

    side_findings = manifest.get("side_findings")
    if not isinstance(side_findings, list):
        errors.append("side_findings must be an array")
        side_findings = []
    deferred_finding_paths: list[str] = []
    for index, finding in enumerate(side_findings):
        if not isinstance(finding, dict):
            errors.append(f"side_findings[{index}] must be an object")
            continue
        if finding.get("disposition") == "deferred":
            paths = finding.get("paths", [])
            if not isinstance(paths, list) or not all(isinstance(item, str) and item for item in paths):
                errors.append(f"side_findings[{index}].paths must be path globs when deferred")
            else:
                deferred_finding_paths.extend(paths)

    allowed = required + consumers + proofs + expansion_patterns
    for path in sorted(changed_paths):
        if _matches(path, deferred):
            errors.append(f"explicitly deferred path was modified: {path}")
        if _matches(path, deferred_finding_paths):
            errors.append(f"deferred side finding was implemented: {path}")
        if not _matches(path, allowed):
            errors.append(f"changed path is outside declared scope: {path}")

    reviewer = manifest.get("reviewer")
    if not isinstance(reviewer, dict) or reviewer.get("status") != "reviewed":
        errors.append("reviewer.status must be reviewed")
    else:
        if reviewer.get("scope_invariant") != invariant:
            errors.append("reviewer scope invariant does not match final change_scope")
        reviewed_paths = reviewer.get("changed_paths")
        if not isinstance(reviewed_paths, list) or set(reviewed_paths) != changed_paths:
            errors.append("reviewer changed_paths must exactly match the actual changed paths")
        reviewed_expansions = reviewer.get("scope_expansion_ids")
        if not isinstance(reviewed_expansions, list) or set(reviewed_expansions) != set(expansion_ids):
            errors.append("reviewer scope_expansion_ids must match the final expansion history")
    return errors


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--repo-root", type=Path, default=Path.cwd())
    parser.add_argument("--base-sha")
    parser.add_argument("--changed-path", action="append", default=[])
    parser.add_argument("--committed-only", action="store_true")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        manifest = load_manifest(args.manifest)
        if args.changed_path:
            changed_paths = set(args.changed_path)
        elif args.committed_only:
            base_sha = args.base_sha or str(manifest.get("task", {}).get("base_sha", ""))
            changed_paths = discover_committed_paths(args.repo_root.resolve(), base_sha)
        else:
            base_sha = args.base_sha
            if base_sha is None:
                base_sha = str(manifest.get("task", {}).get("base_sha", ""))
            changed_paths = discover_changed_paths(args.repo_root.resolve(), base_sha)
        errors = run_checks(manifest, changed_paths)
    except (ScopeError, subprocess.CalledProcessError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1
    print(f"PASS task_scope changed_paths={len(changed_paths)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
