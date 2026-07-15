#!/usr/bin/env python3
"""Compare exact Cargo/libtest failure-name sets from complete evidence logs."""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path


RESULT_RE = re.compile(r"^test result: (ok|FAILED)\..*? (\d+) failed;", re.MULTILINE)
TEST_ACTIVITY_RE = re.compile(r"^(?:running \d+ tests?|test .+ \.\.\. (?:ok|FAILED))$", re.MULTILINE)


class ParseError(ValueError):
    pass


def parse_failure_names(text: str, source: str = "<memory>") -> set[str]:
    matches = list(RESULT_RE.finditer(text))
    if not matches:
        if TEST_ACTIVITY_RE.search(text):
            raise ParseError(f"truncated test log without final result: {source}")
        raise ParseError(f"unrecognized Cargo/libtest log format: {source}")

    names: set[str] = set()
    previous_end = 0
    for match in matches:
        outcome = match.group(1)
        failed_count = int(match.group(2))
        segment = text[previous_end : match.start()]
        previous_end = match.end()
        if outcome == "ok":
            if failed_count != 0:
                raise ParseError(f"inconsistent ok result with failures in {source}")
            continue
        if failed_count == 0:
            raise ParseError(f"inconsistent FAILED result without failures in {source}")

        marker = segment.rfind("\nfailures:\n")
        marker_width = len("\nfailures:\n")
        if marker < 0 and segment.startswith("failures:\n"):
            marker = 0
            marker_width = len("failures:\n")
        if marker < 0:
            raise ParseError(f"FAILED result has no failure summary in {source}")

        summary = segment[marker + marker_width :]
        block_names = []
        for line in summary.splitlines():
            if not line.startswith("    "):
                continue
            name = line.strip()
            if not name or name.startswith("----") or name.endswith(" stdout ----"):
                continue
            block_names.append(name)
        if len(block_names) != failed_count:
            raise ParseError(
                f"failure summary count mismatch in {source}: "
                f"reported={failed_count} parsed={len(block_names)}"
            )
        names.update(block_names)
    return names


def parse_log(path: Path) -> set[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise ParseError(f"cannot read {path}: {error}") from error
    return parse_failure_names(text, str(path))


@dataclass(frozen=True)
class Comparison:
    baseline: list[str]
    current: list[str]
    added: list[str]
    removed: list[str]
    unchanged: list[str]

    @classmethod
    def from_sets(cls, baseline: set[str], current: set[str]) -> "Comparison":
        return cls(
            baseline=sorted(baseline),
            current=sorted(current),
            added=sorted(current - baseline),
            removed=sorted(baseline - current),
            unchanged=sorted(baseline & current),
        )

    def as_dict(self, baseline_log: Path, current_log: Path) -> dict[str, object]:
        return {
            "schema_version": 1,
            "baseline_log": str(baseline_log.resolve()),
            "current_log": str(current_log.resolve()),
            "policy": "fail-on-added; allow-same-or-reduced",
            "result": "FAIL" if self.added else "PASS",
            "baseline": self.baseline,
            "current": self.current,
            "added": self.added,
            "removed": self.removed,
            "unchanged": self.unchanged,
        }


def _markdown_list(values: list[str]) -> str:
    if not values:
        return "- （无）"
    return "\n".join(f"- `{value}`" for value in values)


def render_markdown(comparison: Comparison, baseline_log: Path, current_log: Path) -> str:
    result = "FAIL（出现新增失败）" if comparison.added else "PASS（失败集合相同或减少）"
    return f"""# Full-suite 失败集合比较

- 结果：{result}
- Baseline：`{baseline_log.resolve()}`（{len(comparison.baseline)}）
- Current：`{current_log.resolve()}`（{len(comparison.current)}）

## Added

{_markdown_list(comparison.added)}

## Removed

{_markdown_list(comparison.removed)}

## Unchanged

{_markdown_list(comparison.unchanged)}
"""


def _write(path: Path | None, content: str) -> None:
    if path is None:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--current", type=Path, required=True)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--report-out", type=Path)
    return parser


def main() -> int:
    args = build_parser().parse_args()
    try:
        comparison = Comparison.from_sets(parse_log(args.baseline), parse_log(args.current))
    except ParseError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2

    data = comparison.as_dict(args.baseline, args.current)
    markdown = render_markdown(comparison, args.baseline, args.current)
    _write(args.json_out, json.dumps(data, ensure_ascii=False, indent=2) + "\n")
    _write(args.report_out, markdown)
    if args.report_out is None:
        print(markdown, end="")
    return 1 if comparison.added else 0


if __name__ == "__main__":
    raise SystemExit(main())
