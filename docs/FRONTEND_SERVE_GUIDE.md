# Frontend Serve Guide

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/FRONTEND_CLI.md；docs/INFRA_CLI.md
> 复核触发：crates/infra-cli/src/commands/serve.rs；crates/infra-cli/src/output.rs
> 摘要：说明前端 serve worker 的当前使用方式
> 源摘要：6506a6b990dfbf591677021c52cc6a61e213662f2866112bccb4dd7a9ae3b49c
> 文档摘要：a38461d4239faf5f4f7324cf67286e647c1ea4c6cf309080001fcbe7925a029e
> 复核原因：source-change
> 复核结论：updated
> 稳定事实：说明前端 serve worker 的当前使用方式
> 证据引用：tracked:docs/FRONTEND_SERVE_GUIDE.md

Use `infra-cli serve` instead of spawning one CLI process per layout.

## Protocol

Start once:

```bash
infra-cli serve
```

- stdin: one JSON request per line.
- stdout: one JSON response per line.
- stderr: logs only; do not parse as protocol.

## Request

```json
{"id":1,"method":"plan","params":{"layout":"tmp/layout.json","operbox":"tmp/operbox.json","profile_out":"tmp/profile.json","maa_out":"tmp/maa.json","output_dir":"tmp/shifts","top":20,"maa_title":"My schedule"}}
```

All file paths are chosen by the frontend.

`plan.params`:

| Field | Required | Meaning |
|------|----------|---------|
| `operbox` | yes | OperBox JSON or xlsx |
| `layout` | no | layout JSON; default is built-in 243 fixture |
| `baseline` | no | profile comparison operbox |
| `profile_out` | no | profile JSON output path |
| `maa_out` | no | MAA JSON output path |
| `output_dir` | no | writes `team_shift_*.json` |
| `top` | no | search depth, default `20` |
| `maa_title` | no | MAA title |

## Response

Success:

```json
{"id":1,"ok":true,"elapsed_ms":123,"result":{"layout":"tmp/layout.json","operbox":"tmp/operbox.json","owned":418,"top":20,"profile_out":"tmp/profile.json","maa_out":"tmp/maa.json","output_dir":"tmp/shifts","daily_trade_efficiency":5.288,"daily_manufacture_efficiency":9.175,"daily_power_efficiency":3.552}}
```

Error:

```json
{"id":1,"ok":false,"elapsed_ms":3,"error":{"message":"..."}}
```

## Frontend Changes

1. Spawn `infra-cli serve` once when the app starts or before the first solve.
2. For each solve, write one request line to stdin.
3. Wait for one stdout line with the same `id`.
4. Read `profile_out` and `maa_out` files after `ok: true`.
5. If the process exits, restart it and resend the active request.
