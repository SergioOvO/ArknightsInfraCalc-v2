# Codex 证据与任务边界工具

本目录只机械记录和检查事实，不裁决业务语义。领域正确性仍由用户裁决、当前领域 Markdown、reviewer 和主 Agent 判断。

## 统一命令包装器

```bash
scripts/codex/run_evidence.sh \
  --task issue-slug \
  --category targeted-test \
  --stem infra-core-case \
  --inputs 'fixture=data/example.json' \
  -- cargo test -p infra-core test_name -- --exact
```

支持重复 `--artifact kind=path` 登记命令产物，支持 `--metadata task.json` 写入任务范围、文档影响、旁支发现和 reviewer 声明。metadata 结构见 `task_metadata.example.json`。

证据默认写入：

```text
target/codex-runs/<task>/
  manifest.json
  commands/<run-id>/<category>-<stem>.log
  commands/<run-id>/<category>-<stem>.status
  reports/
```

包装器使用参数数组执行原命令，不执行 `eval`，并返回原命令 exit code。每个命令目录由原子 `mkdir` 占位，manifest 通过文件锁和原子替换更新，因此并发调用不会覆盖证据。

最终证据类别使用：`build`、`targeted-test`、`full-suite`、`cli`、`performance`。格式和结构检查可以使用 `format`、`structure`；渲染器会把它们列在“其他验证”。

## Full-suite 失败集合

```bash
scripts/codex/compare_test_failures.py \
  --baseline target/codex-runs/baseline.log \
  --current target/codex-runs/current.log \
  --json-out target/codex-runs/task/reports/failures.json \
  --report-out target/codex-runs/task/reports/failures.md
```

新增失败返回 1；集合相同或只减少返回 0；日志截断、格式不识别或失败摘要不完整返回 2。

## 完成检查

```bash
scripts/codex/check_docs_impact.py --manifest target/codex-runs/<task>/manifest.json
scripts/codex/check_task_scope.py --manifest target/codex-runs/<task>/manifest.json
scripts/codex/render_evidence.py \
  --manifest target/codex-runs/<task>/manifest.json \
  --output target/codex-runs/<task>/reports/evidence.md
```

`check_docs_impact.py` 使用 `docs_impact.toml` 将 changed paths 路由到必须检查的文档。它验证 `updated`、`not-needed`、`blocked`、领域路由说明、文件存在性、虚假更新声明、局部 Markdown 链接和 CLI 命令地图，但不判断文字语义是否正确。

`check_task_scope.py` 检查实际 changed paths 是否属于 `change_scope`、带理由的 scope expansion、证明路径或已声明文档更新；显式 deferred 路径和 deferred side finding 被修改时硬失败。完成检查还要求 reviewer 登记最终不变量、精确 changed paths 和全部 expansion id。

`render_evidence.py` 会交叉核对 manifest、status、log 内的 exit code / PASS / FAIL，并检查所有登记产物存在。缺少的 build、定向测试、full suite、CLI、性能或 JSON 类别会明确渲染为“未跑”。

## 自测

所有自测也必须通过证据包装器运行：

```bash
scripts/codex/run_evidence.sh \
  --task codex-tools \
  --category targeted-test \
  --stem unittest \
  --inputs 'scripts/codex tests and fixtures' \
  -- python3 -m unittest discover -s scripts/codex/tests -p 'test_*.py' -v
```
