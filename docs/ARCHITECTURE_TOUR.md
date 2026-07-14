# 架构导览：从 `plan` 到 MAA 排班

> 状态：当前实现导览，事实快照为 2026-07-14。本文只解释已经存在的调用链；历史计划和未来候选架构不属于当前运行时。

本文适合第一次追踪完整排班结果的开发者。维护或修 bug 前仍应先读 [AGENTS.md](../AGENTS.md)、[MAINTENANCE_MODE.md](MAINTENANCE_MODE.md) 和 [PROJECT_MAP.md](PROJECT_MAP.md)。如果只想理解基建业务过程而不关心代码入口，读 [GONGSUN_RUNTIME_OVERVIEW.md](GONGSUN_RUNTIME_OVERVIEW.md)。

## 1. 先分清事实来源与代码层

| 层 | 当前职责 | 主要入口 |
|---|---|---|
| 当前用户裁决 | 当前对话中明确补充或纠正的业务口径；应先同步到 Markdown | [SYSTEM_AUDIT_WORKFLOW.md](SYSTEM_AUDIT_WORKFLOW.md) |
| Markdown 真源 | 业务语义与预期行为的最高权威；代码、数据和旧测试不能反推或推翻它 | [INDEX.md](INDEX.md)、`docs/公孙长乐的体系分析文档/` |
| `data/` | 技能、干员实例、体系、shortcut、布局和回归锚点的运行时载体 | `operator_instances.json`、`skill_table.json`、`base_systems.json`、`trade_shortcuts.json` |
| L1 | 把 `buff_id` 解释为 EffectAtom 行为；解释器不认识干员名 | [EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md)、[trade/interpreter.rs](../crates/infra-core/src/trade/interpreter.rs)、[manufacture/interpreter.rs](../crates/infra-core/src/manufacture/interpreter.rs) |
| L2 | 处理不能只靠局部 atom 表达的机制域求解 | [gold_flow.rs](../crates/infra-core/src/trade/gold_flow.rs)、[order_mechanic.rs](../crates/infra-core/src/trade/order_mechanic.rs)、[unit_output.rs](../crates/infra-core/src/trade/unit_output.rs) |
| L3 | 对固定最优或难 atom 化的贸易组合做 shortcut 结算 | [shortcut.rs](../crates/infra-core/src/trade/shortcut.rs)、[segment.rs](../crates/infra-core/src/trade/segment.rs)、[INTERNAL/SHORTCUT_MATCHING.md](INTERNAL/SHORTCUT_MATCHING.md) |
| GL | 生成并消费跨设施资源、全局注入和 scope=Global atom | [control/](../crates/infra-core/src/control/)、[global_resource/](../crates/infra-core/src/global_resource/)、[cross_facility/](../crates/infra-core/src/cross_facility/)、[INTERNAL/CROSS_FACILITY.md](INTERNAL/CROSS_FACILITY.md) |
| Layout | System 选型、落位、设施补位、`used` 互斥、全基建上下文和轮换 | [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md)、[ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md)、[SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md) |
| Scoring | 各生产域保存独立直接效率；中枢局部 heuristic 使用具名 policy | [EFFICIENCY_MODEL.md](EFFICIENCY_MODEL.md)、[SCORING_MODEL.md](SCORING_MODEL.md) |
| CLI | 加载参数、调用 core、保存 JSON 和格式化输出；不实现业务公式 | [INFRA_CLI.md](INFRA_CLI.md)、[commands/plan.rs](../crates/infra-cli/src/commands/plan.rs) |

最重要的边界是：Markdown 决定“应该是什么”，`data/` 描述“运行时拿什么解释”，core 决定“如何求解”，CLI 只决定“如何调用和输出”。

## 2. 完整主线

```text
infra-cli plan
  ├─ load blueprint / operbox / instances / skill_table
  ├─ build_box_profile ───────────────────────────────→ profile JSON
  └─ schedule_team_rotation
       └─ assign_shift_with_plan_skip(Peak)
            ├─ build_plan
            └─ run_shift_pipeline
                 ├─ execute_plan
                 ├─ build facility pools
                 ├─ control / producer / dorm / power / trade / manufacture fill
                 └─ repeated resolve_base snapshots + final room snapshots
       ├─ derive actual shift binds
       ├─ split peak assignment into alpha / beta halves
       ├─ search gamma replacements
       ├─ build and score three shifts
       └─ validate rotation invariants
  └─ build_from_team_rotation ────────────────────────→ MAA JSON
```

`resolve_base` 不是只在末尾调用一次。填房阶段需要不断用当前部分编制重建跨设施上下文；三班生成后又会按每班实际人员重新结算。

## 3. `plan`：用户入口与两条输出支线

[commands/plan.rs](../crates/infra-cli/src/commands/plan.rs) 的 `plan_cmd` 是默认模拟入口：

1. 读取 `--layout`；未给出时使用默认 243 布局。
2. 读取 `--operbox`；JSON 和 xlsx 的解析最终都进入 `OperBox`。
3. 加载 `operator_instances.json` 与 `skill_table.json`。
4. 先调用 `build_box_profile`，写出账号画像 JSON。
5. 再调用 `schedule_team_rotation` 生成 alpha/beta/gamma 三队排班。
6. 如传入 `--maa-out`，把轮换报告转换成 MAA JSON。

账号画像与排班共享同一份输入，但画像不是排班求解器的一部分。画像路径见 [box_profile/build.rs](../crates/infra-core/src/box_profile/build.rs)，排班主路径从 [schedule/team_rotation.rs](../crates/infra-core/src/schedule/team_rotation.rs) 开始。

## 4. OperBox：账号拥有与练度进入求解器

[operbox/mod.rs](../crates/infra-core/src/operbox/mod.rs) 保存玩家拥有的姓名、精英阶段、等级和稀有度；[operbox/xlsx.rs](../crates/infra-core/src/operbox/xlsx.rs) 负责一图流导入。

OperBox 本身不保存完整技能公式。建池时由 `OperatorInstances` 按练度解析设施绑定和 `buff_id`：

```text
OperBoxEntry
  + operator_instances.json 的 tier/facility 绑定
  + skill_table.json 的 buff_id → atoms
  = ControlPool / TradePool / ManuPool / PowerPool entry
```

因此排查“账号有干员但没进池”时，应依次检查姓名匹配、练度 tier、设施绑定、`buff_id` 和池过滤，而不是先改搜索排序。

## 5. `build_plan`：只做 System 选型，不做效率求解

高峰班从 `schedule_team_rotation` 调用 `assign_shift_with_plan_skip`，后者在 [layout/assign.rs](../crates/infra-core/src/layout/assign.rs) 中先调用 [orchestrate/select.rs](../crates/infra-core/src/layout/orchestrate/select.rs) 的 `build_plan`。

`build_plan` 合并两种来源：

- `system_integrity/` 的代码化体系判定，例如迷迭香与红松林；
- `base_systems.json` 的 registry claim。

输出统一的 `AssignmentPlan`，包含：

- `registry_claims` 与已激活体系；
- required anchors；
- optional producers；
- 同房/禁同房等 constraints；
- degradation 信息；
- shift binds；
- 中枢搜索候选要求。

这里不调用贸易、制造或中枢 solver。required anchor、`shift_bind` 和 shortcut 也不是同一种东西：anchor 保证进编，bind 约束已进编人员的跨班关系，shortcut 只负责最终组合结算。

更详细的不变量见 [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md) 和 [ADR 0001](ADR/0001-layout-assignment-decomposition.md)。

## 6. `execute_plan` 与 `used`：把计划落成初始编制

[orchestrate/execute.rs](../crates/infra-core/src/layout/orchestrate/execute.rs) 的 `execute_plan` 接受 seed assignment 和 `AssignmentPlan`，应用 registry claims，返回：

- 已经落位的 `BaseAssignment`；
- 同步的全局 `used: HashSet<String>`。

随后 [layout/assign/pipeline.rs](../crates/infra-core/src/layout/assign/pipeline.rs) 再放置计划中的代码化 anchors，并继续使用同一份 `used`。`used` 是跨设施人员互斥的运行时事实；任何提前落位都会改变后续设施的可选池。

这也是编排 bug 不能在最下游“塞一个人”的原因：若硬核心应在 `AssignmentPlan` 表达，却只在 fill 阶段补人，其他设施已经可能抢走该干员或其队友。

## 7. Pipeline：建池、搜索、补位与反复 resolve

[layout/assign/pipeline.rs](../crates/infra-core/src/layout/assign/pipeline.rs) 是单班阶段顺序的事实源。Peak 主路径当前执行：

1. `execute_plan`，并放置代码化 anchors。
2. 从 operbox 建 control/trade/manufacture/power 四类池。
3. 把 plan tier、候选要求和搜索 anchor 注入池。
4. 比较普通高峰前缀与当前登记的可选动态贸易 producer 前缀。
5. 每个前缀依次处理中枢、体系 producer、宿舍 producer、发电和贸易；阶段间通过 `resolve_snapshot` 获取新上下文。
6. 只让胜出的前缀进入制造站填充。
7. 对最终制造、贸易房间刷新效率快照。

Recovery 有独立顺序，不能用 Peak 的假设推断。各设施的具体补位代码位于 `layout/assign/*_fill.rs`。

搜索层负责枚举候选并按本域口径排序：

- 贸易：[search/trade.rs](../crates/infra-core/src/search/trade.rs)；
- 制造：[search/manufacture.rs](../crates/infra-core/src/search/manufacture.rs)；
- 中枢：[search/control.rs](../crates/infra-core/src/search/control.rs)；
- 发电：[search/power.rs](../crates/infra-core/src/search/power.rs)。

`top_k` 是保留/返回候选的上限，不应被误解为业务规则。完整池与 standalone 池的安全边界见 [PERFORMANCE_ENGINEERING.md](PERFORMANCE_ENGINEERING.md)。

## 8. `resolve_base`：从人员名单构造全基建上下文

[layout/resolve.rs](../crates/infra-core/src/layout/resolve.rs) 的 `resolve_base` 接受 blueprint 与当前 assignment：

1. `WorkforceIndex::build` 建立按房间和设施划分的人员索引。
2. 从蓝图写入设施数、设施等级、宿舍规划、配方种类和初始全局资源。
3. 写入全基建人员名单、跨设施 faction/tag 计数。
4. 依次应用 power、control 和 office 对 `LayoutContext` 的影响。
5. 收集并执行 scope=Global atom。
6. 执行全局资源 conversions。
7. 为贸易、制造和发电房间构造带各自 layout 快照的 resolved room input。

`resolve_base` 负责上下文与房间输入，不替代各房 solver。贸易搜索或最终刷新仍会把 `ResolvedTradeRoom` 交给 `solve_trade_with_shift`；制造同理。

## 9. 单房求解：L1 → L2 → L3

贸易房最终进入 [trade/solver.rs](../crates/infra-core/src/trade/solver.rs)：

```text
TradeRoomInput
  → interpreter：按 Phase 执行 Condition / Selector / Action，并处理 gold_flow 状态
  → 金单尝试 L3 segment + shortcut
       ├─ 命中：shortcut 构造 order mechanic
       └─ 未命中：L2 order_mechanic 正常求解
  → unit_output
  → TradeResult.final_efficiency
```

L3 不是候选选型器。它只在真实三人组和 producer manifest 已确定后判断是否命中社区锚点。贸易内部函数导航见 [INTERNAL/TRADE_INTERPRETER.md](INTERNAL/TRADE_INTERPRETER.md) 与 [INTERNAL/SHORTCUT_MATCHING.md](INTERNAL/SHORTCUT_MATCHING.md)。

制造站使用自己的 interpreter/solver，不复用贸易假设，见 [MANUFACTURE_STATUS.md](MANUFACTURE_STATUS.md)。

## 10. Scoring：生产效率分域，中枢 heuristic 具名

生产域没有匿名总分：

- 贸易按 `final_efficiency`；
- 制造按 `final_efficiency`；
- 发电按 `final_efficiency`；
- 三班日报分别汇总 trade/manufacture/power，不相互混加。

中枢搜索当前使用 `ControlInjectRawSumV0` 局部 policy。它是对贸易/制造注入分量的具名排序 heuristic，不是全基建产出函数，也不能冒充贸易最终效率。数值定义见 [EFFICIENCY_MODEL.md](EFFICIENCY_MODEL.md) 与 [SCORING_MODEL.md](SCORING_MODEL.md)。

## 11. `team_rotation`：从一份高峰编制生成三班

[schedule/team_rotation.rs](../crates/infra-core/src/schedule/team_rotation.rs) 当前采用 12h + 6h + 6h 的 alpha/beta/gamma 模型：

1. 先通过完整单班 pipeline 得到 peak assignment 与 peak plan。
2. 根据实际 peak 编制派生 shift binds。
3. 提取宿舍/办公室共享脚手架，并重新 resolve 共享 layout。
4. 把 peak 生产设施切成 alpha/beta 两个半区。
5. 从剩余池构建 gamma 对两个半区的替补。
6. 按每班活跃队伍重建中枢和各生产房间。
7. 对三班实际 assignment 重新评分，处理菲亚梅塔回岗。
8. final validator 显式校验中枢 5 人、bind 次数/共同 presence 与可选动态 producer presence；生产房容量和人员互斥继续由构造、`used` 与回归验收。
9. 分别汇总三类按时长加权的 daily totals。

轮换不等于连续心情全局最优化；当前范围和导出契约见 [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md)。

## 12. MAA：导出已经求解的排班，不重新选人

[export/maa.rs](../crates/infra-core/src/export/maa.rs) 的 `build_from_team_rotation` 遍历三班：

- 把每班 `BaseAssignment` 映射到 MAA 房间槽位；
- 写入班次名、时长说明、休息队和无人机默认配置；
- 使用 rotation 已确认的菲亚梅塔动作；
- 输出 `MaaSchedule`，由 CLI 保存到 `--maa-out`。

MAA 导出层不运行机制公式，也不应该修正上游非法编制。若 JSON 人员不对，应先检查 rotation assignment；若 assignment 正确但字段形状不对，才检查 exporter。

## 13. 按症状定位

| 症状 | 先看 |
|---|---|
| 干员未进入可选池 | OperBox → instances tier → pool builder |
| 体系未激活或硬核心缺失 | `build_plan`、`AssignmentPlan`、对应体系 Markdown |
| 干员被别的设施抢走 | `execute_plan`、pipeline 阶段顺序、`used` |
| 单房效率错误 | room input → L1/L2/L3；不要先改 CLI |
| 多房选择不理想 | facility fill、role/候选过滤、搜索排序与上下文刷新 |
| 三班同上同下或轮休错误 | actual shift binds、team rotation invariant validation |
| MAA 字段错误但 assignment 正确 | `export/maa.rs` |

维护期的完整复现、修改门禁和验证留痕要求见 [MAINTENANCE_MODE.md](MAINTENANCE_MODE.md)。
