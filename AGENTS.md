# Agent 引导（正常维护 / bug 修复期首读）

> 本项目已进入正常维护期。默认工作模式是“复现 bug、定位边界、最小修复、补回归、保持口径稳定”，不是继续推进架构计划。

## 0. 当前状态

这是一个明日方舟基建效率 / 编排 / 排班引擎：

- `infra-core`：机制解释、搜索、编排、排班、导出数据结构。
- `infra-cli`：命令入口、文件加载、输出格式化、回归验证壳。
- `data/`：技能、干员实例、体系、shortcut、标准夹具等运行时真源。
- `docs/`：当前事实、维护期流程、任务路由和历史归档。

现阶段普通问题的默认目标是稳定维护，不是扩张。除非用户明确要求新增功能，否则不要主动推进 `docs/TODO/` 里的历史 Phase 计划。

2026-07-03 用户确认“90 → 95 质量提升”方案过度设计；该方向已暂停作为默认主线。同日用户确认 `feedback/` 本批线上反馈 bug 已修复，项目进入正常维护期。当前没有默认主动 TODO 队列；[`docs/TODO/TRUST_RECOVERY_PLAN.md`](docs/TODO/TRUST_RECOVERY_PLAN.md) 与 [`feedback/TRACKING.md`](feedback/TRACKING.md) 只作为维护参考、关闭审计和防回归矩阵使用。[`docs/TODO/QUALITY_90_TO_95_PLAN.md`](docs/TODO/QUALITY_90_TO_95_PLAN.md) 只作为历史参考，除非用户明确要求恢复。

非目标仍然是：心情排班、宿管恢复、全基建连续时间最优化。

## 1. 首读顺序

1. 本文。
2. [docs/MAINTENANCE_MODE.md](docs/MAINTENANCE_MODE.md)：维护期 bug 修复流程、分层定位、验收矩阵。
3. [docs/INDEX.md](docs/INDEX.md)：文档入口和任务路由。
4. [docs/PROJECT_MAP.md](docs/PROJECT_MAP.md)：当前代码地图、模块边界、数据真源。
5. 按 bug 类型读取对应领域文档；不要全仓库通读 Markdown。

`plans/` 和 `docs/TODO/` 默认是历史建设期材料。只有用户明确要求继续某个 TODO，或 bug 定位需要理解当时设计，才读取它们。

## 2. 维护期默认动作

用户报告 bug / “结果不对” / “跑一下看看”时，按以下顺序：

1. 记录输入：命令、layout、operbox、assignment、期望值、实际值。
2. 先复现：优先用现有 CLI 入口，不在代码里临时拼新路径。
3. 缩小层级：CLI 参数/输出 → layout 编排 → search 排序 → 单站 solver → L1/L2/L3 机制 → data。
4. 最小修复：只改 bug 所在层；不顺手重构、不重开 Phase 计划。
5. 加回归：能落 CSV/fixture 就落 CSV/fixture；排班 bug 优先保留最小 layout + operbox 或 debug bundle。
6. 验证：跑与改动半径匹配的命令，说明未跑的原因。
7. Git：只提交本轮 agent 自己改的文件。

如果无法复现，不要猜公式；先给出已跑命令、输入差异和下一步需要的最小材料。

### 2.1 禁止补丁式修复

“最小修复”指修正最小的**责任边界**，不是在最下游增加一个能让当前样例通过的条件分支。体系 / 编排 bug 必须先恢复领域不变量，再决定改动位置。

#### 修改前强制门禁

体系 / 编排 bug 在编辑任何代码、数据或测试前，Agent 必须先在对话中向用户输出以下审计；四项未完成时不得开始修改：

1. **领域不变量**：逐条列出用户确认的硬核心、可选 producer、同房 / 跨站 / 在基建内条件、互斥关系、班次绑定和降级关闭条件。
2. **违规位置**：指出当前代码在哪个生命周期阶段违反每条不变量，并给出具体文件 / 类型 / 函数；不得只说“逻辑有问题”。
3. **单一责任边界**：说明修复后由哪个 System / `AssignmentPlan` 字段 / role filter / solver 边界唯一保证该不变量。
4. **删除清单**：列出将删除或改写的错误旧逻辑、错误注释和错误测试；不能只叠加新逻辑而保留冲突路径。

用户明确要求直接实现时，这个门禁仍然有效：先用简短 commentary 给出审计，再开始编辑。若规则仍有歧义，必须停在审计阶段向用户确认，不得自行选择一种解释并写代码。

#### 真源优先级

1. 用户在当前对话明确确认的口径最高。
2. 其次是当前领域真源：技能条件查 `data/MECHANICS_REGISTRY.csv`，体系边界查 `docs/公孙长乐的体系分析文档/` 对应文档，数据归属查运行时 JSON 真源。
3. 当前代码、代码注释、旧测试和历史输出只能用于定位实现，不能反推业务规则。
4. 当旧代码 / 注释 / 测试与用户口径或领域真源冲突时，必须明确报告冲突并删除或改写旧语义；禁止为了“保持测试通过”保留错误模型。

#### 实现纪律

1. 先写出用户确认的体系不变量：硬核心、可选 producer、同房 / 跨站 / 在基建内条件、互斥关系、班次绑定、降级关闭条件。
2. 对照机制真源和体系文档；不要从当前错误代码反推规则。
3. 沿完整生命周期定位不变量在哪一步丢失：`select -> plan -> execute -> fill -> resolve -> rotation -> export`。必须区分：
   - “体系已激活”是否保证硬核心实际进编；
   - `shift_bind` 只约束已进编干员，不能代替 required anchor；
   - shortcut 只负责组合结算，不能代替体系选型或进编约束；
   - `used` / 提前固定落位不得让后续搜索失去本应可选的队友。
4. 若同一规则需要在 pipeline、role、rotation 分别追加特殊判断，说明抽象边界仍错；停止叠补丁，回到 `AssignmentPlan` / System schema / 领域候选约束重建单一事实源。
5. 禁止用以下方式假装修好：
   - 为一个 operbox、room id、班次下标或当前 top hit 写特判；
   - 看到缺人就在下游强塞干员，却不修体系硬核心声明；
   - 用 `shift_bind`、tag、priority 或 shortcut 间接期待某干员“自然入选”；
   - 绕过正常 role 搜索，导致另一体系的硬约束失效；
   - 只改文档或只改测试期待，使错误结果变成“通过”。
6. 回归必须覆盖不变量，而非只钉最终快照：至少包括激活、关闭、进编、禁止同房 / 允许跨站、轮换绑定，并保留一个完整 `plan` 或 `team-rotation` 端到端用例。
7. 修复说明必须回答三件事：根因在哪一层、旧模型为什么允许错误状态、现在由哪个单一边界保证不再发生。不能只描述新增了哪个 `if`。

#### 完成前强制证明

实现完成后，Agent 必须逐条提供下表信息；缺任一列不得宣称 bug 已修复：

| 项 | 必须说明 |
|----|----------|
| 不变量 | 用户确认的原句或等价精确定义 |
| 代码保证 | 唯一负责保证它的类型 / 字段 / 函数 |
| 删除的冲突 | 被移除的旧路径、旧 fallback、旧特判或旧测试 |
| 回归 | 对应测试名与断言内容 |
| 端到端结果 | 用户实际 CLI 路径中的房间 / 队伍 / 关键字段 |

最终回复还必须单独列出：

1. 根因所在层以及旧模型为什么允许非法状态。
2. 新的单一事实源，禁止只罗列修改文件。
3. 未通过的测试，并区分本轮回归、既有失败和未验证风险。
4. 实际运行过的用户入口；只跑单元测试时不得声称排班 bug 已完整修复。

## 3. 硬边界

| 层 | 规则 |
|----|------|
| 数据 | `skill_table.id` 必须等于解包 `buff_id`；干员归属只在 `operator_instances.json` |
| L1 | `trade/interpreter.rs`、`manufacture/interpreter.rs` 只认 `buff_id`，不认识干员名 |
| L2 | `gold_flow.rs`、`order_mechanic.rs`、`unit_output.rs` 处理机制域最优解；`atoms: []` 可表示委托 |
| L3 | `shortcut.rs` + `trade_shortcuts.json` 处理固定最优 / 难 atom 化组合 |
| GL | `cross_facility/`、`global_resource/`、`control/` 处理跨设施资源与注入 |
| Layout | `build_plan -> execute_plan -> pipeline -> resolve_base` 是主路径；不要绕过它修排班 |
| Scoring | 生产域统一用 `Efficiency` 直接小数效率并保留贸易 / 制造 / power 分量；global 局部 heuristic 需要命名 policy |
| CLI | 不写机制、公式、求解；只做命令、加载、输出、回归 |

不要为了“零 warning”破坏 API / serde / 预留机制。当前允许保留 `private_interfaces`、未来机制 `dead_code`、预留字段 warning。

### 3.1 已确认的体系不变量

以下口径是维护期硬约束。修改迷迭香、贸易 core role、三队轮换前，必须读取 [`docs/公孙长乐的体系分析文档/ROSEMARY_PERCEPTION_CHAIN.md`](docs/公孙长乐的体系分析文档/ROSEMARY_PERCEPTION_CHAIN.md) 和对应 `MECHANICS_REGISTRY.csv` 行。

#### 迷迭香感知体系

1. 迷迭香 E2 与黑键 E2 是缺一不可的硬核心；缺黑键时体系整体关闭，不得降级成“只有迷迭香”。
2. 体系激活后，peak 编制必须同时包含迷迭香和黑键。`shift_bind` 不能充当黑键进编保证。
3. 三队轮换中迷迭香与黑键必须同队、上 2 休 1。
4. 黑键必须位于非巫恋贸易站；黑键与但书 / 可露希尔的具体搭配由体系候选与效率搜索决定，不在这里额外钉死房号或固定三人组。

#### 龙巫自动编排

1. 自动编排的龙巫站必须包含巫恋、龙舌兰，以及一名持有 `tailor_alpha` 或 `tailor_beta` 的裁缝技能干员。
2. 非裁缝技能持有者不得作为自动编排龙巫站第三人；不得用普通白板、贝洛内或其他高效率散件替代裁缝位。
3. 历史 `gsl_witch_*_blank` 若为单站结算兼容而保留，不得进入自动编排 `witch` role 的候选集合。

#### 叙拉古跨站语义

1. 八幡海铃、伺夜、贝洛内按跨站机制建模；伺夜与贝洛内不要求同站。
2. `MECHANICS_REGISTRY.csv` 中“同一个贸易站”“在基建内”“每个进驻在贸易站的叙拉古干员”是三种不同作用域，不得合并成固定同房组合。
3. 搜索自然把伺夜、贝洛内放在同站是允许的；编排层不得把同站写成体系激活前提。

## 4. Bug 路由

| 现象 | 先读 | 优先入口 |
|------|------|----------|
| `plan` / MAA / 三队轮换不对 | [docs/MAINTENANCE_MODE.md](docs/MAINTENANCE_MODE.md)、[docs/SCHEDULE_ROTATION.md](docs/SCHEDULE_ROTATION.md)、[docs/INFRA_CLI.md](docs/INFRA_CLI.md) | `cargo run -q -p infra-cli -- plan ...` |
| 单班布局结果不对 | [docs/BASE_ASSIGNMENT.md](docs/BASE_ASSIGNMENT.md)、[docs/ADR/0001-layout-assignment-decomposition.md](docs/ADR/0001-layout-assignment-decomposition.md) | `layout test` / `layout eval` |
| 贸易站效率 / shortcut 不对 | [docs/EFFECT_ATOM_DESIGN.md](docs/EFFECT_ATOM_DESIGN.md)、[docs/INTERNAL/TRADE_INTERPRETER.md](docs/INTERNAL/TRADE_INTERPRETER.md)、[docs/INTERNAL/SHORTCUT_MATCHING.md](docs/INTERNAL/SHORTCUT_MATCHING.md) | `verify --case` / `trade yield` |
| 制造站效率不对 | [docs/MANUFACTURE_STATUS.md](docs/MANUFACTURE_STATUS.md) | `layout test` / `bench --recipe` |
| 中枢 / 全局注入 / 木天蓼不对 | [docs/INTERNAL/CROSS_FACILITY.md](docs/INTERNAL/CROSS_FACILITY.md)、[docs/SCORING_MODEL.md](docs/SCORING_MODEL.md) | `layout eval` / targeted core test |
| CLI / 前端调用问题 | [docs/INFRA_CLI.md](docs/INFRA_CLI.md)、[docs/FRONTEND_CLI.md](docs/FRONTEND_CLI.md)、[docs/FRONTEND_SERVE_GUIDE.md](docs/FRONTEND_SERVE_GUIDE.md) | 复用用户命令 |
| 数据缺漏 / 干员建模 | [docs/MODELLED_OPERATORS.md](docs/MODELLED_OPERATORS.md)、[docs/需要完成的干员建模.md](docs/需要完成的干员建模.md) | pool / verify / targeted fixture |

## 5. 改动策略

### 5.1 机制 bug

1. 先确认数据是否已有：`operator_instances.json`、`skill_table.json`、必要时 `trade_shortcuts.json` / `base_systems.json`。
2. 能用现有 `EffectAtom` 表达时只改数据。
3. 新 Selector / Action / Condition / Phase 才改 `types.rs`。
4. L1 真错才改 interpreter；贸易和制造不要互套假设。
5. 订单分布、赤金闭环、单位产出进 L2；固定最优组合进 L3 shortcut。
6. 增加或更新回归锚点。

### 5.2 排班 / 编排 bug

1. 先用 `plan` 或 `layout team-rotation` 复现用户路径。
2. 单班问题用 `layout test`；指定编制结算用 `layout eval`。
3. 只在以下层级中修对应问题：
   - 体系选型：`layout/orchestrate/select.rs`、`layout/system.rs`、`data/base_systems.json`
   - 落位语义：`layout/orchestrate/execute.rs`、`layout/assign/pipeline.rs`
   - 设施补位：`layout/assign/*_fill.rs`
   - 三队轮换：`schedule/team_rotation.rs`、`schedule/shift_bind.rs`
   - MAA 导出：`export/maa.rs`
4. 不为单一 bug 泛化 ADR 决策 D 的 execute_plan 三态；只有出现第二个数据驱动、且需要 anchor + 搜索半固定的体系时再抽象。
5. 若 bug 涉及体系硬核心缺失、required anchor 被降成 bind/tag、或多个 role 争夺同一贸易站，不能只改 `*_fill.rs`；应先修 `System / AssignmentPlan` 对硬核心和约束的表达，再让 fill 消费计划。

### 5.3 评分 / 输出 bug

1. 先判断是核心结果错，还是 CLI 展示错。
2. 核心分量错改 `infra-core`；列名、文本、JSON 形状错改 `infra-cli/output.rs`。
3. 不新增匿名综合权重。
4. 现有排序口径：
   - 贸易搜索：`final_efficiency`
   - 制造搜索：`final_efficiency`
   - 发电搜索：`final_efficiency`
   - 中枢搜索：`ControlInjectRawSumV0`，即 `trade_inject + manu_gold + manu_br` 的局部 heuristic

## 6. 默认命令

本仓库 warning 多。跑 Cargo 时先编译落日志，只看 tail；编译通过后再运行测试 / CLI。

```bash
mkdir -p target/codex-logs

cargo test -p infra-core --no-run > target/codex-logs/infra-core-test-build.log 2>&1
tail -80 target/codex-logs/infra-core-test-build.log
cargo test -p infra-core --quiet

cargo build -p infra-cli > target/codex-logs/infra-cli-build.log 2>&1
tail -80 target/codex-logs/infra-cli-build.log
cargo run -q -p infra-cli -- verify --all
```

编译失败时：

```bash
rg -n "error\\[|error:|failed|panicked" target/codex-logs
```

### 用户说“跑一遍模拟”

默认理解为：全精2 练度盒 + 243 布局 + 账号分析 + αβγ ABC 三队轮换 + 写出 MAA JSON。

```bash
cargo run -q -p infra-cli -- plan \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --maa-out out/243_maa.json
```

仅排班时：

```bash
cargo run -q -p infra-cli -- layout team-rotation \
  --layout data/fixtures/243/layout.json \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --maa-out out/243_maa.json
```

不要用 `layout test`（单班探测）代替模拟；A-B-A 入口已移除。

### 改机制后的 smoke test

```bash
cargo run -q -p infra-cli -- layout test \
  --layout data/fixtures/243/layout.json \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --text
```

## 7. Git 协作默认

用户希望 agent 主动整理 Git，而不是只把改动留在工作区。除非用户明确说“不要动 git / 不要提交”：

1. 开始和结束都看 `git status --short`。
2. 只 stage / commit 本轮 agent 自己改的文件；不要用 `git add .`。
3. 若本轮改动和既有用户改动在同一文件内交织，不能可靠拆分时不要强行提交。
4. 验证通过，或验证未跑但原因已说明时，若本轮改动形成独立单元，默认创建简短 commit。
5. 不自动 `amend`、`rebase`、`reset`、`checkout --` 或清理未跟踪文件。

### 7.1 Rust 格式化口径

1. `rustfmt` 的输出是本仓库 Rust 代码的规范格式；修改 Rust 后运行 `cargo fmt --all`。
2. 保留 `rustfmt` 产生的换行、缩进与 import / re-export 排序，不要为了缩小 diff 手动改回格式化前的单行写法。
3. 若一次格式化触及多个既有文件，也应保留格式化结果；可在 Git 中单独整理格式化提交，但不要反向手工排版。

## 8. 不必通读

- `target/`、`out/`、release 产物、xlsx 二进制、`.venv/`。
- PRTS HTML 快照。`MECHANICS_REGISTRY.csv` 不需日常全量阅读，但同房 / 跨站 / 在基建内条件存在争议时必须按干员或技能定向读取。
- `plans/` 与 `docs/TODO/` 历史计划，除非用户明确要求继续对应事项。
- `docs/公孙长乐的体系分析文档/` 全部理论链，除非当前 bug 是体系 / 等效效率锚点。
- `trade/interpreter.rs`、`manufacture/interpreter.rs`、`infra-cli/output.rs` 全文；优先按内部地图或函数名定位。
