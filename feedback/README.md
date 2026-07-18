# Feedback Tracking

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/MAINTENANCE_MODE.md；docs/文档生命周期.md
> 复核触发：scripts/sync_feedback.py
> 摘要：说明本地生产反馈 bundle 和 evidence ledger 边界
> 源摘要：b5704c4f9534b41799501f898e65e5731e05fd5ba6b1991801b6357b5e0580b0
> 文档摘要：7644bf13e89e91800eba333aea40b0f191775f51ce91fb7f28cb01acfc03e623
> 复核原因：source-change
> 复核结论：updated
> 稳定事实：说明本地生产反馈 bundle 和 evidence ledger 边界
> 证据引用：tracked:feedback/README.md

This directory stores production feedback bundles copied from the deployed beta/test service.

Treat each feedback folder as raw evidence. Do not edit `issue.json`, `meta.json`, `debug-bundle.json`, or `operbox.json` while triaging. Add investigation state in [TRACKING.md](TRACKING.md) instead.

The imported 2026-06-27 to 2026-06-30 batch is closed as of 2026-07-03. [TRACKING.md](TRACKING.md) is now a closure audit and regression guard, not an open bug queue. Reopen a row only if the same symptom is reported again.

## Folder Shape

Each case normally contains:

| File | Meaning |
|------|---------|
| `issue.json` | User-visible complaint, selected room, note, and original online command |
| `meta.json` | Feedback id, saved time, source name, and bundle availability |
| `debug-bundle.json` | Online run context: layout, operbox summary, command, exit code, and result payload when available |
| `operbox.json` | The submitted operator box for local reproduction |

## Status Values

| Status | Meaning |
|--------|---------|
| `intake` | Imported and indexed, not reproduced locally yet |
| `reproduced` | Local command reproduces the reported symptom |
| `localized` | Suspected layer is confirmed |
| `fixing` | A minimal fix is in progress |
| `regressed` | A fixture, CSV anchor, or test now protects the case |
| `closed` | Fix and regression are verified |
| `blocked` | Missing input, unclear expectation, or cannot reproduce |
| `duplicate` | Tracked through another case id |
| `duplicate-covered` | Closed through another feedback case in the same root-cause family |

## Triage Rule

For each case:

1. Record the local reproduction command before changing code.
2. Localize to one layer: CLI, layout, schedule, search, solver, mechanism, data, or output.
3. Fix only that layer.
4. Add the smallest regression that would fail before the fix.
5. Update [TRACKING.md](TRACKING.md) with status, fixed commit, and verification command.

If a case uses private account data, keep the raw bundle local unless the user explicitly asks to commit it.
