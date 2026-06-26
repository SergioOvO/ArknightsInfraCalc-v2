# ADR 0001: layout assignment 编排拆分

> 状态：accepted
> 日期：2026-06-26
> 关联文档：[../ORCHESTRATION_LAYER.md](../ORCHESTRATION_LAYER.md)、[../BASE_ASSIGNMENT.md](../BASE_ASSIGNMENT.md)、[../TODO/SYSTEM_REGISTRY_NORMALIZATION_REPORT.md](../TODO/SYSTEM_REGISTRY_NORMALIZATION_REPORT.md)

## 背景

`crates/infra-core/src/layout/assign.rs` 当前是全基建单班与轮换填房的事实入口。它已经调用 `layout/orchestrate::{build_plan, execute_plan}`，但主流程和大量策略仍集中在一个文件中：

- `assign_shift_with_plan_skip` 串联 seed、registry plan、producer、resolve、建池、发电、贸易、制造。
- 中枢补位、宿舍/办公室 producer、感知 producer、深巡/乌尔比安宿舍锚点都在同一层。
- 贸易核心优先通过 `pick_trade_meta_then_plain` 调 `search/role_pick.rs`，但还保留 `skip_trade_core_registry_systems` 跳过旧 registry 抢站条目。
- 制造存在公孙金线固定锚点、候选池扩展、容量兜底等局部策略。
- `assign_team_producer_rooms`、`assign_team_gamma_half` 等轮换半区填充函数也放在同一文件。

这不是机制层错乱。L1/L2/L3 求解、`resolve_base`、`search/*` 的职责基本清楚。问题是 **assignment orchestration facade 过胖**：宏观流水线、设施填房 policy、producer policy、轮换填充 helper、提交/快照工具混在一起，导致后续新增体系时容易继续把特例塞回 `assign.rs`。

## 决策

保留现有 `assign_base_greedy` / `assign_shift` / `assign_shift_with_plan` 公开 API，不做大重写，不引入全局联合最优。将 `layout/assign.rs` 拆为一个薄 facade 和若干按职责命名的子模块。

目标模块结构：

```text
crates/infra-core/src/layout/assign/
  mod.rs              # public facade: assign_base_greedy / assign_shift / result/options
  pipeline.rs         # 单班流水线：build plan -> execute -> producer -> resolve -> fillers
  context.rs          # AssignmentRun / FillContext：blueprint、options、used、durin、layout 快照
  commit.rs           # commit room、names_disjoint、efficiency snapshot
  control_fill.rs     # 中枢补位
  producer_fill.rs    # 宿舍/办公室/global producer 的临时落位入口
  trade_fill.rs       # 贸易余站、role priority、恢复班孑站
  manufacture_fill.rs # 制造产线、候选池、容量兜底、当前公孙金线锚点
  power_fill.rs       # 发电站填充
  team_fill.rs        # αβγ 半区填充 helper
```

`layout/orchestrate/` 保持为 registry plan 层，不吸收设施搜索逻辑：

```text
layout/orchestrate/
  plan/select/execute # 只处理 System 选型与 fixed/bond/pick_one 落位
```

## 职责边界

| 职责 | 目标归属 | 说明 |
|------|----------|------|
| 公开单班 API | `layout::assign::mod` | 保持调用方稳定 |
| 单班执行顺序 | `assign/pipeline.rs` | 只描述阶段顺序和 resolve 时机 |
| `used`、layout 快照、durin 计数 | `assign/context.rs` | 避免参数列表继续膨胀 |
| registry system 认领 | `layout/orchestrate/` | 不调贸易/制造/发电 search |
| 中枢补位 | `assign/control_fill.rs` | 允许调用 `search_control_combos` |
| producer 临时落位 | `assign/producer_fill.rs` | 感知/宿舍 producer 先集中，后续迁 global policy |
| 贸易余站 | `assign/trade_fill.rs` | 允许调用 `search/role_pick.rs` 与贸易搜索 |
| 制造余站 | `assign/manufacture_fill.rs` | 允许调用制造搜索；公孙金线先搬迁后语义化 |
| 发电余站 | `assign/power_fill.rs` | 允许调用发电搜索 |
| 轮换半区填房 | `assign/team_fill.rs` | 被 `schedule/team_rotation.rs` 调用 |
| 提交房间与效率快照 | `assign/commit.rs` | 贸易/制造/发电共用 |

## 非目标

- 不把单班排班改成全局最优、整数规划、模拟退火或 `C(n,3)^站数`。
- 不在本次拆分中改变贸易/制造/发电搜索评分。
- 不把 `resolve_base` 逻辑搬进 assign。
- 不为了拆文件同步清理所有 global hardcode；语义迁移分阶段做。
- 不改变 CLI 输出和现有 `layout test` / `plan` 行为。

## 迁移方案

### Phase 1：机械拆文件，行为不变

动作：

1. 将 `assign.rs` 改为 `layout/assign/mod.rs`，保留 public API 与 re-export。
2. 抽出 `commit.rs`，包含 `commit_operators_to_room`、`commit_trade_room`、`commit_manu_room`、snapshot helper、disjoint helper。
3. 抽出 `power_fill.rs`，包含 `assign_power_stations`、`assign_power_rooms`、`filter_power_pool`、power snapshot。
4. 抽出 `trade_fill.rs`，包含 `assign_trade_remainder`、`assign_trade_jie_remainder`、`pick_trade_meta_then_plain`、贸易 options。
5. 抽出 `manufacture_fill.rs`，包含制造候选池、固定金线锚点、capacity fallback、制造 options。
6. 抽出 `control_fill.rs`、`producer_fill.rs`、`team_fill.rs`。

验收：

```powershell
New-Item -ItemType Directory -Force target/codex-logs | Out-Null
cargo test -p infra-core --no-run *> target/codex-logs/infra-core-test-build.log
Get-Content target/codex-logs/infra-core-test-build.log -Tail 80
cargo test -p infra-core --quiet
cargo run -q -p infra-cli -- plan `
  --operbox data/fixtures/243/operbox_full_e2.json `
  --maa-out out/243_maa.json
```

Phase 1 完成条件：测试与默认模拟行为等价；没有策略语义变化。

### Phase 2：引入运行上下文，收敛参数

新增内部结构：

```rust
pub(crate) struct AssignmentRun<'a> {
    blueprint: &'a BaseBlueprint,
    instances: &'a OperatorInstances,
    table: &'a SkillTable,
    options: &'a AssignBaseOptions,
    durin_plan: Option<u32>,
    assignment: BaseAssignment,
    used: HashSet<String>,
}
```

`AssignmentRun` 只用于 assign 内部，提供：

- `resolve_snapshot(...)`
- `build_pools(...)`
- `mark_used(...)`
- `is_room_empty(...)`

规则：

- 不把 `AssignmentRun` 暴露到 `layout/orchestrate`、`search` 或 CLI。
- 不把机制公式塞进 `AssignmentRun`。
- 先替换参数最长、重复最多的路径：trade/manu/power/team。

### Phase 3：语义策略迁移

在文件拆清后，再逐项迁移策略归属：

| 当前位置 | 目标方向 | 备注 |
|----------|----------|------|
| `skip_trade_core_registry_systems` | 删除或缩成兼容列表 | 前提是 `base_systems.json` 不再把 core priority 当 fixed registry |
| `pick_trade_meta_then_plain` | `search/role_pick.rs` 或 role policy facade | `assign/trade_fill.rs` 只决定“填哪个房”，不维护 role 链细节 |
| 公孙金线固定锚点 | manufacturing policy / production-line system | 先保留行为，再决定是否进 registry |
| 感知 producer 硬编码 | `scope=global` atom + `cross_facility` | 与 Phase 4 global effect 收拢一致 |
| 深巡/乌尔比安宿舍锚点 | global/producer policy | 当前可集中在 `producer_fill.rs`，避免散落 |

## 期望结果

拆完后，阅读入口应变为：

1. 看 `layout/assign/mod.rs` 知道 public API。
2. 看 `assign/pipeline.rs` 知道单班顺序。
3. 改贸易排班只进 `assign/trade_fill.rs` 或 `search/role_pick.rs`。
4. 改制造排班只进 `assign/manufacture_fill.rs`。
5. 改 producer/global 迁移只进 `assign/producer_fill.rs`、`cross_facility/`、`resolve.rs`。
6. 改 registry 体系只进 `layout/orchestrate/` 与 `data/base_systems.json`。

`assign.rs` 不再成为所有新特例的默认落点。

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 机械拆分时误改行为 | Phase 1 不改逻辑；每次只搬一类函数并跑 `infra-core` 测试 |
| 私有 helper 可见性膨胀 | 子模块统一 `pub(crate)`，不对 crate 外暴露 |
| 循环依赖 | `commit.rs` 只依赖 assignment/pool hit 类型，不反调 fill 模块 |
| 测试仍堆在 `mod.rs` | 先保留；后续按模块迁移测试 |
| 文档与实现再次漂移 | `ORCHESTRATION_LAYER.md` 保持语义路线，ADR 只记录拆分决策 |

## 后续记录

若未来决定把 `assign/` 再拆成独立 crate 或引入 declarative policy registry，需要新 ADR。当前决策只覆盖 `infra-core::layout` 内部模块边界。
