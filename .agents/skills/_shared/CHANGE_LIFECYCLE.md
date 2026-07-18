# Change Lifecycle Protocol

Use this protocol when a task owns an active TODO, plan, change package, or an
explicit documentation-governance migration. It supplements the task's primary
Skill; it does not replace maintenance, feature, or quality ownership.

## Document Roles

- Current canonical documents describe behavior that is true now.
- Active change documents describe proposed deltas, design, tasks, and evidence.
- ADRs preserve accepted or superseded decisions and their rationale.
- Generated references carry facts reproducible from code, schemas, or commands.
- Archived documents preserve closed context and never act as current truth.
- Agent memory may point to these sources but must not define domain semantics,
  implementation status, architecture decisions, or active work.

## Close as a Transaction

A tracked change is not complete when only its implementation or checklist is
complete. Before reporting completion:

1. Verify the real entry and risk-matched regressions through the evidence
   protocol.
2. Merge confirmed behavior and limits into the unique current canonical owner.
3. Record durable architectural decisions in an ADR when the rationale must
   survive the change package.
4. Split every still-valid open item into a new, independently owned change;
   do not archive unresolved work as completed.
5. Remove the closed item from active indexes and move it to the appropriate
   completed, superseded, or historical-design archive.
6. Update indexes and repository references, then prove links, lifecycle state,
   docs impact, and task scope.

Treat these steps as one closure transaction. Do not stop after adding an
"obsolete" or "completed" banner to a document that belongs in the archive.

## Autonomous Closure

Archive or remove an old path without another approval wait when all of the
following are true:

- the implementation or replacement is verified;
- the unique current owner contains every still-valid semantic fact;
- open work is empty or has been split into a new change;
- no current canonical documents conflict;
- indexes and references can be updated in the same change.

Pause only when closing the change would choose between conflicting domain
rules, discard an unabsorbed user decision, change product scope, or cross an
otherwise irreversible user boundary. File count and migration size alone are
not reasons to stop.

## Documentation-Governance Unit

For a documentation-governance task, the smallest correct unit is a closed
lifecycle boundary, not the fewest edited files. It may include moving,
merging, archiving, or deleting every conflicting path required to leave one
current owner. Avoid unrelated prose rewrites once that boundary is closed.

## Required Report

Report the current owner, absorbed facts, archived or deleted paths, split open
work, index/reference updates, structure evidence, unverified semantics, and
remaining active changes.
