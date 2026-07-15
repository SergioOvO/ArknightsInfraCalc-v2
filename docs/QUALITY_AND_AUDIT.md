# 质量与验证证据总则

> 状态：维护期当前规则。
> 责任：本文件是验证、完成证明、失败基线和 Bake 安全的唯一文字真源。
> 边界：不裁决领域业务语义；业务规则以用户裁决和对应领域 Markdown 为准。

## 1. 入口与适用范围

- 普通 bug 和结果不对：先读 [MAINTENANCE_MODE.md](MAINTENANCE_MODE.md)。
- 体系 / 编排正式审计：只在 `formal-audit` 模式读取 [SYSTEM_AUDIT_WORKFLOW.md](SYSTEM_AUDIT_WORKFLOW.md)。
- 当前代码与命令事实：读取 [PROJECT_MAP.md](PROJECT_MAP.md)；领域文档由 [INDEX.md](INDEX.md) 路由。
- 命令执行和 manifest：使用 [scripts/codex/README.md](../scripts/codex/README.md)。

本文件不复制四项体系审计、完整生命周期问题表或普通 bug 复现命令；这些内容分别由 system audit 和 maintenance 文档负责。

## 2. 业务语义边界

验证只能证明命令、输入、输出和证据的一致性，不能用代码行为、旧测试、fixture、历史输出或脚本结果推翻领域 Markdown。两个当前领域 Markdown 冲突时，任务状态必须为 `blocked`，等待用户裁决；不能选择一份继续实现。

## 3. 回归分层

回归应按责任半径分层，不用单个大快照代替全部证明。

### 3.1 数据与机制单元层

- `skill_table.id == buff_id` 和 operator instance 归属。
- Selector、Condition、Action、Phase 和 family 合并规则。
- 0 / 1 / 多消费者、阈值边界、同房与跨站反例。
- shortcut 命中与拒绝条件。

### 3.2 单站 solver 层

- 指定房间成员的最终效率和分量。
- 普通候选与 shortcut 候选使用同一最终排序口径。
- 候选自身必须进入 `base_workforce` / facility workforce 投影，并按姓名去重。

### 3.3 Layout 生命周期层

- 激活、关闭、anchor 进编、禁止替代和跨房可见性。
- 顺序搜索必须让后房看到前房已提交成员。
- 最终 assignment 完成后刷新所有受跨房状态影响的 snapshot；若前房最优性本身依赖后房成员，必须联合枚举，不能只在末尾刷新旧选择。
- 完整池与结构化扩池必须覆盖合法候选，不能只依赖工具人白名单。

### 3.4 Rotation 层

- 实际 bind 成员同上同下、上岗 / 休息次数正确。
- 未入选候选不得在轮换层重新强塞。
- producer 休息班不得残留其作用；每班中枢和生产设施满编且无重复干员。
- 暖机稳定只约束实际连续上岗的房间，不构成最低进编班数。

### 3.5 CLI 与导出层

- 至少运行一次用户真实 `plan` 或 `layout team-rotation` 入口。
- profile JSON 和 MAA JSON 使用任务专属路径写入 `out/`。
- 对 stdout / MAA 断言设施类型、实际成员、工作 / 休息状态和核心字段；profile 只断言账号分析与 rotation 指标，不要求它包含最终 MAA 那次完整房间 assignment。
- `plan` 当前会为 profile 和最终 stdout / MAA 分别运行 rotation；比较两者时核对指标与不变量，不宣称它们共享同一个 in-memory assignment。

## 4. 验证留痕

任何用于结论的 test、build、CLI smoke、benchmark、格式和结构校验都必须通过 `scripts/codex/run_evidence.sh` 或兼容包装器留痕；裸跑后必须带证据重跑。每份日志至少包含：

- 完整命令和 cwd。
- 输入 layout、operbox、assignment、fixture 和 baseline 路径。
- 开始、结束时间和耗时。
- 完整 stdout + stderr。
- exit code 和明确的 PASS / FAIL 摘要。

真实 `plan` 必须显式指定：

```text
--profile-out out/<task>-profile.json
--maa-out out/<task>-maa.json
```

包装器把日志、status 和聚合 manifest 写入 `target/codex-runs/<task>/`。使用 `--artifact kind=path` 登记 profile、MAA、失败集合报告和其他产物；重复运行不得覆盖旧日志。裸跑结果、终端滚屏、Agent 消息、`/tmp` 和 commit hash 都不能代替最终证据。

每个代码 / 数据任务 manifest 必须包含：

- `change_scope`：唯一不变量、根因层、required paths、allowed consumers、proof paths 和 explicitly deferred；
- `scope_expansions` 与 `side_findings`；
- `docs_impact.status`、`checked`、`updated`、`routes` 和具体 `reason`；
- reviewer 最终核对的 invariant、实际 changed paths 和 expansion id；
- runs、artifacts、输入和 exit code。

完成前运行 `check_docs_impact.py`、`check_task_scope.py` 和 `render_evidence.py`。检查器只验证声明与文件事实，不能替代 reviewer 的语义判断。

证据至少保留到任务交付完成。交付后可按任务目录归档或清理；具有长期审计价值的结论应保存稳定摘要或 artifact，不能假设 `target/` 永远不会被 `cargo clean` 删除。本次工作流不要求迁移或批量删除旧日志。

## 5. Full suite 与失败基线

本仓库当前不能被笼统描述为“全套测试全绿”。历史和当前维护证据中存在既有失败；具体集合以本轮开始前保存的完整 baseline 日志为准，不在本文硬编码一个容易过时的数量。

Full suite 验收必须：

1. 保存完整失败列表。
2. 从原始 baseline 和当前日志提取测试全名集合。
3. 分别报告 additions、removals 和 unchanged；不能只比较失败数量。
4. 新增失败为 0 才能宣称“没有新增 full-suite 回归”。
5. 修复或删除一个旧失败必须说明其业务依据；旧测试可能本身保护错误语义。

使用 `scripts/codex/compare_test_failures.py` 从完整日志生成 JSON 与 Markdown；新增失败返回非零，日志截断或格式不识别必须硬失败，不能解析成空集合成功。

Baseline 是检测回归的工具，不是允许既有错误永久存在的豁免，也不能用于宣称全套测试通过。

## 6. Bake 安全门禁

Bake 是加速载体，不是业务真源。预计算结果只有在候选结构和所有相关上下文与生成模型兼容时才能使用。

### 6.1 当前 schema v10 已实现的门禁

- schema、CLI generator 和输入文件 fingerprint 一致；输入覆盖 baseline layout、instances、skill table、standalone roster、segments、shortcuts 和 systems。
- runtime pool 中的候选名必须被 catalog 覆盖，且当前快速路径要求兼容的 E2 tier 模型。
- room level、capacity、recipe / order、mood、shift hours 与 baseline context 必须满足各设施 gate。
- 动态 inject、候选投影、`OperatorInBase` / `OperatorInTrade`、跨房 workforce 或 `full_pool` 等未被当前表精确表达的上下文会拒绝 Bake。
- catalog 缺失、schema / generator / input mismatch 时返回实时搜索。

当前工作区旧 catalog 与代码 schema 的具体状态见 [PERFORMANCE_ENGINEERING.md](PERFORMANCE_ENGINEERING.md)。当前 loader 尚未校验 `combo_table.bin` 自身内容 hash；非白名单反序列化错误也可能作为错误返回，而不是无条件 fallback；生成物也尚未采用完整 generation-id 原子切换。因此不能把下面的未来要求写成当前能力。

### 6.2 下一代 catalog 必须补齐

- catalog 自身 bytes、内容 hash、row count 和 index checksum。
- 损坏、反序列化失败、未知 signature 与缺行全部安全进入同语义 live 求值。
- 临时 generation 目录、完整校验和原子切换，避免读取新旧文件混合状态。
- cache miss 只替代候选生成 / 求值方式，不能改用更小候选集、固定 top-K 或旧 pipeline。

未来联合候选 Bake 的 winner 仍必须在完整临时 assignment 上统一 resolve；DP、Pareto 和安全上界不是首期前提。详见 [DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

## 7. 完成证明

交付前逐条填写：

| 不变量 | 代码保证 | 删除的冲突 | 回归 | 端到端结果 | 日志 | 产物 |
|--------|----------|------------|------|------------|------|------|
| 逐项填写 | 唯一类型 / 字段 / 函数 | 被删除的旧路径 | 测试名与断言 | 实际房间 / 队伍 / 字段 | 可点击绝对路径 | 可点击绝对路径 |

最终回复还必须单列：

- 根因层和旧模型为什么允许非法状态。
- 新的单一事实源。
- 本轮通过项、既有失败、新增失败和未验证风险。
- 实际运行过的用户入口。
- Build、定向测试、full suite、真实 CLI、性能和 JSON 的验证证据。
- commit hash；未提交时明确写“未提交”。
