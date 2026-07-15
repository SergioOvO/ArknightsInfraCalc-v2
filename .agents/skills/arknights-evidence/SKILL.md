---
name: arknights-evidence
description: Run and audit reproducible evidence for ArknightsInfraCalc builds, tests, CLI smoke checks, benchmarks, formatting, generated JSON, full-suite failure baselines, documentation impact, and task scope. Use whenever a conclusion depends on a command or artifact; never replace domain review with a script result.
---

# Arknights Evidence

Use the repository evidence tools for every build, test, CLI, benchmark, format, or structure check that supports a conclusion. A command that was run without a retained log must be rerun through the wrapper before delivery.

## Record a Run

1. Create a task manifest or metadata file with `change_scope`, `scope_expansions`, `side_findings`, `docs_impact`, and reviewer fields. Start from [the example](../../../scripts/codex/task_metadata.example.json).
2. Run the command with `scripts/codex/run_evidence.sh --task <slug> --category <category> --stem <name> --inputs '<reproducible inputs>' -- [command args...]`.
3. Use `--artifact kind=path` for profile, MAA, comparison reports, or other generated files. The wrapper executes an argument array, records cwd/inputs/command/timing/exit code, returns the original exit code, and atomically appends a task manifest.
4. Keep each repeated run. Never overwrite an earlier log or use a shared generic output filename for a new comparison.

Use these categories: `build`, `targeted-test`, `full-suite`, `cli`, `performance`, `format`, and `structure`. Include the actual fixture, layout, operbox, assignment, and output paths in `--inputs`.

## Compare and Render

For a full suite, run `scripts/codex/compare_test_failures.py` against complete Cargo logs. It extracts exact failure-name sets, writes JSON and Markdown reports, returns 1 for added failures, 0 for an unchanged/reduced set, and 2 for truncation or an unrecognized format. Compare the set, not only the count.

Before claiming completion:

1. Run `scripts/codex/check_docs_impact.py --manifest <manifest>`; resolve `updated`, `not-needed`, `blocked`, required document, route, link, and generated-fact errors.
2. Run `scripts/codex/check_task_scope.py --manifest <manifest>`; do not suppress undeclared paths, deferred paths, or missing expansion reasons.
3. Run `scripts/codex/render_evidence.py --manifest <manifest> --output <report>` and inspect its explicit “未跑” categories, exit-code consistency, artifact existence, scope, and deferred findings.

The renderer is evidence formatting, not a semantic judge. The main Agent must still inspect domain output against Markdown, decide whether a test expresses correct semantics, and distinguish new failures, baseline failures, and unverified risks. A missing build, targeted test, full suite, real CLI, performance run, or JSON artifact must be reported as not run rather than implied to pass.
