---
name: arknights-maintenance
description: Reproduce and repair ArknightsInfraCalc maintenance issues with the smallest responsible change, focused regression coverage, and a real CLI proof. Use for bug reports, wrong results, CLI, data, solver, layout, schedule, export, or "run it once" requests; hand formal system audits to arknights-system-audit and all validation evidence to arknights-evidence.
---

# Arknights Maintenance

Use this skill for ordinary maintenance work. Preserve domain semantics and keep the change tied to one observable invariant.

## Route

1. Read the repository `AGENTS.md`, then [maintenance mode](../../../docs/MAINTENANCE_MODE.md), [project map](../../../docs/PROJECT_MAP.md), and the domain document routed by [INDEX](../../../docs/INDEX.md).
2. Choose `maintenance` for local CLI/data/solver issues. Choose `system-fix` for a system, cross-facility, orchestration, or rotation issue when the governing Markdown is clear. Choose `formal-audit` only when the user asks for a strict audit or current domain Markdown conflicts; hand that flow to `arknights-system-audit`.
3. Record the actual command, layout, operbox, assignment, expected result, and current result. Reproduce through the existing CLI entry before editing.

## Before Writing

Declare a task scope in the evidence manifest or task brief:

- one precise invariant;
- the layer where the invalid state first appears;
- `required_paths`, `allowed_consumers`, `proof_paths`, and `explicitly_deferred`;
- expected regression and the user-facing CLI entry;
- `docs_impact` with checked documents and a concrete reason.

For `system-fix`, also provide the four-item audit: domain invariants, violating lifecycle location, the single post-fix responsibility boundary, and the old conflict paths to delete or rewrite. Do not use a downstream fallback, `shift_bind`, priority, room id, operator name, or current top hit as a substitute for a declared invariant.

If Markdown semantics conflict, stop writing and request the user’s ruling. Ordinary implementation choices do not require a second approval or a subagent. With an existing dirty worktree, avoid writing concurrently; use an isolated worktree when practical.

## Implement and Stop

1. Change only the layer that first permits the invalid state. Keep the normal `select -> plan -> execute -> fill -> resolve -> rotation -> export` path intact.
2. Add regression assertions for activation/closure, members, scope, recipe, rotation, or exported fields as applicable, not only a total efficiency snapshot.
3. Run evidence commands through `scripts/codex/run_evidence.sh`; use categories `targeted-test`, `cli`, `build`, or `performance` and register generated artifacts.
4. Stop when the invariant has one responsible boundary, direct conflict paths are removed, every changed path is declared, focused regression and the required real entry have evidence, and remaining discoveries are deferred side findings.

Do not revive unrelated TODO plans, repair the existing full-suite debt in a local bug, or expand a fix merely because adjacent code is nearby. Report the root-cause layer, single fact source, changed paths, deferred findings, documentation decision, tests, real CLI entry, and evidence links.
