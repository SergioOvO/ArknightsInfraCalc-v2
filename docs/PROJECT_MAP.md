# 项目地图（Agent / 开发者入门）

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/文档生命周期.md；docs/ORCHESTRATION_LAYER.md；docs/INFRA_CLI.md
> 摘要：提供当前代码、数据和命令 owner 地图

> 本文是当前代码、数据和命令地图，不是每个 Agent 任务的无条件首读。先由 [AGENTS.md](../AGENTS.md) 选择 Skill；只有 owner、入口或调用链不明时才定向读取本文。领域语义见对应 canonical Markdown，文档位置未知时查 [INDEX.md](INDEX.md)。

如果读者懂基建体系但不关心代码入口，先看 [GONGSUN_RUNTIME_OVERVIEW.md](GONGSUN_RUNTIME_OVERVIEW.md)。

项目同时进行 debug、文档一致性重建、feature 和独立 quality-refactor。改动大小由当前不变量的真实责任边界决定；debug 见 [MAINTENANCE_MODE.md](MAINTENANCE_MODE.md)，任务路由见 [INDEX.md](INDEX.md)。

## 项目是什么

明日方舟**基建**效率求解器（v2 绿场重写）。给定干员练度（operbox）与场景假设（布局、产线数、全局资源等），计算同房三人组的贸易/制造效率、机制等效效率、单位产出，并支持**全基建单班进驻编制**（`assign_base_greedy`）、当前 **Team A/B/C + Shift 1/2/3** 轮换（`schedule_team_rotation`）以及穷举搜索。旧版 A-B-A 已从 CLI 与 core API 移除，见 [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md)。

**当前范围**：
- **贸易站**：主力完成（L1+L2+L3+回归齐全）
- **制造站**：L1+搜索+bench（无 L2/L3）
- **控制中枢**：`search_control_combos` + 全局注入 + 心情/公招补位策略
- **发电站**：`search_power_assignment`（充能 + 虚拟发电折算）
- **全基建宏观排班**：`assign_shift`（编排 `build_plan` → `execute_plan`）已落地（`layout test` / `plan` / `team-rotation` 默认调用）
- **box_profile**：练度分析工具（`plan` / `layout analyze`）
- **编排层**：`orchestration_rules.json` 声明贸易核心、迷迭香、自动化、红松、莱茵；`base_systems.json` 仅承载尚未迁移的兼容 registry；`cross_facility` 结算 global atom

### 各域落地状态

| 域 | L1 | L2 | L3 | 搜索 | 排班 | CLI 回归 | 说明 |
|----|----|----|-----|------|------|----------|------|
| **贸易站** | ✅ | ✅ | ✅ | ✅ | ✅ Team ABC / Shift 1–3 | ✅ | 主力；A-B-A 已移除 |
| **制造站** | ✅ | — | — | ✅ | ✅ Shift 1–3（含产线拆解） | — | 见 [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md) |
| **控制中枢** | ✅ | — | — | ✅ `search_control_combos` | ✅ 宏观排班内 | — | `control/`：木天蓼 producer、精2 全局注入、公招/心情补位 |
| **发电站** | ✅ | — | — | ✅ `search_power_assignment` | ✅ 宏观排班内 | — | 充能 + 虚拟发电折算（晨曦等） |
| **全局资源** | 注册表 ✅ | 池 ✅ | — | — | — | — | P0：木天蓼 / 人间烟火（简化）/ 魔物料理（基准注入） |

域详情：**制造** → [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md)；**全局资源** → [EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md) §8.13；**评分口径/分量化策略** → [SCORING_MODEL.md](SCORING_MODEL.md)。历史迁移计划见 [ARCHIVE/plans/SCORING_REFACTOR_PLAN.md](ARCHIVE/plans/SCORING_REFACTOR_PLAN.md)。

**当前边界**：mood 内核与 peak 主力最长工作时间已接入；宿管分配、按 ETA 改写短班、
全基建连续心情最优化仍由后续上层规划器负责。历史设计见 [mood_eta_design.md](ARCHIVE/plans/mood_eta_design.md)，不得作为当前实现说明。

**全基建单班进驻编制**（`assign_shift` → `build_plan` / `execute_plan`、并行搜 + `used` 顺序落位）：现行见 **[BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md)**；声明式规则由 `layout/orchestrate/rules.rs` 把 `data/orchestration_rules.json` 编译成完全解析的 `AssignmentPlan`，兼容 registry 在主规则与 late competitive rule 之间执行。

---

## 求解流水线（一眼看懂）

```
operbox / roster + operator_instances + skill_table
        ↓
  pool（可建模干员池，C(n,3) 组合基数）
        ↓
  search（穷举三人组，rayon 并行） 或  schedule（当前 Shift 1–3 逐站）
        ↓
  solve_trade_with_shift（单站核心）
        ├─ L1 interpreter   Phase 排序 → Selector/Condition/Action
        ├─ L2 gold_flow     赤金虚拟线链（进驻顺序状态机）
        ├─ L2 order_mechanic 订单分布 → 机制等效效率
        └─ L3 shortcut      trade_shortcuts.json 表化最优解（热路径 + 回归锚点）
        ↓
  unit_output             单位贸易量 / 无人机 / 日产量
```

**关键约定**：L1 代码**不认识干员名**，只认 `buff_id`。L2/L3 可按 `buff_id` 或组合类型分支。`skill_table` 中 `atoms: []` 表示**委托给域引擎**，不是未建模。

---

## 仓库目录

```
ArknightsInfraCalc-v2/
├── Cargo.toml              workspace：infra-core + infra-cli
├── README.md
├── AGENTS.md               Agent / 新会话首读（链到本文）
├── .agents/skills/         项目 Skills：debug / feature / quality / system audit / evidence / training review
├── docs/
│   ├── INDEX.md            文档总入口：首读、TODO、归档、任务路由
│   ├── PROJECT_MAP.md      ← 本文：当前架构地图
│   ├── GONGSUN_RUNTIME_OVERVIEW.md 给基建策略作者看的运行流程说明
│   ├── EFFECT_ATOM_DESIGN.md   机制词汇、已建模干员、分层求解定稿
│   ├── MANUFACTURE_STATUS.md   制造站域状态（勿按贸易站假设改）
│   ├── BASE_ASSIGNMENT.md      全基建进驻编制（宏观排班）设计
│   ├── ORCHESTRATION_LAYER.md  编排层 System → Plan → Execute（Phase 0–3/5 已落地）
│   ├── FRONTEND_CLI.md         前端集成：`plan`、MAA JSON、layout-gen
│   ├── SCHEDULE_ROTATION.md    Team ABC / Shift 1–3 轮换现行契约
│   ├── SYSTEM_CHAINS.md        谜迭香/自动化/红松林/莱茵 体系链参考
│   ├── INFRA_CLI.md            infra-cli 模块职责与改动边界
│   ├── MAINTENANCE_MODE.md     Debug / conformance 修复流程、回归与验收矩阵
│   ├── TODO/                   未实施提案；仅在当前 feature/quality 任务明确恢复后执行
│   ├── ARCHIVE/                已完成 / 废弃 / 历史材料
│   └── INTERNAL/               大文件内部地图（interpreter / shortcut）
├── crates/
│   ├── infra-core/         库：类型、解释器、求解、搜索、排班、编排
│   └── infra-cli/          命令行：plan / advice / verify / pool / search / trade / bench / bake / serve / layout / profile
├── data/                   机制注册表、干员实例、规则与回归用例（运行时实现载体）
├── scripts/                数据工具与 Codex 证据 / 范围检查器
├── docs/ARCHIVE/plans/     历史设计；开放工作位于 docs/TODO/
└── release/                发布产物（layout-gen、fixtures）
```

---

## `infra-core` 模块索引

| 模块 | 文件 | 职责 |
|------|------|------|
| **types** | `src/types.rs` | `Phase` / `Selector` / `Action` / `Condition` / `EffectAtom` / `SkillDef` — JSON 与解释器共享的类型 |
| **tier** | `src/tier.rs` | `PromotionTier`（精0 / 精1+） |
| **layout/tier** | `src/layout/tier.rs` | `OperatorTier`（`CrossStation` / `SameStation` / `Standalone`）— 三层分配优先级 |
| **skill_table** | `src/skill_table.rs` | 加载 `data/skill_table.json`；`data_path()` / `workspace_root()` |
| **instances** | `src/instances.rs` | `operator_instances.json`；`resolve_buff_ids`（含 stepwise 技能）；`buff_stem` |
| **roster** | `src/roster.rs` | 贸易站干员名单 CSV（`roster.csv` 等），按设施过滤 |
| **operbox** | `src/operbox/mod.rs`、`operbox/xlsx.rs` | 玩家练度盒 JSON / 一图流 xlsx 导入 |
| **training_advice** | `src/training_advice/` | 练卡推荐：加载 `training_recommendations.json` v2，按 operbox 输出 `now` / `conditional` / `blocked` / `ready` / `review`；`rag.rs` 生成受限伪 RAG 输入 → [练卡推荐规则](练卡推荐规则.md) |
| **error** | `src/error.rs` | 统一 `Error` / `Result` |
| **pool** | `src/pool/trade.rs`、`pool/manufacture.rs`、`pool/control.rs`、`pool/power.rs`、`pool/base.rs` | 设施可求解池；泛型 `PoolCore<T>` 消除结构体重复 |
| **search** | `src/search/trade.rs`、`search/manufacture.rs`、`search/control.rs`、`search/power.rs`、`search/role_pick.rs` | C(n,k) 穷举 + 评分；中枢/发电搜索 |
| **schedule** | `src/schedule/team_rotation.rs`、`schedule/shift_bind.rs`、`schedule/base_rotation.rs` | **Team ABC / Shift 1–3 轮换**；`base_rotation.rs` 只保留逐房直接效率结算 → [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md) |
| **control** | `src/control/` | 中枢 `solve_control` → `apply_control_to_layout` 写回 layout 全局注入 |
| **global_resource** | `src/global_resource/` | `GlobalResourceKey`、`REGISTRY`、`CONVERSIONS`、`GlobalResourcePool`、`GlobalInjectManifest` |
| **layout** | `src/layout/` | `BaseBlueprint` / `BaseAssignment` / `resolve_base` / `assign_shift` / `orchestrate/` / `system.rs` |
| **manufacture** | 见 [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md) | 制造 L1 + 求解 + 搜索（无 L2/L3） |
| **trade** | 见下表 | 贸易站求解核心 |
| **cross_facility** | `src/cross_facility/` | 跨设施编排；收集并执行 scope=Global atom，统一注入全局资源池 |
| **box_profile** | `src/box_profile/` | 练度概况分析（`layout analyze` 子命令） |
| **export** | `src/export/` | MAA 排班 JSON 导出 |
| **office** | `src/office/`、`src/support_facility.rs` | 办公室显式编制静态求值；既有感知 producer 路径保持独立 |
| **meeting** | `src/meeting/`、`src/support_facility.rs` | 会客室显式编制静态求值；概率、线索事件和交流状态不算分 |
| **power** | `src/power/` | 发电站求解 `solve_power` + `apply_power_to_layout` |
| **eff_ramp** | `src/eff_ramp.rs` | 时间爬升效率（芬/克洛丝等） |

### `trade/` 子模块

| 文件 | 层级 | 职责 |
|------|------|------|
| `input.rs` | — | `TradeOperator` / `TradeRoomInput`；`TradeLayoutContext` 是 `LayoutContext` 兼容别名（优先读 `layout/context.rs`） |
| `interpreter.rs` | L1 | `TradeContext`、`apply_trade_phases`；按 Phase 执行 EffectAtom；挂钩 `gold_flow`。**内部地图**：[INTERNAL/TRADE_INTERPRETER.md](INTERNAL/TRADE_INTERPRETER.md) |
| `gold_flow.rs` | L2 | `apply_gold_flow_chain`：绮良/鸿雪/图耶等虚拟赤金线累加 |
| `order_mechanic.rs` | L2 | 订单 tag/分布 → `mechanic_equiv_eff_pct`（违约、裁缝、龙舌兰等） |
| `shortcut.rs` | L3 | 加载 `trade_shortcuts.json`；但书 solo / 巫恋组 / 可露希尔 / 黑键 / 推王 / 企鹅分档；同房互斥校验。**内部地图**：[INTERNAL/SHORTCUT_MATCHING.md](INTERNAL/SHORTCUT_MATCHING.md) |
| `segment.rs` | — | 链段注册表 `trade_segments.json`（docus_syracusa / blackkey_closure / vina_lungmen / penguin_*） |
| `solver.rs` | — | **`solve_trade` / `solve_trade_with_shift`**：串联 L1→L2→L3，输出 `TradeResult` |
| `unit_output.rs` | — | 单位产出、无人机折算、`TradeDailyYield` |

**改机制时最常打开的顺序**：`types.rs`（新词汇）→ `skill_table.json` → `interpreter.rs` 或 L2 引擎 → `shortcut.rs` / `trade_shortcuts.json` → `solver` 测试 → `REGRESSION_CASES.csv`。

---

## `layout/` 子模块

| 文件 | 职责 |
|------|------|
| `blueprint.rs` | `BaseBlueprint`、`RoomBlueprint`、`FacilityKind`、`RoomProduct`；`trade_station_scenario`、`manu_line_scenario` |
| `assignment.rs` | `BaseAssignment`、`AssignedOperator`、`RoomAssignment` |
| `assign.rs` | **`assign_shift` / `assign_shift_with_plan`**：编排 `build_plan` → `execute_plan`；贸易余站贪心 |
| `orchestrate/` | **`AssignmentPlan`**、通用 rules compiler、registry 兼容汇合、`execute_plan`（Rule → resolved Plan → Execute；executor 不按体系名分派） |
| `resolve.rs` | `resolve_base`：蓝图+编制 → `ResolvedBase`；集成 `cross_facility` global 池 |
| `context.rs` | `LayoutContext`、`SharedLayout`、`DEFAULT_DORM_OCCUPANT_COUNT` |
| `shift.rs` | `AssignShiftMode`（`Peak` / `Recovery`） |
| `system.rs` | `base_systems.json` 解析（含 `tier` 字段）与兼容 helper；主路径落位在 `orchestrate/execute.rs` |
| `tier.rs` | `OperatorTier` 枚举：`CrossStation` / `SameStation` / `Standalone`（三层分配优先级） |
| `workforce.rs` | `WorkforceIndex`、杜林计数 tag |

---

## `infra-cli` 命令

| 命令 | 用途 |
|------|------|
| **`plan`** | **用户主入口 / Agent 默认模拟入口**：账号画像 JSON + Team ABC / Shift 1–3 排班 + MAA；`--operbox` 支持 JSON/xlsx；布局默认 243 |
| `advice --operbox <path>` | **练卡推荐**：加载 `data/training_recommendations.json` v2，按 operbox 输出结构化建议包（`now`/`conditional`/`blocked`/`ready`/`review`）；`--explain` 包装确定性事实骨架与仓库内 Markdown 片段；`--rules` 可换规则文件；`--pretty` 美化 JSON。领域真源：[练卡推荐规则](练卡推荐规则.md) |
| `verify --case <id>` / `--all` | 跑 `REGRESSION_CASES.csv` + `UNIT_OUTPUT_ANCHORS.csv` |
| `pool [--trade] [--manufacture]` | 打印贸易 / 制造池统计与跳过原因；至少选择一类，制造池需要 operbox |
| `search trade [--roster] [--top N]` | 全池 C(n,3) 搜索 Top-K |
| `bench --operbox <path>` | 243c 基准布局 + operbox 贸易/制造搜索（**无**怪猎木天蓼；怪猎号见下） |
| `bake [all\|trade\|manufacture]` / `bake validate` | 生成或校验本地 Bake 加速表 |
| `serve` | 启动前端 JSON line 常驻 worker |
| **`layout test`** | **自定义 `BaseBlueprint` + operbox（默认 `assign_base_greedy` 宏观排班）** |
| **`layout team-rotation`** | **Team ABC / Shift 1–3 轮换（含 MAA 导出）— 仅排班入口** |
| **`layout analyze`** | **练度 box profile 分析（对比基线）** |
| **`layout eval`** | **评估指定编制各房间效率** |
| `profile layout-full` / `profile analyze-compare` | CLI 性能画像 / 分析链路对比辅助 |
| `trade yield <fixture> [--level] [--shift]` | 单站产量探测（fixture 名见 `verify/fixtures.rs` 的 `unit_fixture`） |

**Agent 默认夹具**（无用户路径时）：`data/fixtures/243/layout.json` + `data/fixtures/243/operbox_full_e2.json`。具体复现和证据参数见 [Debug 指南](MAINTENANCE_MODE.md)。

**用户说「跑一遍模拟」**：默认跑 `plan`（账号分析 + Team ABC / Shift 1–3 排班）；仅排班时才用 `layout team-rotation`。具体任务专属输出参数见 [Debug 指南](MAINTENANCE_MODE.md) 和 [INFRA_CLI.md](INFRA_CLI.md)「跑一遍模拟」。

**CLI 任务按需读取**：[INFRA_CLI.md](INFRA_CLI.md) 约定 `commands` / `verify` / `output` 分工，避免把机制或夹具塞回 `main.rs`。

### `infra-cli` 源码索引

| 路径 | 职责 |
|------|------|
| `src/main.rs` | 进程入口、子命令路由；`pool` / `search` / `trade` / `bench` 编排（部分暂留 `main.rs`） |
| `src/commands/advice.rs` | `advice`：加载 v2 练卡规则 + operbox；默认输出 `TrainingAdviceReport`，`--explain` 输出 report + `TrainingAdviceRagInput` |
| `src/commands/bake.rs` | `bake`：生成或校验加速表 |
| `src/commands/plan.rs` | **`plan`**：box profile + `schedule_team_rotation` + MAA |
| `src/commands/layout.rs` | `layout test` / `team-rotation` / `analyze` / `eval` 全部子命令 |
| `src/commands/profile.rs` | `profile layout-full` / `profile analyze-compare`：CLI 性能画像与分析链路对比 |
| `src/commands/serve.rs` | `serve`：前端 JSON line 常驻协议 |
| `src/commands/verify.rs` | `verify` 子命令：遍历 CSV、断言、PASS/FAIL |
| `src/verify/cases.rs` | 加载 `REGRESSION_CASES.csv`、`UNIT_OUTPUT_ANCHORS.csv` |
| `src/verify/fixtures.rs` | 硬编码 `TradeRoomInput`（回归 + `trade yield`） |
| `src/output.rs` | CSV/文本/JSON 输出；**不含求解** |

回归夹具在 `verify/fixtures.rs`；CSV `operators` 列尚未驱动夹具选择（按 `expect_shortcut` / `case_id` 映射）。

### 回归夹具映射（`verify_cmd`）

| `expect_shortcut` / 条件 | 夹具函数 | 典型 case |
|--------------------------|----------|-----------|
| `gsl_witch_*` | `witch_fixture(shortcut_id, level)` | 巫恋核各档 |
| `case_id contains ling_jie` + `expect_shortcut=none` | `ling_jie_fixture` | 灵知 + 孑 + 银灰 + 琳琅诗怀雅；L1 自然 129，shortcut=None |
| `gsl_docus_*` | `docus_fixture(case_id, level)` | 但书三人组 |
| `expect_shortcut` 为 closure 且已接线 | `closure_fixture(case_id, level)` | `reg_gsl_closure_tier90` 等 |
| 其他 | — | 打印 `fixture not wired` 并 skip |

`trade yield <fixture>` 用 `unit_fixture`：`closure_solo` / `docus_solo` / `witch_long_beta` 等（见 `fixtures.rs`）。

`UNIT_OUTPUT_ANCHORS.csv` 的 `fixture` 列驱动 `unit_fixture` 名。

---

## `data/` 文件职责

下列文件是运行时实现载体和核对材料，不裁决领域业务语义；业务规则仍以用户当前裁决和对应领域 Markdown 为准。

| 文件 | 角色 | 维护方式 |
|------|------|--------|
| **`skill_table.json`** | `buff_id` → EffectAtom 列表；空 `atoms` = 委托 L2 | Agent / 维护者（业务变更需用户确认） |
| **`operator_instances.json`** | `干员@tier_0` / `干员@tier_up` → `buff_ids` 的运行时归属映射 | Agent / 维护者 |
| **`trade_shortcuts.json`** | L3 组合表化最优解 + verify / reference 锚点；`gsl_ling_jie_yaxin` 仅参考，不 active 匹配 | 双方 |
| **`trade_segments.json`** | 链段注册表（docus_syracusa / blackkey_closure / vina_lungmen / penguin_*）+ 贸易 core role fallback 链（docus / closure / witch） | 双方 |
| **`orchestration_rules.json`** | 声明式有限 alternatives：贸易核心、迷迭香、自动化、红松、莱茵；gate/role/relation/工作状态/轮换依赖由同一编译器消费 | 手工（Markdown 为业务真源） |
| **`base_systems.json`** | 尚未迁移体系的兼容 registry；字段含 `tier`、priority、`exclusive_group`、slots；不得再加入复杂降级体系 | 脚本 + 手工 |
| **`REGRESSION_CASES.csv`** | CLI `verify` 用例：期望最终效率、机制等效效率与 `rule_id` | 双方 |
| **`UNIT_OUTPUT_ANCHORS.csv`** | 单位产出 / GSL 赤金锚点 | 双方 |
| **`prts_trade_skills.json`** / `.csv` / `_table.html` | PRTS 贸易站技能原文快照（核对用） | 脚本抓取 |
| **`prts_manufacturing_skills.json`** / `.csv` / `_table.html` | PRTS 制造技能原文快照 | 脚本抓取 |
| **`roster.csv`** | 默认贸易站搜索名单 | 脚本 / 手工 |
| **`roster_gongsun.csv`** / **`roster.csv`** | 公孙长乐等扩展名单 | 导入 |
| **`data/fixtures/243/`** | **243 标准测试样例**：`layout.json` + `operbox_full_e2.json` + `schedule_export.json` | 夹具 |
| **`data/fixtures/training_advice/`** | 练卡推荐场景夹具：`witch_only_tequila` / `witch_ready_untrained` / `closure_partial` / `standalone_e1_four_star` / `all_ready` | 夹具 |
| **`training_recommendations.json`** | 练卡推荐规则表 v2（`system`/`combo`/`standalone`/`soft_combo`）；人工维护，非 solver 候选池 | 手工 |
| **`operbox_gongsun.json`** | 练度盒样例（较小子集） | 脚本 / 测试 |
| **`data/layout/243_use_this_.json`** | 公孙 243 事实蓝图（2 金贸）；同 `fixtures/243/layout.json` | 模板 |
| **`data/layout/243c.json`** | 旧版 243c（3 贸易：2 金 + 1 源石）；怪猎 `snhunt` 等同结构 | 模板 |
| **`data/layout/snhunt.json`** | 怪猎评估蓝图（物理同 243c；木天蓼靠中枢编制） | 模板 |
| **`MECHANICS_REGISTRY.csv`** | 全基建机制归档（727 条）；贸易站核对**不再依赖** | 归档 |
| **`需要完成的干员建模.md`** | 未完全建模干员清单与近期落地记录 | 维护 |
| **`data/tags/*.csv`** | 干员 tag（阵营等），供 Selector 引用 | 数据 |
| **`data/box_profile_knightcode.json`** | 练度分析基准 profile（`layout analyze --baseline`） | 导出 |

---

## `scripts/` 脚本

| 脚本 | 用途 |
|------|------|
| `build_skill_table.py` | 构建/校验 skill_table；pilot 干员硬失败 |
| `check_trade_roster.py` | roster 与 skill_table / instances 一致性 |
| `audit_trade_buffs.py` | buff 覆盖审计 |
| `audit_trade_skill_rename.py` | 技能重命名审计 |
| `merge_closure_skills.py` | 合并技能表片段 |
| `build_roster_from_operbox.py` | 从 operbox 生成 roster |
| `inspect_xlsx_operators.py` | 练度表 xlsx → operbox JSON |
| `build_base_systems_from_gongsun_xlsx.py` | 从工具人表 xlsx 生成 `base_systems.json` |
| `build_243_schedule_fixture.py` | 构建 243 排班夹具 |
| `fix_trade_test_literals.py` | 测试字面量修复辅助 |
| `build_manufacturing_skill_table.py` | 制造技能表构建/校验 |
| `build_power_skill_table.py` | 发电技能表构建/校验 |
| `audit_control_buffs.py` | 中枢 buff 审计 |
| `audit_tier_mapping.py` | tier 映射审计 |
| `render_training_recommendations.py` | 将 v2 练卡规则表投影为公孙长乐中文验收稿（规则验收，非账号过滤结果） |
| `migrate_training_recommendations_v2.py` | 历史 v1→v2 机械迁移工具；日常维护直接改 v2 JSON |
| `codex/run_evidence.sh` | 统一执行验证命令并原子追加任务 manifest |
| `codex/compare_test_failures.py` | 比较 Cargo full-suite 失败名称集合 |
| `codex/render_evidence.py` | 校验 manifest / status / 日志 / 产物并生成证据 Markdown |
| `codex/check_task_scope.py` | 检查实际 diff、范围扩展和 deferred side findings |
| `codex/check_repository_facts.py` | CI 检查稳定 Markdown 链接、状态字段和 CLI 命令地图 |

---

## 测试在哪里

- **单元测试**：各模块 `mod tests`（`cargo test -p infra-core`），重点在 `interpreter`、`solver`、`shortcut`、`gold_flow`、`pool`、`schedule`、`layout/assign`。
- **回归**：`infra-cli verify` + CSV；夹具 JSON 在 `tests/fixtures/`。
- **不变式**：`skill_table.id` 必须等于解包 `buff_id`；原文只信 `prts_trade_skills.json`。

---

## 常见任务 → 打开哪里

| 任务 | 先看 | 可能还要改 |
|------|------|------------|
| 新增/修改干员技能 | `EFFECT_ATOM_DESIGN.md` §四、`prts_trade_skills.json` | `skill_table.json`、`operator_instances.json`、L1/L2 引擎 |
| 新 Selector/Action | `types.rs` | `interpreter.rs`、设计文档 §三 |
| 赤金线链式机制 | `gold_flow.rs` | `skill_table` 空 atoms 注册 |
| 订单违约/裁缝/特别订单 | `order_mechanic.rs` | `shortcut.rs`、`trade_shortcuts.json` |
| 组合表化（巫恋/可露希尔档） | `trade_shortcuts.json` | `shortcut.rs`、`REGRESSION_CASES.csv`、`verify/fixtures.rs` |
| 搜索变慢/评分不对 | `search/trade.rs` | `solver.rs` 的 score 逻辑 |
| Team ABC / Shift 1–3 | `schedule/team_rotation.rs`、`schedule/shift_bind.rs` | `operbox` 数据、`TradeSearchOptions` |
| 编排层 / 体系认领 | [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md) | `layout/orchestrate/`、`data/orchestration_rules.json`、兼容 `data/base_systems.json` |
| 全基建单班进驻编制 | [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md) | `layout/assign.rs`、`layout/orchestrate/` |
| 宏观排班/中枢搜索 | `layout/assign.rs`、`search/control.rs` | `assign.rs` 的 `assign_control` / `assign_dorm_producers` |
| Team ABC / Shift 1–3 | `schedule/team_rotation.rs` | `export/maa.rs` 导出 |
| 当前轮换（含制造/发电） | `schedule/team_rotation.rs` | `layout team-rotation` |
| 单位产出/无人机 | `unit_output.rs` | `UNIT_OUTPUT_ANCHORS.csv`、`verify/fixtures.rs` |
| 改 CLI 结构 / 回归夹具放哪 | [INFRA_CLI.md](INFRA_CLI.md) | 勿在 `main.rs` 堆新夹具 |
| L1 Phase / Condition 局部改 | [INTERNAL/TRADE_INTERPRETER.md](INTERNAL/TRADE_INTERPRETER.md) | 通读 interpreter 全文 |
| L3 匹配 / 同房互斥 | [INTERNAL/SHORTCUT_MATCHING.md](INTERNAL/SHORTCUT_MATCHING.md) | 只改 JSON 不改 matcher |
| 制造站搜索 / 池 | [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md) | `trade/shortcut.rs` |
| 中枢 / 全局资源 | `control/`、`global_resource/`、`layout/resolve.rs` | `EFFECT_ATOM_DESIGN.md` §4.8–4.12、§8.13 |
| 怪猎账号 / 木天蓼链 | `snhunt_baseline()`、`data/layout/snhunt.json` | 泰拉调查团、火龙S黑角、麒麟R夜刀 |
| 宿舍人数链（黑键/乌有/铎铃） | `DEFAULT_DORM_OCCUPANT_COUNT`、`layout/resolve.rs` | `EFFECT_ATOM_DESIGN.md` §4.8–4.10 |
| 导入玩家练度 | `operbox/mod.rs`、`operbox/xlsx.rs`、`inspect_xlsx_operators.py` | operbox JSON |
| 数据一致性报错 | `check_trade_roster.py`、`instances.rs` | roster / instances / skill_table |
| 练度概况分析（box profile） | `box_profile/` | `plan` / `layout analyze` CLI |
| **练卡推荐 / advice / 该练谁** | [练卡推荐规则](练卡推荐规则.md)、`training_advice/`、`data/training_recommendations.json` | `advice` CLI；夹具 `data/fixtures/training_advice/`；验收 skill `gongsun-training-review`；开放项 [练卡推荐规则表剩余人工验收](TODO/练卡推荐规则表剩余人工验收.md) |
| 练卡规则表人工验收 | `gongsun-training-review` + `render_training_recommendations.py` | 只改规则/canonical，不改 solver 候选池 |
| 前端集成 / 发布包 | [FRONTEND_CLI.md](FRONTEND_CLI.md)、`release/README.md` | `plan`、`--maa-out` |
| MAA 排班导出 | `export/maa.rs` | `plan` 或 `layout team-rotation --maa-out` |

---

## Agent 渐进式读取

1. 先读 [AGENTS.md](../AGENTS.md) 并选择对应项目 Skill。
2. Skill 指定的 canonical 领域文档必须完整读取；位置未知时才查 [INDEX.md](INDEX.md)。
3. 代码 owner、入口或调用链未知时，从本文对应表格定位，不要求通读全文。
4. 改干员时读 [需要完成的干员建模.md](需要完成的干员建模.md) 和对应领域文档。
5. 改求解时从相应 `solver.rs` / `search/*.rs` 入口进入，再按需用 [INTERNAL/](INTERNAL/) 定位大文件函数段。
6. 改制造读取 [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md)；改数据只读相关 JSON 条目和其 canonical 规则。

**不必通读**：`interpreter.rs` / `output.rs` 全文（用 INTERNAL 地图或 `emit_*` 名定位）、`MECHANICS_REGISTRY.csv`、PRTS HTML 快照、xlsx 练度表二进制。

---

## 依赖与技术栈

- Rust 2021 workspace，`serde`/`serde_json`/`csv`/`rayon`/`thiserror`/`chrono`
- 无数据库、无 Web 服务；纯本地计算 + JSON/CSV 数据
- 旧仓库 `ArknightsInfraCalc - 副本` 仅归档参考，**不迁移**其 Rust 求解器

---

## 相关文档

| 文档 | 内容 |
|------|------|
| [EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md) | EffectAtom 模型、词汇表、分层求解概要、全局资源注册表 |
| [MODELLED_OPERATORS.md](MODELLED_OPERATORS.md) | 已建模干员索引（从 EFFECT_ATOM_DESIGN.md §4 抽出） |
| [SYSTEM_CHAINS.md](SYSTEM_CHAINS.md) | 谜迭香/自动化/红松林/莱茵 四大体系链参考手册 |
| [INDEX.md](INDEX.md) | 渐进式文档路由、任务 Skill、TODO / ARCHIVE 分层 |
| [MAINTENANCE_MODE.md](MAINTENANCE_MODE.md) | Debug / conformance 修复流程、分层定位、回归与验收矩阵 |
| [TODO/](TODO/) | 未实施提案；由当前 feature/quality 任务显式恢复 |
| [ARCHIVE/](ARCHIVE/) | 已完成、废弃或历史文档 |
| [INFRA_CLI.md](INFRA_CLI.md) | CLI 分层原则、`commands` / `verify` / `output` 职责 |
| [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md) | 制造站实现范围与缺口 |
| [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md) | 全基建单班进驻编制设计（已落地） |
| [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md) | 编排层 System / Plan / Execute（Phase 0–3/5 已落地；剩余 Phase 默认冻结） |
| [FRONTEND_CLI.md](FRONTEND_CLI.md) | 前端集成：`plan`、MAA JSON、layout-gen |
| [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md) | Team ABC / Shift 1–3 轮换与现行入口 |
| [INTERNAL/](INTERNAL/) | `interpreter` / `shortcut` 大文件内部地图 |
| [AGENTS.md](../AGENTS.md) | Agent 新会话首读、任务分类、全局硬门禁和 Skill 路由 |
| [README.md](../README.md) | 项目原则摘要与快速命令 |
