# 质量与严格审计总则

> 状态：维护期当前规则。
> 目的：把“业务真源、生命周期审计、回归、验证证据、失败基线和 bake 安全”收敛到一个稳定入口。
> 本文不是体系业务说明；具体机制仍以对应领域 Markdown 为最高权威信源。

## 1. 入口与适用范围

普通维护、bug 修复和“结果不对”先读：

1. [AGENTS.md](../AGENTS.md)：仓库级硬门禁、模块边界、验证留痕和 Git 纪律。
2. [MAINTENANCE_MODE.md](MAINTENANCE_MODE.md)：维护期复现、分层定位和最小修复流程。
3. [SYSTEM_AUDIT_WORKFLOW.md](SYSTEM_AUDIT_WORKFLOW.md)：体系逐项审计、用户裁决和完成证明。
4. [PROJECT_MAP.md](PROJECT_MAP.md)：当前代码地图和数据真源。
5. 当前问题对应的领域 Markdown；从 [INDEX.md](INDEX.md) 路由，不全仓库盲读。

标准 243 全精二入口示例见 [EXAMPLES/243_FULL_E2.md](EXAMPLES/243_FULL_E2.md)。动态中枢 producer 的未来联合搜索计划见 [TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

## 2. Markdown 真源

业务语义优先级固定如下：

1. 用户在当前对话明确确认或纠正的口径。
2. 当前维护期领域 Markdown。
3. 当前流程、模块和数据边界 Markdown。
4. 数据载体、代码、注释、测试、fixture 和历史输出。

用户纠正文档时，必须先更新相应 Markdown，再修改实现和回归。代码当前行为不能反推业务规则；旧测试若保护错误语义，应改写或删除，而不是让 Markdown 迁就测试。

若两个当前 Markdown 冲突，停止实施并列出冲突文件、具体口径和需要用户裁决的问题。不得自行折中，也不得保留两条互相冲突的兼容路径。

## 3. 修改前四项门禁

体系、编排、跨设施或轮换问题在编辑前必须完成：

| 门禁 | 必须写清 |
|------|----------|
| 领域不变量 | 硬核心、可选 producer、最低数量、同房 / 跨站 / 在基建内作用域、互斥、班次、降级和关闭条件 |
| 违规位置 | 非法状态在哪个生命周期阶段出现，具体到文件、类型或函数 |
| 单一责任边界 | 修复后由哪个 System、plan 字段、role filter、solver 或通用约束唯一保证 |
| 删除清单 | 将删除或改写的旧 registry、fallback、特判、注释和错误测试 |

“最小修复”是修正最小责任边界，不是在最终输出层追加一个让当前 fixture 通过的 `if`。

## 4. 完整生命周期审计

每条领域不变量都要沿完整路径检查：

```text
select -> plan -> execute -> fill -> resolve -> rotation -> export
```

| 阶段 | 审计问题 |
|------|----------|
| select | 激活 / 关闭条件是否正确；软组合是否被错误升级为 fixed System |
| plan | required anchor、候选最低数量、互斥和 shift bind 是否被准确声明 |
| execute | anchor 是否实际落位；`used` 是否提前抢走后续合法候选 |
| fill | 是否保留已有全部 anchor；普通池是否完整；房间遍历顺序是否形成隐式规则；前房选型是否错误忽略后房最终状态 |
| resolve | selector 作用域、跨房 workforce、候选投影、family 最大值和最终快照是否正确；需要联合反馈的候选是否先完整落位再统一 resolve |
| rotation | 同上同下是否由结构保证；休息班 producer 是否真正关闭；每班设施是否满编 |
| export | text 与 MAA 是否忠实映射同一次最终 core assignment；profile 的独立指标快照是否与其自身 rotation 一致，不把它误当成完整 assignment 副本 |

当前结果看起来正确但只依赖 top hit、房间顺序、`used`、标准 243 容量或固定班次下标，仍视为未完整实现。应使用删核心、最低人数、竞争候选、跨站和房间重排等最小反例证明结构保证。

## 5. 回归分层

回归应按责任半径分层，不用单个大快照代替全部证明。

### 5.1 数据与机制单元层

- `skill_table.id == buff_id` 和 operator instance 归属。
- Selector、Condition、Action、Phase 和 family 合并规则。
- 0 / 1 / 多消费者、阈值边界、同房与跨站反例。
- shortcut 命中与拒绝条件。

### 5.2 单站 solver 层

- 指定房间成员的最终效率和分量。
- 普通候选与 shortcut 候选使用同一最终排序口径。
- 候选自身必须进入 `base_workforce` / facility workforce 投影，并按姓名去重。

### 5.3 Layout 生命周期层

- 激活、关闭、anchor 进编、禁止替代和跨房可见性。
- 顺序搜索必须让后房看到前房已提交成员。
- 最终 assignment 完成后刷新所有受跨房状态影响的 snapshot；若前房最优性本身依赖后房成员，必须联合枚举，不能只在末尾刷新旧选择。
- 完整池与结构化扩池必须覆盖合法候选，不能只依赖工具人白名单。

### 5.4 Rotation 层

- 实际 bind 成员同上同下、上岗 / 休息次数正确。
- 未入选候选不得在轮换层重新强塞。
- producer 休息班不得残留其作用；每班中枢和生产设施满编且无重复干员。
- 暖机稳定只约束实际连续上岗的房间，不构成最低进编班数。

### 5.5 CLI 与导出层

- 至少运行一次用户真实 `plan` 或 `layout team-rotation` 入口。
- profile JSON 和 MAA JSON 使用任务专属路径写入 `out/`。
- 对 stdout / MAA 断言设施类型、实际成员、工作 / 休息状态和核心字段；profile 只断言账号分析与 rotation 指标，不要求它包含最终 MAA 那次完整房间 assignment。
- `plan` 当前会为 profile 和最终 stdout / MAA 分别运行 rotation；比较两者时核对指标与不变量，不宣称它们共享同一个 in-memory assignment。

## 6. 验证留痕

任何 test 调用都无例外使用 [AGENTS.md](../AGENTS.md) 的 `run_logged` 模板。用于结论的 build、CLI smoke、benchmark、格式和结构校验也按同样标准留痕。

每份 `target/codex-logs/*.log` 至少包含：

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

裸跑结果、终端滚屏、Agent 消息、`/tmp` 和 commit hash 都不能代替最终证据。若探索阶段裸跑，交付前必须带日志重跑。

## 7. Full suite 与失败基线

本仓库当前不能被笼统描述为“全套测试全绿”。历史和当前维护证据中存在既有失败；具体集合以本轮开始前保存的完整 baseline 日志为准，不在本文硬编码一个容易过时的数量。

Full suite 验收必须：

1. 保存完整失败列表。
2. 从原始 baseline 和当前日志提取测试全名集合。
3. 分别报告 additions、removals 和 unchanged；不能只比较失败数量。
4. 新增失败为 0 才能宣称“没有新增 full-suite 回归”。
5. 修复或删除一个旧失败必须说明其业务依据；旧测试可能本身保护错误语义。

Baseline 是检测回归的工具，不是允许既有错误永久存在的豁免，也不能用于宣称全套测试通过。

## 8. Bake 安全门禁

Bake 是加速载体，不是业务真源。预计算结果只有在候选结构和所有相关上下文与生成模型兼容时才能使用。

### 8.1 当前 schema v10 已实现的门禁

- schema、CLI generator 和输入文件 fingerprint 一致；输入覆盖 baseline layout、instances、skill table、standalone roster、segments、shortcuts 和 systems。
- runtime pool 中的候选名必须被 catalog 覆盖，且当前快速路径要求兼容的 E2 tier 模型。
- room level、capacity、recipe / order、mood、shift hours 与 baseline context 必须满足各设施 gate。
- 动态 inject、候选投影、`OperatorInBase` / `OperatorInTrade`、跨房 workforce 或 `full_pool` 等未被当前表精确表达的上下文会拒绝 Bake。
- catalog 缺失、schema / generator / input mismatch 时返回实时搜索。

当前工作区旧 catalog 与代码 schema 的具体状态见 [PERFORMANCE_ENGINEERING.md](PERFORMANCE_ENGINEERING.md)。当前 loader 尚未校验 `combo_table.bin` 自身内容 hash；非白名单反序列化错误也可能作为错误返回，而不是无条件 fallback；生成物也尚未采用完整 generation-id 原子切换。因此不能把下面的未来要求写成当前能力。

### 8.2 下一代 catalog 必须补齐

- catalog 自身 bytes、内容 hash、row count 和 index checksum。
- 损坏、反序列化失败、未知 signature 与缺行全部安全进入同语义 live 求值。
- 临时 generation 目录、完整校验和原子切换，避免读取新旧文件混合状态。
- cache miss 只替代候选生成 / 求值方式，不能改用更小候选集、固定 top-K 或旧 pipeline。

未来联合候选 Bake 的 winner 仍必须在完整临时 assignment 上统一 resolve；DP、Pareto 和安全上界不是首期前提。详见 [DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

## 9. 完成证明

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

## 10. Git 与工作区

- 开始和结束查看 `git status --short`。
- 现有改动和未跟踪文件默认属于用户。
- 只 stage / commit 本轮文件，不使用 `git add .`。
- `target/codex-logs/` 和 `out/` 默认不提交，但保留到交付。
- 无法可靠拆分同文件用户改动时不强行提交。
