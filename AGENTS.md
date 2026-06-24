# Agent 引导（新会话首读）

> 本文件是 agent runbook，只放当前规则、路由和默认命令。项目地图、长表和历史说明放在 `docs/`。

## 0. 项目边界

这是一个明日方舟基建效率 / 编排 / 排班引擎：

- `infra-core`：机制、搜索、编排、排班、导出所需数据结构。
- `infra-cli`：命令入口、文件加载、输出格式化、回归验证壳。
- `data/`：技能、干员实例、体系、shortcut、标准夹具等运行时真源。
- 当前架构目标：站内搜索使用清晰效率量纲；跨贸易 / 制造 / 全局注入比较统一进入 scoring / balance 公式入口。

非目标：心情排班、宿管恢复、全基建连续时间最优化。

## 1. 首读顺序

1. 本文。
2. [docs/INDEX.md](docs/INDEX.md)：文档入口、TODO / ARCHIVE 分层。
3. [docs/PROJECT_MAP.md](docs/PROJECT_MAP.md)：当前架构、模块索引、数据真源。
4. 按任务读对应文档，避免全仓库通读 Markdown。

准备实现事项看 [docs/TODO/](docs/TODO/)。完成或废弃的事项归档到 [docs/ARCHIVE/](docs/ARCHIVE/)。

`plans/` 默认是历史设计记录；除非当前文档明确引用，否则不是首读材料。

## 2. 当前主线

评分 Phase 3 公式接口已完成并归档到 [docs/ARCHIVE/done/SCORING_PHASE3.md](docs/ARCHIVE/done/SCORING_PHASE3.md)：

- `crates/infra-core/src/scoring/` 已提供 `BalanceFormulaId`、`TradeManuBalanceInput`、`BalancedEff`。
- `search/control.rs` 已通过 placeholder 公式入口维持旧裸加排序行为。
- 后续主线等待公孙长乐贸易-制造平衡公式与锚点，再进入 [docs/SCORING_REFACTOR_PLAN.md](docs/SCORING_REFACTOR_PLAN.md) Phase 5。

用户说“继续下一步”“看准备实现的功能”时，先看 [docs/TODO/README.md](docs/TODO/README.md)；若无 ready TODO，不要自行发明公式权重。

## 3. 硬规则

| 层 | 约束 |
|----|------|
| L1 | `trade/interpreter.rs`、`manufacture/interpreter.rs` 只认 `buff_id`，不认识干员名 |
| L2 | `gold_flow.rs`、`order_mechanic.rs`、`unit_output.rs` 处理机制域最优解；`atoms: []` 可表示委托，不等于缺失 |
| L3 | `shortcut.rs` + `trade_shortcuts.json` 处理固定最优 / 难 atom 化组合 |
| GL | `cross_facility/`、`global_resource/`、`control/` 处理跨设施资源与注入 |
| Scoring | 同设施内可按设施效率排序；跨贸易 / 制造 / 全局注入必须经公式入口 |
| CLI | 不写机制、公式、求解；只做命令、加载、输出、回归 |

不要为了“零 warning”破坏 API / serde / 预留机制。当前允许保留 `private_interfaces`、未来机制 `dead_code`、预留字段 warning。

## 4. 任务路由

| 任务 | 先读 |
|------|------|
| 评分 / 排序口径 | [docs/SCORING_MODEL.md](docs/SCORING_MODEL.md)、[docs/SCORING_REFACTOR_PLAN.md](docs/SCORING_REFACTOR_PLAN.md)、[docs/ARCHIVE/done/SCORING_PHASE3.md](docs/ARCHIVE/done/SCORING_PHASE3.md) |
| 编排 / 体系 / meta 组合 | [docs/ORCHESTRATION_LAYER.md](docs/ORCHESTRATION_LAYER.md)、[docs/BASE_ASSIGNMENT.md](docs/BASE_ASSIGNMENT.md) |
| CLI / 前端调用 | [docs/INFRA_CLI.md](docs/INFRA_CLI.md)、[docs/FRONTEND_CLI.md](docs/FRONTEND_CLI.md) |
| 贸易 L1/L2/L3 | [docs/EFFECT_ATOM_DESIGN.md](docs/EFFECT_ATOM_DESIGN.md)、[docs/INTERNAL/TRADE_INTERPRETER.md](docs/INTERNAL/TRADE_INTERPRETER.md)、[docs/INTERNAL/SHORTCUT_MATCHING.md](docs/INTERNAL/SHORTCUT_MATCHING.md) |
| 制造站 | [docs/MANUFACTURE_STATUS.md](docs/MANUFACTURE_STATUS.md) |
| 跨设施 global atom | [docs/INTERNAL/CROSS_FACILITY.md](docs/INTERNAL/CROSS_FACILITY.md) |
| 排班轮换 | [docs/SCHEDULE_ROTATION.md](docs/SCHEDULE_ROTATION.md) |
| 待建模 / 已建模干员 | [docs/需要完成的干员建模.md](docs/需要完成的干员建模.md)、[docs/MODELLED_OPERATORS.md](docs/MODELLED_OPERATORS.md) |

## 5. 改机制工序

改干员 / 技能 / 机制时按层走：

1. 数据层：`data/operator_instances.json`、`data/skill_table.json`、必要时 `data/trade_shortcuts.json` / `data/base_systems.json`。
2. 类型层：新 Selector / Action / Condition / Phase 先改 `types.rs`。
3. L1：贸易改 `trade/interpreter.rs`，制造改 `manufacture/interpreter.rs`，不要互套假设。
4. L2：订单分布、赤金闭环、单位产出进 `trade/order_mechanic.rs`、`gold_flow.rs`、`unit_output.rs`。
5. L3 / 编排：固定最优组合进 shortcut / `base_systems.json`，不要靠 `search` 临时发现 meta。
6. 验证：加或改测试，再按 §6 运行。
7. 文档：更新对应领域文档；准备实现事项放 `docs/TODO/`，完成后移入 `docs/ARCHIVE/done/`。

加干员前先判断：数据有了吗？L1 够吗？要不要 L2/L3？制造是否仍全池穷举？有没有混量纲排序？

## 6. 验证与默认命令

### 6.1 Cargo 输出纪律

本仓库 warning 多。跑 Cargo 时先编译落日志，只看 tail；编译通过后再运行测试 / CLI。

```powershell
New-Item -ItemType Directory -Force target/codex-logs | Out-Null

cargo test -p infra-core --no-run *> target/codex-logs/infra-core-test-build.log
Get-Content target/codex-logs/infra-core-test-build.log -Tail 80
cargo test -p infra-core --quiet

cargo build -p infra-cli *> target/codex-logs/infra-cli-build.log
Get-Content target/codex-logs/infra-cli-build.log -Tail 80
cargo run -q -p infra-cli -- verify --all
```

编译失败时先用：

```powershell
rg -n "error\[|error:|failed|panicked" target/codex-logs
```

### 6.2 用户说“跑一遍模拟”

默认理解为：全精2 练度盒 + 243 布局 + αβγ ABC 三队轮换 + 写出 MAA JSON。

```powershell
cargo run -q -p infra-cli -- plan `
  --operbox data/fixtures/243/operbox_full_e2.json `
  --maa-out out/243_maa.json
```

仅排班时：

```powershell
cargo run -q -p infra-cli -- layout team-rotation `
  --layout data/fixtures/243/layout.json `
  --operbox data/fixtures/243/operbox_full_e2.json `
  --maa-out out/243_maa.json
```

不要用 `layout rotation`（A-B-A，已废弃）或 `layout test`（单班探测）代替模拟。

### 6.3 改机制后的 smoke test

```powershell
cargo run -q -p infra-cli -- layout test `
  --layout data/fixtures/243/layout.json `
  --operbox data/fixtures/243/operbox_full_e2.json `
  --text
```

## 7. 数据与评分不变式

1. `skill_table.id` 必须等于解包 `buff_id`。
2. 干员归属只在 `operator_instances.json`。
3. 贸易站技能原文只信 `prts_trade_skills.json`。
4. `REGRESSION_CASES.csv` 的 `operators` 列未驱动夹具；按 `expect_shortcut` / `case_id` 映射。
5. `skill_table.atoms = []` 可表示“交给 L2 / L3 / 特例域处理”。
6. 贸易搜索 `score == trade_pct == order_eff_total`；`gold_pct` 单独展示。
7. 制造搜索当前按 `prod_total`；发电搜索当前按 `charge_speed_pct`。
8. 中枢已通过 scoring placeholder 入口维持 `trade_inject + manu_gold + manu_br` 临时裸加口径，等待真实公式替换。
9. 跨贸易 / 制造 / power / global 注入比较时，公式未到前只能 placeholder / TODO，不猜权重。

## 8. 不必通读

- `target/`、`out/`、release 产物、xlsx 二进制、`.venv/`。
- PRTS HTML 快照、`MECHANICS_REGISTRY.csv`。
- `docs/公孙长乐的体系分析文档/` 全部理论链，除非当前任务是体系 / 公式。
- `trade/interpreter.rs`、`manufacture/interpreter.rs`、`infra-cli/output.rs` 全文；按 `docs/INTERNAL/` 或函数名定位。
