# 正常维护期指南

> 本文是正常维护 / bug 修复阶段的主入口。目标是让每一次修复都能被复现、被定位、被回归保护，并且不重新打开已经收敛的架构问题。

## 1. 总原则

本项目现在按“稳定优先”维护：

1. **先复现，再修改**：没有复现路径时，不猜测公式和机制。
2. **先缩层，再动刀**：确认 bug 属于 CLI、编排、搜索、solver、interpreter、数据中的哪一层。
3. **最小修复**：只改出错层和必要回归，不顺手重构。
4. **回归优先**：bug 能落成 fixture / CSV / 最小 JSON，就不要只靠人工观察。
5. **口径冻结**：不新增匿名综合分，不重启贸易-制造平衡公式，不用局部需求推翻 scoring policy。

### 1.1 任务模式边界

- `maintenance`：普通 CLI、数据、solver 或局部结果 bug，按本文复现、缩层、最小修复和验证。
- `system-fix`：体系、跨设施、编排或轮换 bug，且当前领域 Markdown 已无歧义；完成四项修改前审计后可直接实施，不要求两次用户等待，也不强制调用 subagent。
- `formal-audit`：用户明确要求逐项严格审计，或两个当前领域 Markdown 互相冲突；按 [SYSTEM_AUDIT_WORKFLOW.md](SYSTEM_AUDIT_WORKFLOW.md) 执行审计裁决与计划批准两个等待点。

模式只决定流程半径，不改变业务真源和四项审计门禁。`system-fix` 发现真实语义冲突时应升级为 `formal-audit`；代码组织、局部接口和测试选择等可逆实现细节不构成升级理由。

## 2. Bug 处理流程

### 2.1 收集输入

每个 bug 至少记录：

| 项 | 说明 |
|----|------|
| 命令 | 用户实际运行的 CLI 命令；没有就用最近等价入口 |
| layout | 布局 JSON 路径或内容 |
| operbox | 练度盒 JSON / xlsx 路径 |
| assignment | 若是 `layout eval` / MAA 导入问题，记录编制 JSON |
| 期望 | 用户期望的干员、效率、产量、队伍或 JSON 字段 |
| 实际 | 当前输出、报错、panic 或差异 |

用户只说“结果不对”时，优先要求或寻找最小可运行输入；如果仓库已有 debug bundle，先从 bundle 跑起。

### 2.2 复现入口

| 场景 | 命令 |
|------|------|
| 用户说“跑一遍模拟” | `cargo run -q -p infra-cli -- plan --operbox data/fixtures/243/operbox_full_e2.json --profile-out out/<task>-profile.json --maa-out out/<task>-maa.json` |
| 只验证三队排班 / MAA | `cargo run -q -p infra-cli -- layout team-rotation --layout <layout> --operbox <operbox> --maa-out <out>` |
| 单班布局搜索 | `cargo run -q -p infra-cli -- layout test --layout <layout> --operbox <operbox> --text` |
| 指定编制结算 | `cargo run -q -p infra-cli -- layout eval --layout <layout> --operbox <operbox> --assignment <assignment> --text` |
| 贸易 shortcut / 产量 | `cargo run -q -p infra-cli -- verify --case <case>` 或 `trade yield <fixture>` |
| 贸易 / 制造池缺人 | `cargo run -q -p infra-cli -- pool --trade --manufacture --operbox <operbox> --text` |
| 性能回退 | `cargo run -q -p infra-cli -- profile layout-full --layout <layout> --operbox <operbox>` |

A-B-A 的 `layout rotation` / `schedule rotation` 已移除；三队轮换 bug 只走 `plan` 或 `layout team-rotation` 复现。

### 2.3 验证证据硬门禁

验证和证据要求以 [QUALITY_AND_AUDIT.md](QUALITY_AND_AUDIT.md) 为唯一文字真源，命令执行以 [scripts/codex/README.md](../scripts/codex/README.md) 为唯一工具入口。任何 test、build、CLI、benchmark、格式或结构校验都必须留痕；full suite 必须比较完整失败集合，真实 `plan` 必须使用任务专属 `out/` 产物。最终回复按实际类别链接证据，未跑项明确写“未跑”。

## 3. 分层定位

### 3.0 代码所有权速查

| 问题落点 | 主要文件 | 备注 |
|----------|----------|------|
| 命令分发、legacy `pool/search/bench/trade` | `crates/infra-cli/src/main.rs` | 不写机制 |
| `plan` 用户主入口 | `crates/infra-cli/src/commands/plan.rs` | profile + team_rotation + MAA |
| `layout test/team-rotation/eval/analyze` | `crates/infra-cli/src/commands/layout.rs` | 只编排 core 调用 |
| 前端常驻 worker | `crates/infra-cli/src/commands/serve.rs` | JSON line 协议 |
| CLI 展示 | `crates/infra-cli/src/output.rs` | CSV/text/JSON，不改评分 |
| 回归壳 | `crates/infra-cli/src/commands/verify.rs`、`crates/infra-cli/src/verify/*` | fixture 与断言 |
| 数据路径 / 嵌入 fallback | `crates/infra-core/src/skill_table.rs` | 发布包常见问题 |
| 干员实例 / tier 合并 | `crates/infra-core/src/instances.rs` | `stepwise` 在这里 |
| operbox / xlsx | `crates/infra-core/src/operbox/*` | 用户输入解析 |
| 贸易单站 | `crates/infra-core/src/trade/*` | L1/L2/L3/产量 |
| 制造单站 | `crates/infra-core/src/manufacture/*` | 当前无 L2/L3 |
| 搜索排序 | `crates/infra-core/src/search/*` | 各域排序口径 |
| 单班编排 | `crates/infra-core/src/layout/assign*`、`layout/orchestrate/*` | 主路径不可绕过 |
| 三队轮换 | `crates/infra-core/src/schedule/team_rotation.rs`、`shift_bind.rs` | 当前排班主路径 |
| MAA 导出 / 导入 | `crates/infra-core/src/export/maa.rs` | JSON 结构问题 |
| 全局资源 | `crates/infra-core/src/global_resource/*`、`cross_facility/*` | scope=Global atom |
| 控制中枢 | `crates/infra-core/src/control/*`、`search/control.rs` | 注入与补位 |
| bake 加速表 | `crates/infra-core/src/bake.rs`、`commands/bake.rs` | 本地生成，谨慎提交产物 |

### 3.1 CLI / 输出层

先看：

- `crates/infra-cli/src/commands/*.rs`
- `crates/infra-cli/src/output.rs`
- `docs/INFRA_CLI.md`
- `docs/FRONTEND_CLI.md`
- `docs/FRONTEND_SERVE_GUIDE.md`

判断标准：

- core 返回值正确，但 CSV / text / JSON 字段错：改 `output.rs`。
- 参数默认值、路径、输出文件错：改对应 `commands/*.rs`。
- 前端常驻协议错：改 `commands/serve.rs`。
- 不要在 CLI 里修机制公式。

### 3.2 数据加载层

先看：

- `crates/infra-core/src/skill_table.rs`
- `crates/infra-core/src/instances.rs`
- `crates/infra-core/src/operbox/mod.rs`
- `data/skill_table.json`
- `data/operator_instances.json`

关键事实：

- `data_path()` 搜索 `ARKNIGHTS_INFRA_DATA_DIR`、可执行文件附近的 `data/`、当前目录 `data/`，最后使用嵌入数据 fallback。
- `operator_instances.json` 是干员到 buff 的运行时归属映射；业务含义仍由领域 Markdown 裁决。
- `skill_table.id` 是解包 `buff_id`，不是旧 `skill_*`。
- tier_up 的 `stepwise` 语义在 `instances.rs::resolve_buff_ids`。

### 3.3 单站贸易

主路径：

```text
TradeRoomInput
  -> TradeContext::from_room
  -> apply_trade_phases
  -> resolve_trade_shortcut 或 resolve_order_mechanic
  -> compute_unit_output / daily_yield
```

优先看：

- `crates/infra-core/src/trade/solver.rs`
- `crates/infra-core/src/trade/shortcut.rs`
- `crates/infra-core/src/trade/order_mechanic.rs`
- `crates/infra-core/src/trade/unit_output.rs`
- `docs/INTERNAL/TRADE_INTERPRETER.md`
- `docs/INTERNAL/SHORTCUT_MATCHING.md`

只有当 EffectAtom 解释顺序、condition、selector、action 真错时，才改 `trade/interpreter.rs`。

回归位置：

- `data/REGRESSION_CASES.csv`
- `data/UNIT_OUTPUT_ANCHORS.csv`
- `crates/infra-cli/src/verify/fixtures.rs`
- `crates/infra-cli/src/commands/verify.rs`

### 3.4 单站制造

主路径：

```text
ManuRoomInput
  -> ManuContext::from_room
  -> apply_manu_phases
  -> prod_total / storage_limit
```

优先看：

- `crates/infra-core/src/manufacture/solver.rs`
- `crates/infra-core/src/manufacture/input.rs`
- `crates/infra-core/src/search/manufacture.rs`
- `docs/MANUFACTURE_STATUS.md`

制造目前没有贸易那样的 L2/L3 shortcut 层；不要把贸易站假设搬过去。

### 3.5 搜索 / 排序

| 域 | 文件 | 当前排序口径 |
|----|------|--------------|
| 贸易 | `search/trade.rs` | `final_efficiency` 直接效率 |
| 制造 | `search/manufacture.rs` | `final_efficiency`；多产线为各线直接效率和 |
| 发电 | `search/power.rs` | `final_efficiency` 直接充能效率 |
| 中枢 | `search/control.rs` | `ControlInjectRawSumV0`：`trade + manu_gold + manu_br` |

如果用户觉得排序“不符合直觉”，先确认是不是展示分量不清，而不是直接改排序。

### 3.6 编排 / 排班

单班主路径：

```text
assign_shift_with_plan_skip
  -> build_plan
  -> execute_plan
  -> run_shift_pipeline
  -> resolve snapshots
  -> control / producer / power / trade / manufacture fill
```

三队轮换主路径：

```text
schedule_team_rotation
  -> peak assignment + plan
  -> alpha / beta / gamma split
  -> shift_bind / control rotation
  -> weighted totals
  -> MAA export
```

优先看：

- `crates/infra-core/src/layout/assign.rs`
- `crates/infra-core/src/layout/assign/pipeline.rs`
- `crates/infra-core/src/layout/assign/*_fill.rs`
- `crates/infra-core/src/layout/orchestrate/*.rs`
- `crates/infra-core/src/schedule/team_rotation.rs`
- `crates/infra-core/src/schedule/shift_bind.rs`
- `crates/infra-core/src/export/maa.rs`
- `docs/ADR/0001-layout-assignment-decomposition.md`
- `docs/SCHEDULE_ROTATION.md`

不要绕过 pipeline 手写 assignment 修 bug；这样会让 `plan`、`layout test`、`team-rotation` 三条路径再次分叉。

## 4. 回归策略

| Bug 类型 | 首选回归 |
|----------|----------|
| 贸易 shortcut / 机制等效 | `REGRESSION_CASES.csv` + `verify/fixtures.rs` |
| 单位产出 / 赤金锚点 | `UNIT_OUTPUT_ANCHORS.csv` + `unit_fixture` |
| 制造单站公式 | `infra-core` 单元测试或最小 `layout test` fixture |
| 搜索排序 | 对应 `search/*` 单元测试，断言排序 key 和 top hit |
| 编排体系选型 | `layout/orchestrate` 单元测试，断言 `AssignmentPlan` |
| 单班落位 | `layout/assign` 单元测试或最小 layout + operbox |
| 三队轮换 | `schedule/team_rotation.rs` 测试或固定 `data/fixtures/243` smoke |
| MAA JSON | `export/maa.rs` 测试或 `plan --maa-out` 文件结构检查 |
| CLI 输出 | `infra-cli` 命令 smoke；必要时 snapshot 最小字段 |

如果 bug 来自用户私有 operbox/layout，优先做可脱敏的最小 fixture，不要提交完整私人数据。

## 5. 验收矩阵

按 [QUALITY_AND_AUDIT.md](QUALITY_AND_AUDIT.md) 选择与改动半径匹配的回归层级，并使用 [scripts/codex/run_evidence.sh](../scripts/codex/run_evidence.sh) 留痕。普通机制问题通常需要数据/solver 定向回归；编排和排班问题必须增加生命周期反例与一次真实 `plan` 或 `layout team-rotation`；CLI/MAA 问题必须检查实际输出字段和任务专属 JSON。

不要用单元测试代替用户入口，也不要用 `layout test` 代替完整模拟；不要为了 `bake validate` 通过而提交大体积生成物。

## 6. 禁止清单

- 不要把 CLI 当作机制层。
- 不要为单一 bug 引入新的全局抽象。
- 不要新增匿名混合权重。
- 不要把复杂降级体系塞回 `base_systems.json`；迷迭香、自动化、红松、莱茵和贸易核心统一由 `data/orchestration_rules.json` 编译成 resolved `AssignmentPlan`，不得恢复 `system_integrity` 专用 evaluator。
- A-B-A 入口已移除，不再作为复现或对照路径。
- 不要提交用户私有 operbox、xlsx、debug bundle，除非用户明确允许并已脱敏。
- 不要用 `git add .`。

## 7. 结束时必须说明

最终回复至少包含：

- 修了什么层级的问题。
- 改了哪些文件。
- 跑了哪些验证命令。
- 若有未验证项，说明原因。
- 若创建 commit，给出 commit hash。
- “验证证据”段：为实际运行的 build、每组定向测试、full suite、真实 CLI、性能和生成 JSON 分别提供可点击绝对路径链接，并尽量定位到结果摘要 / 完整失败列表行。没有链接的验证必须标记未跑。
