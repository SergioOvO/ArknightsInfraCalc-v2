# 243 全精二默认模拟案例

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/BASE_ASSIGNMENT.md；docs/SCHEDULE_ROTATION.md
> 复核触发：data/fixtures/243/**；crates/infra-cli/src/commands/plan.rs；crates/infra-core/src/schedule/**
> 摘要：记录 243 全精二当前示例和可复现入口
> 源摘要：e19b9b939270bc6b2b8b5f6c4776105bd2be26cd972c90003ca2947e68b3bcde
> 文档摘要：5aecb8e9749ca50d36513e12dce188a5d79a98d556ae1fb3217b75e85b32910d
> 复核原因：user-ruling
> 复核结论：updated
> 稳定事实：记录 243 全精二当前示例和可复现入口
> 证据引用：tracked:docs/EXAMPLES/243_FULL_E2.md

> 用途：给维护者和 Agent 一个可重复的默认 `plan` 入口，并说明如何阅读输出。
> 本页描述当前 CLI 能力和 fixture，不定义任何体系的业务不变量。

## 1. 输入

| 输入 | 路径 | 说明 |
|------|------|------|
| 243 layout | [data/fixtures/243/layout.json](../../data/fixtures/243/layout.json) | 默认 243 测试布局 |
| 全精二 operbox | [data/fixtures/243/operbox_full_e2.json](../../data/fixtures/243/operbox_full_e2.json) | 仓库标准全精二练度盒 |
| 账号画像参考 | [data/fixtures/243/schedule_export.json](../../data/fixtures/243/schedule_export.json) | profile 对比参考，不是当前 solver 的强制排班 |

运行前应先读 [INFRA_CLI.md](../INFRA_CLI.md)、[SCHEDULE_ROTATION.md](../SCHEDULE_ROTATION.md) 和 [QUALITY_AND_AUDIT.md](../QUALITY_AND_AUDIT.md)。

## 2. 默认命令

```bash
cargo run -q -p infra-cli -- plan \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --profile-out out/243-full-e2-profile.json \
  --maa-out out/243-full-e2-maa.json
```

正式验证必须使用 [AGENTS.md](../../AGENTS.md) 的 `run_logged` 模板包装该命令，并使用任务专属文件名，避免覆盖其他证据。

`plan` 未显式传 `--layout` 时使用仓库默认 243 fixture。用户提供 layout、operbox 或输出路径时，以用户参数为准。

## 3. 输出结构

### 3.1 人类可读输出

stdout 当前包括：

- layout、operbox 和已拥有干员摘要。
- 当前账号画像与参考基准分域对比。
- 24 小时贸易、制造、发电加权结果。
- αβγ 队伍花名册。
- `12h + 6h + 6h` 三班一览。
- 每班设施成员、休息队和直接 / 加权效率。
- peak 主力最长工作时间等当前排班元数据。

stderr 包括加载提示、阶段计时、写出路径和 warning。warning 不等于失败；最终以 exit code、结果摘要和产物校验为准。

### 3.2 Profile JSON

`out/243-full-e2-profile.json` 是账号画像与轮换指标快照，当前主要包含：

- `schema_version`
- `layout_label`、`operbox_label`、`baseline_label`
- `summary`、`domains`、`actions`、`flags`
- `rotation` 与 `baseline_rotation`
- `narration_hints`

字段形状以当前序列化类型和 [INFRA_CLI.md](../INFRA_CLI.md) 为准；消费者应检查 `schema_version`，不要假设历史 JSON 永久兼容。

### 3.3 MAA JSON

`out/243-full-e2-maa.json` 当前顶层包含：

- `title`
- `description`
- `plans`

每个 plan 包含班次名称、说明、无人机、菲亚梅塔设置和 `rooms`；`rooms` 下按 trading、manufacture、power、control、dormitory、hire、meeting 等设施输出成员。

MAA 导出是当次 core 排班的映射，不应在导出层重新选择干员或改变班次语义。

## 4. 当前可复现能力

使用标准输入，当前入口能够：

- 完成账号画像和参考基准对比。
- 运行 peak 单班编排、αβγ 三队轮换和逐班效率结算。
- 保持每班生产设施与中枢按当前约束满编，报告工作队和休息队。
- 将当前实际 assignment 输出为人类可读表并导出为 MAA JSON；profile JSON 保存账号分析与轮换指标快照，不是完整房间 assignment 的副本。
- 对已建模的跨设施资源、候选投影、shortcut 和实际 shift bind 进行当前实现范围内的求值。
- 在 baked catalog 不兼容时安全回退实时搜索。

这不表示：

- 全仓库 full suite 当前全绿。
- 当前搜索已经实现全基建、全班次的全局最优。
- 当前所有体系都完成严格审计。
- 当前动态中枢 producer 已完成未来联合 baked search 计划。

`plan` 当前会在账号画像阶段计算一次 rotation，再为最终 stdout / MAA 计算一次 rotation。两次使用相同输入和确定性规则，但它们是两次独立求解；验证时应比较指标与不变量，不应假设 profile 内存在 MAA 那次完整房间 assignment。

## 5. 2026-07-14 观察快照

以下结果来自代码基线 `dfe3cf0`、标准全精二 operbox 和本页默认命令；之后只有文档变更。它展示当前入口能产出什么，不代表未来版本必须固定选择相同第三人、房号或班次。

| 项目 | 本次结果 |
|------|----------|
| operbox | 418 名，全部 `tier_up` |
| 可建模池 | 贸易 75，制造 90 |
| 轮换 | `12h α+β`、`6h β+γ`、`6h γ+α` |
| 24h 加权效率 | 贸易 5.181，制造 9.196，发电 3.552 |
| peak 体系 | `automation_group`、`human_fireworks_perception`、`pinus_sylvestris` |
| 第二次 rotation 耗时 | 7.48 秒（本机 debug `cargo run` 观察值，不是 benchmark） |

实际贸易与中枢快照：

| 班次 | 中枢 | 贸易站 1 | 贸易站 2 |
|------|------|----------|----------|
| Shift 1 / 12h | 令、八幡海铃、焰尾、薇薇安娜、重岳 | 伺夜、但书、黑键（2.883） | 贝洛内、乌有、可露希尔（2.432） |
| Shift 2 / 6h | 凛御银灰、斩业星熊、歌蕾蒂娅、玛恩纳、诗怀雅 | 卡夫卡、巫恋、龙舌兰（2.482） | 吉星、能天使、蕾缪安（2.293） |
| Shift 3 / 6h | 令、八幡海铃、焰尾、薇薇安娜、重岳 | 伺夜、但书、黑键（2.883） | 贝洛内、乌有、可露希尔（2.432） |

该快照也诚实暴露了下一步边界：三类动态 producer 尚未进入统一联合搜索，具体改造见 [DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](../TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

## 6. 排班快照不是业务规则

同一 fixture 的具体 lineup 会随以下内容变化：

- 业务 Markdown 裁决。
- skill / instance / shortcut / segment 数据。
- 候选池完整性和 tie-break。
- scoring policy、bake schema 和搜索算法。

因此不要把某次输出中的具体房号、队名、第三人或某个 top hit 写成测试常量。业务断言应来自领域 Markdown，例如：

- 必须存在的硬核心。
- 允许跨站或禁止同房的作用域。
- 实际 producer / consumer 的同上同下关系。
- 每班中枢人数和设施容量。
- 未入选候选不得被轮换层重新强塞。

如需记录当前 lineup，应将其标为“日期 + commit + 输入 + 命令”的观察快照，并保留对应日志和 JSON；它只能用于 diff 与复现，不能推翻领域真源。

## 7. 最低验收检查

一次正式默认模拟至少检查：

1. 命令 exit code 为 0。
2. profile JSON 和 MAA JSON 均存在且可解析。
3. MAA `plans` 与当前轮换班次数一致。
4. 每个工作房成员不超过容量，同一班无重复干员。
5. 每班中枢按当前规则满 5 人。
6. 明确 bind 的成员实际 presence 一致。
7. stdout 的核心成员与同一次最终 rotation 生成的 MAA 工作房一致；宿舍休息成员不能误算为在岗。profile 只比较轮换指标，不比较完整房间 assignment。
8. 日志包含命令、输入、耗时、exit code 和结果摘要。

Full suite 有既有失败时，按 [QUALITY_AND_AUDIT.md](../QUALITY_AND_AUDIT.md) 比较精确失败集合；不得只因本次 `plan` 成功就声称项目全部通过。

## 8. 常见误用

- 用 `layout test` 单班探测代替用户要求的完整模拟。
- 不传 `--profile-out` / `--maa-out`，覆盖 operbox 相邻文件或丢失证据。
- 把 MAA 宿舍中的休息干员算作工作 presence。
- 把一次全精二 top hit 固定为体系 hard core。
- 只检查 JSON 文件存在，不检查解析、班次数、中枢人数和核心成员。
- 把 profile JSON 当成最终 MAA assignment 的逐房序列化副本。
- 用旧 baked catalog 的结果判断当前实现；catalog mismatch 时应确认实时 fallback。
