# 术语表

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/EFFECT_ATOM_DESIGN.md；docs/EFFICIENCY_MODEL.md；docs/ORCHESTRATION_LAYER.md；docs/排班模式.md
> 复核触发：crates/infra-core/src/types.rs；crates/infra-core/src/layout/**；crates/infra-core/src/schedule/**
> 摘要：路由项目术语到各 canonical owner
> 源摘要：c854818367db9037b953b37a9d36c80452fda6c484051c72b114cad1060677f8
> 文档摘要：77f66f042906b3a5d6f7168f594198305b86b880099b52d09b17a54380dc8f06
> 复核原因：source-change
> 复核结论：updated
> 稳定事实：路由项目术语到各 canonical owner
> 证据引用：tracked:docs/GLOSSARY.md

> 实现快照：Current
> 读者：所有文档读者与协作者
> 本文负责：统一术语；不裁决具体体系业务规则

本文统一 README、领域文档、JSON 和 Rust 代码中的常用词。中文描述业务含义，反引号内保留实际类型、字段或 ID。

## 输入、布局与运行时对象

| 术语 | 含义 |
|------|------|
| 练度盒 / operbox | 玩家实际拥有的干员及其精英化、等级、潜能等信息。代码类型为 `OperBox`；它决定候选是否存在以及解锁哪一档技能。 |
| roster | 为某个设施构造搜索池时使用的干员名单。它可以来自 operbox，也可以来自维护用 CSV；不是最终上岗名单。 |
| 工具人池 / standalone roster | 在明确适用的领域中用于缩小自由搜索空间的高价值散件目录。它是性能边界，不是技能语义真源，也不适用于普通排班制造的 `full_pool` 搜索。 |
| `BaseBlueprint` / 蓝图 | 房间结构：`room_id`、设施类型、等级、容量、配方、宿舍参数等。它描述“有哪些岗位”，不描述“谁上岗”。 |
| `BaseAssignment` / 编制 | `room_id → operators` 的实际落位结果。同一班内，同一干员至多出现一次。 |
| `RoomAssignment` | `BaseAssignment` 中的一间房及其成员、可选效率快照。 |
| `LayoutContext` | 从蓝图和当前编制派生的全基建只读上下文：房间数量、workforce、标签计数、全局资源、中枢注入等。 |
| `SharedLayout` | `Arc<LayoutContext>` 的共享别名；搜索热路径复用同一份只读布局，避免重复复制。 |
| `ResolvedBase` | `resolve_base` 的完整结果，包含派生布局与各设施的 resolved room input；生产房真实结果仍由对应 search / solver 结算。 |
| `WorkforceIndex` | 按房间和设施索引当前进驻成员，并向 `LayoutContext` 写入 workforce、阵营计数和跨设施统计。 |
| workforce | 当前实际在基建或某类设施中上岗的成员集合。`base_workforce`、`trade_workforce`、`control_workforce` 等作用域不同。 |
| `room_id` | 蓝图、编制、输出和 MAA 映射共同使用的稳定房间标识；不能把当前样例的房号写成业务公式。 |

## 练度、技能与声明式机制

| 术语 | 含义 |
|------|------|
| `PromotionTier` | 技能解锁档：运行时主要区分 `tier_0` 与 `tier_up`。它不是编排优先级。 |
| `OperatorTier` | 编排候选层级：`CrossStation`、`SameStation`、`Standalone`。它不是精英化等级。 |
| `buff_id` | 游戏解包技能 ID，也是 `skill_table.json` 的结构键。`skill_table.id` 必须与它一致。 |
| `operator_instances.json` | 干员、练度档、设施和 `buff_id` 的唯一归属表。L1 不应自行识别干员名。 |
| `skill_table.json` | `buff_id → SkillDef / EffectAtom[]` 的运行时机制表。 |
| `EffectAtom` | 一个平坦机制单元，由 selector、action、可选 condition、phase、phase order、tag 和 scope 组成。 |
| `Selector` | atom 的取数方式，例如订单上限、宿舍人数、同房标签人数或全贸易 workforce 数量。 |
| `Action` | atom 的操作，例如增加效率、修改订单上限、写入状态池或注入制造效率。 |
| `Condition` | atom 的触发门禁，例如搭档同房、某人在基建内、心情达到阈值。 |
| `Phase` | atom 的执行阶段。它把状态写入、固定效率、上限、变量效率、吸收和订单机制排成稳定顺序。 |
| `phase_order` | 同一 Phase 内的稳定次序；不能用 JSON 偶然排列代替。 |
| `AtomScope::Room` | atom 只在当前房间解释。 |
| `AtomScope::Global` | atom 由 `cross_facility` 收集，在全基建上下文执行。 |
| `atoms: []` | 技能已登记但委托给 L2/L3 或其他领域引擎；不自动表示“未建模”。 |
| tag | 对干员阵营、技能类别、订单类型或 atom family 的结构化标记。不同文件中的 tag 作用域可能不同。 |

## L1、L2、L3 与跨设施机制

| 术语 | 含义 |
|------|------|
| L1 / interpreter | 按 Phase 解释通用 EffectAtom。贸易和制造解释器只认 `buff_id`，不按干员名写公式。 |
| L2 / 域引擎 | 处理难以由独立 atom 正确表达的领域机制，例如 `gold_flow`、订单分布和单位产出。 |
| L3 / shortcut | 根据实际组合和上下文命中表化规则，输出社区单位产出或固定最优档。它负责结算，不负责强制进编。 |
| shortcut | `trade_shortcuts.json` 中的组合结算条目。通常带稳定 `rule_id`、匹配条件和单位产出信息。 |
| segment / 链段 | `trade_segments.json` 中 producer 前提、consumer 类型与 shortcut 的连接。它可以被 role pick step 使用。 |
| `rule_id` | 本次 L3 或社区规则命中的稳定标识，供输出和回归审计；`None` 表示未命中 active shortcut。 |
| producer | 在一个设施或阶段生产资源、注入或前提的成员，例如中枢产生贸易注入、宿舍生产全局资源。producer 不一定是硬核心。 |
| consumer | 读取 producer 结果的房间或成员。consumer 与 producer 可以同房、跨站或只要求同时在基建内，必须按领域语义区分。 |
| `GlobalResourcePool` | 感知信息、人间烟火、木天蓼、魔物料理、虚拟发电等具名全局资源的运行时池。 |
| `GlobalInjectManifest` | 中枢等设施向贸易 / 制造写入的全局效率规则和 producer gate。它与可消耗资源池不是同一个对象。 |
| `cross_facility` | 收集所有 `scope=Global` atom，执行状态生产与转化，并生成全基建资源快照的统一编排器。 |
| 全局转化 / conversion | 资源之间的具名转换，例如梦境、记忆碎片到感知。共享读取资源不应被误当成一次性扣减。 |

## 搜索、约束与编排

| 术语 | 含义 |
|------|------|
| pool / 候选池 | 按设施、练度、技能和当前约束构造的可搜索成员集合。 |
| candidate / 候选 | 尚未提交的合法成员组合或体系方案。候选可以带 role、效率分解和规则 ID。 |
| hit / search hit | 已由单房 solver 求值的候选结果，例如 `TradeSearchHit`。 |
| `top_k` | 搜索报告保留的前 K 个结果。它是候选报告边界，不应被当作硬业务规则。 |
| 自然搜索 | 在满足硬约束后的合法空间内，按实际 solver 结果选择成员，不用名字顺序、人工固定分或样例特判强塞。 |
| 硬约束 / invariant | 任何合法结果都必须满足的规则，例如 required anchor、同房互斥、中枢满 5、同上同下。 |
| System | 可被编排层选择的一套跨站或同站结构，来自 `base_systems.json` 或代码化完整性规则。 |
| `exclusive_group` | 多个 System 之间的互斥组；选中一个后，同组其他方案不能同时认领。 |
| `AssignmentPlan` | System 选型后的可执行计划，保存 anchors、producers、constraints、候选要求和 `shift_bind`。 |
| select | 从 registry 和当前 operbox 中选择合法 System 的生命周期阶段。 |
| execute | 把 plan 中的 fixed、bond、anchor 等确定位置写入编制和 `used`。 |
| fill | 对尚未确定的中枢、贸易、制造、发电位置执行领域搜索。 |
| resolve | 从当前蓝图和编制重新计算 workforce、全局资源、注入和房间结果。 |
| anchor | 提前声明的落位核心。一般 anchor 只固定必要成员，其余位置仍由搜索补齐。 |
| required anchor | 体系激活后必须实际进编的 anchor。tag、priority 或 `shift_bind` 不能代替它。 |
| bond | 明确要求同房的固定核心关系，通常固定 A+B，再让搜索选择第三人。 |
| role | 贸易等领域的核心优先 / fallback 策略，例如 `docus`、`closure`、`witch`。role 选择候选边界，房内组合仍由 solver 排序。 |
| core priority | “优先保留某个核心，再搜索队友”的策略，不等于固定三人组。 |
| degradation / 降级 | 体系缺少可选成员时，按文档允许的较弱档继续运行。硬核心缺失时若文档要求关闭，就不能自行降级。 |
| fallback | 上一步不可行时尝试下一条合法路径。它必须是已定义的业务或搜索路径，不能是吞掉错误的异常处理。 |
| `used` | 当前班次已经占用的干员名集合。提交新房前必须与候选做互斥检查。 |
| commit | 把一个候选写入 `BaseAssignment`，同步效率快照并更新 `used`。 |
| pipeline | `select → plan → execute → fill → resolve → rotation → export` 的完整生命周期。修 bug 时需要指出不变量在哪一步丢失。 |

## 排班与轮换

| 术语 | 含义 |
|------|------|
| Peak / 高峰班 | `AssignShiftMode::Peak` 生成的主力编制，也是 αβγ 切队和绑定派生的起点。 |
| Recovery / 恢复班 | `AssignShiftMode::Recovery` 的较弱替补 / 恢复路径；不是完整心情规划器。 |
| αβγ ABC | 当前唯一的三队轮换模型。S1 为 α+β，S2 为 β+γ，S3 为 γ+α。 |
| H1 / H2 | 把生产设施切成的两个稳定半区。peak 的 H1/H2 分别形成 α/β，γ 为替补。 |
| cohort | 必须共享出勤节奏的一组房间或成员。跨房 `shift_bind` 可能让多个房间进入同一 cohort。 |
| `shift_bind` | 已入选成员之间的同队、同上同下、上 N 休 M 约束。它不负责让成员第一次入选。 |
| presence vector | 某成员在 S1/S2/S3 是否上岗的三位向量，用于验证 producer/consumer 或 bind 的同班关系。 |
| α 队 | S1、S3 上岗，S2 休息。 |
| β 队 | S1、S2 上岗，S3 休息。 |
| γ 队 | S2、S3 上岗，S1 休息。 |
| mood ETA | 从当前净心情消耗估算主力可持续工作时间的锚点。当前不会自动改写固定班次。 |
| 菲亚覆盖 | 当前 ABC 路径中的一次轻量菲亚梅塔主力回岗替换；不是完整心情调度。 |
| MAA JSON | 由最终三班 assignment 映射出的 MAA 基建排班协议文件。导出层不重新计算机制。 |

## 效率、产出与评分

| 术语 | 含义 |
|------|------|
| `Efficiency` | 生产域唯一效率类型，内部使用 `i32` 千分位；`1.000` 表示基础 100%。 |
| base efficiency | 房间基础效率。 |
| occupancy efficiency | 因进驻人数产生的基础占用效率。 |
| skill efficiency | 房内技能贡献。 |
| control efficiency | 中枢对贸易房产生的注入分量。 |
| paper efficiency | 贸易纸面效率：基础、占用、技能和中枢分量之和。 |
| unit output | 社区确认的单位日产出 / 订单产出模型。 |
| unit output multiplier | 单位日产出相对基准日产出的倍率。 |
| final efficiency | 生产搜索和排班汇总使用的最终直接效率。贸易通常为纸面效率乘单位产出倍率。 |
| mechanic equivalent efficiency | 对特殊订单机制的解释值。它不参与第二次乘法，也不是排序真源。 |
| weighted efficiency | 按班次分钟数对直接效率做的整数比例折算。 |
| `DailyTotals` | 贸易、制造、发电分别保存的 24 小时加权汇总，不存在匿名跨域总计。 |
| scoring policy | 对非生产或局部混合分量进行排序的具名规则。policy 必须有稳定 ID 和 breakdown。 |
| `ControlInjectRawSumV0` | 当前中枢普通候选的局部 policy：贸易注入 + 赤金制造注入 + 经验制造注入。不是生产效率。 |

## Bake、验证与维护

| 术语 | 含义 |
|------|------|
| Bake / 烘焙 | 预先生成 3/2/1 人单房候选索引，减少兼容上下文中的实时组合求值。 |
| baked catalog | `combo_table.bin`、`operators.json`、`manifest.json` 等本地 Bake 产物。当前代码要求 schema v12；仓库内旧 catalog 仍可能是更早 schema，门禁拒绝后会走实时搜索。 |
| compatibility gate | 读取 Bake 前对输入指纹、生成器、练度、布局和动态上下文做的安全检查。不兼容就实时搜索。 |
| effect signature | 能决定某个候选结算结果的上下文摘要。当前 Bake 只覆盖严格受控的签名；更完整的 A+ 设计仍是计划。 |
| A+ Bake | 完整单房 tuple 目录 + 运行时安全连接的未来设计，见 [动态 producer / Bake TODO](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)；当前尚未实现。 |
| fixture / 夹具 | 可重复构造输入的最小或标准数据，例如 `data/fixtures/243/` 和 verify 中的三人房间。 |
| regression anchor | 锁定已确认数值或不变量的期望，例如最终效率、单位产出、规则 ID 或 required anchor。 |
| smoke test | 用真实 CLI 主路径快速验证改动没有破坏基础运行；它不能代替定向不变量回归。 |
| baseline | 用于性能、失败集合或账号画像比较的已知参考。比较时必须记录具体文件和命令。 |
| 验证留痕 | 将完整命令、输入、时间、stdout/stderr、exit code 和摘要保存到 `target/codex-logs/`，将真实 JSON 保存到 `out/`。 |
| 维护期 | 当前默认项目阶段：复现 bug、定位责任层、最小修复、补回归、保持口径稳定，而不是主动扩张历史 Phase。 |
| 真源 / source of truth | 对某类事实具有最高裁决权的材料。业务语义服从用户裁决和维护期 Markdown；运行时数据只负责实现这些语义。 |

## 容易混淆的四组词

1. **required anchor ≠ `shift_bind`**：前者保证进编，后者只约束已入选成员怎样轮换。
2. **shortcut ≠ System**：前者结算实际组合，后者描述编排结构。
3. **role ≠ 固定三人组**：role 保留核心或 fallback 顺序，队友仍可自然搜索。
4. **mechanic equivalent efficiency ≠ final efficiency**：前者解释特殊机制，后者才用于生产排序与汇总。
