---
name: arknights-system-audit
description: Audit ArknightsInfraCalc systems, hard cores, producers, same-room or cross-station scope, required anchors, rotation binds, and orchestration changes across the full lifecycle. Use for system bugs, cross-facility behavior, producer selection, required anchors, rotation binding, or a user-requested formal audit; do not infer domain rules from top hits or current fixtures.
---

# Arknights System Audit

Use this skill when a rule must be structurally guaranteed by the system/plan boundary rather than accidentally produced by search order.

## Select the Mode

- Use `system-fix` when the current governing Markdown is clear: produce the four-item audit, declare scope, then implement after the audit without two user waits.
- Use `formal-audit` when the user asks for a strict itemized audit or current domain Markdown conflicts: submit the audit, wait for rule rulings, submit the modification plan, wait for approval, then implement with one writer and a read-only reviewer.
- Never choose between conflicting Markdown documents from code, CSV, tests, fixtures, or output. Ask the user and update the authoritative Markdown first.

## Audit Invariants First

Read `AGENTS.md`, [maintenance mode](../../../docs/MAINTENANCE_MODE.md), [system audit workflow](../../../docs/SYSTEM_AUDIT_WORKFLOW.md), the target system Markdown, and the relevant registry/data rows. Write an invariant table covering:

- hard cores, elite levels, minimum counts, and whether all available members are required;
- optional producers and whether they are anchors;
- same-room, cross-station, in-base, recipe, and facility-capacity scope;
- mutual exclusion and shared-room/resource competition;
- team, shift, work/rest, and binding semantics;
- closure and real downgrade conditions;
- numeric scope and effect atoms.

## Trace the Lifecycle

For each invariant, inspect `select -> plan -> execute -> fill -> resolve -> rotation -> export`. Record the exact file/type/function where it can be lost and distinguish:

- activation from actual anchor entry;
- role filters from final solver comparison;
- `shift_bind` from the guarantee that a member is actually assigned;
- shortcut settlement from system selection and membership;
- cross-station scope from same-room coincidence;
- final snapshots from candidate/partial assignment context;
- rotation bindings from accidental `used` or room ordering;
- export fidelity from internal assignment state.

Use minimum-member, missing-core, competing-candidate, reordered-room, cross-station-capacity, and work/rest closure counterexamples where they can change the responsibility boundary. A standard full-E2 top hit is not structural proof.

## Scope and Handoff

Before writing, report the four items: domain invariants, violating lifecycle location, the single responsibility boundary that will guarantee each invariant, and the old fallback/special case/test to delete or rewrite. Add `change_scope` with one primary invariant, root-cause layer, allowed paths, proof paths, and explicitly deferred discoveries. Put adjacent issues in `side_findings` with a disposition of `deferred`; do not create TODOs automatically.

Keep domain Markdown as the business source of truth. Do not encode a rule by operator name, room id, shift index, priority, tag, shortcut, or current top hit. Do not add a second special case in pipeline, role, and rotation when the plan/system schema should own the fact.

After implementation, require invariant-level regression and the real `plan` or `layout team-rotation` entry for scheduling/export changes. Use `arknights-evidence` for logs and `scripts/codex/check_task_scope.py` for the final changed-path review. Report root cause, old illegal-state path, new single source of truth, deleted conflicts, tests, CLI result, and unresolved risks.
