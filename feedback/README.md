# Feedback Tracking

This directory stores production feedback bundles copied from the deployed beta/test service.

Treat each feedback folder as raw evidence. Do not edit `issue.json`, `meta.json`, `debug-bundle.json`, or `operbox.json` while triaging. Add investigation state in [TRACKING.md](TRACKING.md) instead.

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

## Triage Rule

For each case:

1. Record the local reproduction command before changing code.
2. Localize to one layer: CLI, layout, schedule, search, solver, mechanism, data, or output.
3. Fix only that layer.
4. Add the smallest regression that would fail before the fix.
5. Update [TRACKING.md](TRACKING.md) with status, fixed commit, and verification command.

If a case uses private account data, keep the raw bundle local unless the user explicitly asks to commit it.
