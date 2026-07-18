# 排班轮换（Schedule Rotation）

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/排班模式.md；docs/定时换班.md；docs/Fiammetta.md
> 复核触发：crates/infra-core/src/schedule/**；crates/infra-core/src/export/**；crates/infra-cli/src/commands/plan.rs；crates/infra-cli/src/commands/layout.rs
> 摘要：记录当前 ABC 三队轮换实现事实和入口
> 源摘要：18b9596f5a12cd2f2e6bafe0b951e0807d1c259b70c9edae3295eefe940396ca
> 文档摘要：d21840ee8c72f9b97aab4e8bb1fe67ed10fe28ee56cbe64cf784c2ea05e4aedf
> 复核原因：document-change
> 复核结论：updated
> 稳定事实：记录当前 ABC 三队轮换实现事实和入口
> 证据引用：tracked:docs/SCHEDULE_ROTATION.md

> **当前实现参考**：αβγ **ABC 三队轮换**（Agent 默认经 `plan` 触发；仅排班入口为 `layout team-rotation`；核心 API 为 `schedule_team_rotation`）。模式边界由 [排班模式](排班模式.md) 裁决，定时换班规则由 [定时换班](定时换班.md) 裁决。A-B-A 的 CLI、core API 与 MAA 导出已移除。

> **已知缺口**：本页流程图中的 `meta_vina` 是当前 legacy 代码路径，不是业务目标。三类可选动态贸易 producer 的统一搜索与 dependency 见 [CONTROL_CENTER_ASSIGNMENT.md](CONTROL_CENTER_ASSIGNMENT.md) 和 [A+ TODO](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

---

## 1. 当前模型

| 项 | αβγ ABC |
|---|---|
| CLI | `plan`（默认，含账号分析）/ `layout team-rotation`（仅排班） |
| 核心 API | `schedule_team_rotation` |
| 班次结构 | 12h + 6h + 6h；每班 **两队上岗、一队休息** |
| 生产设施 | 切成 H1/H2 两半；α/β 来自 peak 切半，γ 替补 |
| 中枢 / 宿舍 | 宿舍与非绑定办公室成员共享；绑定办公室成员按 cohort 轮换；中枢按 αβγ 轮休重分配，每班补满 5 人 |
| 设施空转 | **禁止**（每班满编） |

用户说「跑一遍模拟」「三班模拟」时，Agent 默认用 **`plan`** + `--maa-out`（账号分析 + αβγ 排班）；只有用户明确要求“仅排班”时才用 **`layout team-rotation`**。见 [INFRA_CLI.md](INFRA_CLI.md)。

---

## 2. ABC 轮换流程

```
peak = assign_shift_with_plan(Peak) → { assignment, plan }
shared = pinned_assignment_excluding(peak, bound_office_ops)
                                       # 宿舍/非绑定办公室共享；中枢不钉死
[h1, h2] = split_production_facilities
align_shift_binds(h1, h2)            # 迷迭香+黑键等同队
α = peak ∩ h1,  β = peak ∩ h2
γ = assign_team_gamma_half(h1) + assign_team_gamma_half(h2)  # 贸易同样走 docus/closure/witch/meta_vina/witch_fallback role
team_ctrl = build_team_control_map(peak.control, plan, h1)    # 中枢干员归入 αβγ
team_ctrl += core inject / hr-mood control candidates         # 效率注入/公招心情散件

S1 (12h): shared + control(α+β) + α(H1) + β(H2)   休息 γ
S2 (6h):  shared + control(β+γ) + β(H2) + γ(H1)   休息 α
S3 (6h):  shared + control(γ+α) + γ(H2) + α(H1)   休息 β

菲亚覆盖：三班组装完成后，从 peak 主力按确认优先级选择一人，在其所属队伍
原本休息的班次中放回原房间，并换下一个当前在岗干员。
```

γ 替补贸易与 peak `assign_trade_remainder` 同路径：金单先尝试 `docus → closure → witch → meta_vina → witch_fallback → karlan → penguin`，再 plain；制造/发电仍站绑定贪心。

实现：`crates/infra-core/src/schedule/team_rotation.rs`。

---

## 3. 中枢轮休规则

中枢现行规则是**按队伍轮休**，不是三班钉死同一套 5 人。目标：

- 每班中枢满编 5 人。
- 休息队的中枢干员不进入当班中枢。
- 每个中枢干员在 αβγ 周期内至少休息 1 班。
- 优先保留贸易 / 制造效率注入；未被体系认领的 producer 不作为中枢插件补位。

中枢干员归队：

1. 体系绑定中枢位跟随其生产体系所在半区：H1 → α，H2 → β；纯中枢体系默认 α。
2. peak 中枢里的非体系散件按当前队伍人数最少优先均分到 α/β/γ。
3. 额外中枢候选只收两类：核心贸易/制造注入，或公招/心情回复类补位。
   未被 `base_systems` 认领的状态 producer（热情、木天蓼、情报储备等）不作为普通插件放入中枢。
4. 若额外候选已经在生产队中，跟随该生产队；否则分到当前中枢人数最少的队。

每个班次只用活跃两队的中枢候选建池。体系中枢位先 pin 到 `control`，再由 `assign_control` 补满 5 人。轮换内设置 `skip_standalone_control = true` 以保留体系 pin，但补位仍按 plugin 规则过滤；排序分为 `ControlInjectRawSumV0` 注入分量（`trade_inject + manu_gold + manu_br`）加公招/心情补位分。这是中枢候选的局部排序 policy，不是贸易/制造平衡公式。

S2 有深海短班特例：若可构造歌蕾蒂娅中枢 + 深海制造候选，且制造最终效率优于普通 S2，才采用该路径；深海链不进入 12h 主班。四名深海猎人在制造加成上等价，候选只枚举“每个制造站放几名深海猎人”（单站 0–3、总数 4），不枚举具体干员排列或原房间成员子集。

相关单测：

- `team_rotation_control_center_rest_rotates`
- `team_rotation_control_center_respects_resting_team`
- `team_rotation_control_prefers_trade_manu_inject_over_resource_only_fillers`
- `team_rotation_abyssal_only_runs_in_s2_short_shift`

---

## 4. 243 贸易 core role

243 双贸中，贸易站不再依赖 `blackkey_closure` / `witch_long_beta` fixed registry 早占房。peak 与 γ 替补都走同一条 role 顺序：

> **已知实现缺口（2026-07-14）**：`meta_vina` 仍是当前 legacy role，不是已确认业务目标。八幡海铃、戴菲恩、凛御银灰应由同一 control + trade 联合候选自然入选，并由 winner 的实际 consumer 派生轮换依赖。详见 [CONTROL_CENTER_ASSIGNMENT.md](CONTROL_CENTER_ASSIGNMENT.md) 与 [A+ 实施 TODO](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

1. `docus`：只在蓝图实际有至少 2 间贸易站且至少 1 间为龙门币订单时成为 required core；全源石单不启用。anchor 解析到实际龙门币站后，再从所有可用干员中按最终效率选择队友。叙拉古不注册固定体系：八幡海铃、伺夜、贝洛内均可不入选，伺夜与贝洛内可单走、同上且跨站、或自然同站；包含八幡海铃的中枢候选只有在最终实际贸易成员使其动态收益胜出时才采用。shortcut 仅按最终实际同房组合自然结算。
2. `closure`：只在蓝图实际恰有 1 间贸易站且该站为龙门币订单时成为 required core；全源石单不启用。有黑键时可命中 `gsl_blackkey_closure`，缺黑键时仍上可露。
3. `witch`：巫恋 + 龙舌兰 + 裁缝 β/α；普通白板不得进入自动龙巫站。
4. **legacy `meta_vina`（待删除）**：当前由戴菲恩 producer 激活推王 + 摩根 + 维娜固定优先；目标状态让格拉斯哥候选按实际 `final_efficiency` 自然竞争。
5. `witch_fallback`：无龙舌兰时的巫恋兜底。
6. `karlan` / `penguin` / plain：灵知孑、企鹅、散件工具人。

双贸易站中 Rosemary/黑键已激活，且但书、可露希尔和完整龙巫均可形成时，三个贸易 cohort 定义为：

| cohort | 贸易站 | 成员约束 |
|--------|--------|----------|
| A | 但书核心站 | 但书 + 正式效率搜索选中的两名队友；full-E2 期望伺夜+贝洛内 |
| B | Rosemary 黑键站 | 可露希尔 + 黑键 + 正式搜索第三人 |
| C | 自动龙巫站 | 巫恋 + 龙舌兰 + 裁缝 β/α |

菲亚后处理之前的基础 rotation 必须容纳三组，组合为 `A+B → B+C → C+A`；房号、H1/H2、αβγ 标签不固定。可露希尔与黑键同组是该完整 Rosemary alternative 的声明式 packing 结果，不是全局“黑键固定可露”；B 的第三人和 A 的叙拉古成员仍由正式搜索决定，C 仍只接受合法裁缝。A/B 若被已有 exact bind 连成不可分分量，或自然 H1/H2 打包不能让两边各有目标，计划阶段事务性降级，不得到 schedule 再搬房或报错。缺可露、缺完整龙巫或 Rosemary 未激活时同样分别降级，不得把降级态伪报成完整三组。单测：

- `team_rotation_binds_only_actual_syracusa_members`
- `team_rotation_partial_trade_meta_keeps_docus_closure_and_witch`
- `team_rotation_full_e2_resolves_gamma_cohort_before_postprocess`

`ResolvedRoleReserve.reuse_policy` 是基础 rotation 的通用轮换语义：`once` 只在 H1/H2 中首个可行目标房使用一次；`every_eligible_half` 则要求两个 half 都存在 eligible room，并在 `apply_fiammetta_return` 前把同一组已解析成员分别写入两个 γ half。带 `require_pre_split_halves` 的 pack 在 plan 提交前先验证自然切半，失败即整包降级；schedule 只校验已经提交的 reserve 未缺目标、未被基础 fill 覆盖。reserve 不保护菲亚后处理后的最终房间成员。

暖机干员的“房间稳定”仍只约束其连续上岗时不跨房；基础三 cohort 状态额外要求 C 作为同一 γ 替补组复用于 A、B 各自休息的班次。实际叙拉古 producer dependency 只能绑定 A 中真正贡献的成员，不能把 B 或 C 并入该 bind，也不能借此把 A/B 两个贸易房合并成同一 cohort。

---

## 5. 班次绑定（shift_bind）

部分干员须 **同上同下、上 N 休 M**，在 schedule 层处理（非编排层、非 global effect）。

资源转换链遵循相同的实际 presence 原则：provider 与 converter 同班实际工作时转换才激活；
这不是 required admission，也不要求同房。当前同一 buff 自闭合链无需新增 bind；跨干员链必须
由联合 winner 返回实际 provider/converter dependency 后才能进入 `shift_bind`，不能由 schedule
按名字反推或强塞成员。

| 绑定 ID | 干员 | 规则 | 模块 |
|---------|------|------|------|
| `rosemary_blackkey` | 迷迭香、黑键 | 同队；αβγ 周期内上岗 2 班、休息 1 班 | `schedule/shift_bind.rs` |
| `human_fireworks_*` | 乌有、实际入选的重岳/令；纯分支另含桑葚 | 同队、上 2 休 1；桑葚休息班办公室合法补位 | `AssignmentPlan` + `team_rotation.rs` |
| `syracusa_actual` | 当前实现：最终实际入选的八幡海铃 E2 + 带目标 tag 的贸易成员 | 当前 Haru 专用路径；仅有效 producer 与至少一名实际 consumer 同时存在时派生 | `AssignmentPlan::derive_actual_shift_binds` |

目标状态用统一 `ResolvedProducerDependency` 取代 Haru 专用推断：八幡海铃绑定所有实际叙拉古贸易 consumer；戴菲恩绑定各房实际格拉斯哥 consumer；凛御银灰只绑定达到三人阈值房内的谢拉格 consumer。三者均同队、上 2 休 1，但不同贸易站不要求同房；未入选或未贡献者不绑定。

**对齐**：若 peak 编制下绑定组成员落在不同 H1/H2 半区，`align_shift_binds_in_halves` 交换同类设施房间，使二者进入同一 cohort（α 或 β）。

**休息班次**（与队伍标签绑定）：

| 队 | 休息班 |
|----|--------|
| γ | S1（12h） |
| α | S2（6h） |
| β | S3（6h） |

单测：`team_rotation_rosemary_blackkey_shift_bind`。

---

## 6. CLI 与 MAA

```bash
cargo run -p infra-cli -- plan \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --maa-out out/243_maa.json
```

- stdout：账号画像摘要 + 人类可读三班排班表 + 队伍花名册
- stderr：写出路径与运行元数据
- `--maa-out`：MAA 排班 JSON（见 `export/maa.rs`）

仅排班、不需要账号画像时：

```bash
cargo run -p infra-cli -- layout team-rotation \
  --layout data/fixtures/243/layout.json \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --maa-out out/243_maa.json
```

不存在 A-B-A 兼容入口；旧调用应直接迁移到 `plan` 或 `layout team-rotation`。

---

## 7. 与编排层的关系

- **单班编制**（peak/recovery）：`assign_shift` → 编排 `System → Plan → Execute`（见 [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md)）。
- **多班轮换**：`AssignmentPlan` 只声明或从 peak 实际落位派生 `shift_bind`；schedule
  在 peak 编制之上切半 + γ 替补，并唯一负责执行 αβγ 归队与同上同下。
- 宿舍与未绑定办公室成员：由 `pinned_assignment_excluding` 从 peak 拷贝并共享。
- 绑定办公室成员：从 shared 排除，随其消费方 cohort 轮换；休息班由未占用的合法
  办公室候选按实际办公室效率补位，不固定房号或班次。
- 中枢：不在 `pinned_assignment` 中钉死；每班按活跃两队重搜/补位，休息队不得上岗。
- 迷迭香感知 producer：不作为完整 `rosemary_perception` System 进编；由 global effect + `assign_perception_producers` 处理（Phase 4）。
- 黑键贸易站：`blackkey_closure` 仍是 L3 shortcut / segment 锚点；排班入口由 `closure` role 选择，不再作为 fixed registry 早占站。

`TeamRotationReport.peak_plan` 携带完整 `AssignmentPlan`（JSON 可序列化）；text 输出打印已选体系与贸易 meta 房间。

### 7.1 peak 主力最长工作时间

`schedule_team_rotation` 在当前启发式流程选出的 peak assignment 生成后立即调用 mood ETA 内核，
把以下信息写入 `TeamRotationReport.peak_mood_eta`：

- 每名在岗生产/功能干员的净心情消耗；
- 每名干员从满心情到休息阈值的可工作时间；
- 首个瓶颈干员；
- peak 主力班的最长工作时间。

CLI 文本和 CSV 会展示该锚点，JSON 保留完整 `per_op` 明细。当前固定
`12h + 6h + 6h` 尚未据此改变；下一步才用它设计和校验短班。

### 7.2 菲亚梅塔主力回岗覆盖

当前 ABC 主路径已接入一次轻量菲亚覆盖：

1. 账号必须拥有菲亚梅塔；
2. 从 peak 主力按 `但书 > 巫恋 > 龙舌兰 > 清流 > 可露希尔` 查找目标；
3. 定位该主力所属队伍的休息班；
4. 将主力放回 peak 原房间，在所有合法替换位中选择该设施最终效率最高的一种；
5. 被换下的干员离开当班工作 assignment；具体宿舍安排由 MAA 执行，本项目不保证写入某个宿舍槽位；
6. 重新计算该班直接效率、加权效率与全日 totals；
7. MAA 只在实际发生回岗的 plan 中输出 `Fiammetta.enable=true`。

每个 24 小时 αβγ 周期当前只安排一次回岗。该步骤是基础 rotation 之后唯一允许改写生产房成员的后处理：它可以让但书回岗并换下 B 或 C 中的一名当班人员，因此最终 assignment / MAA 不再承诺精确 `A+B → B+C → C+A`，也不承诺 C 在两个最终班次中保持完全相同；reserve 只证明菲亚前的基础 γ 正确。布局动态优先级、菲亚实时心情与多次使用序列仍属于后续完整心情排班器。

当前固定业务优先级高于单班瞬时效率门槛：系统会在合法替换位中选最终效率最高者，
但不会用当前仍待校正的局部效率数值取消已确认的主力回岗。全周期收益仍应结合
主力最长工作时间、替补持续时间和宿舍恢复计算。

---

## 8. 相关文件

| 文件 | 作用 |
|------|------|
| `schedule/team_rotation.rs` | ABC 主流程 |
| `schedule/shift_bind.rs` | 班次绑定定义与对齐 |
| `layout/assign.rs` | `pinned_assignment_excluding`、`assign_control`、`assign_team_gamma_half`（γ 贸易 role + plain） |
| `search/role_pick.rs` | `docus` / `closure` / `witch` / `witch_fallback` 贸易 role fallback 链 |
| `search/control.rs` | 中枢候选搜索、`ControlInjectRawSumV0` 排序、补位策略 |
| `schedule/base_rotation.rs` | `evaluate_base_assignment_efficiencies`（ABC 的逐房直接效率结算） |
| `infra-cli/commands/layout.rs` | `team-rotation` 子命令 |
| `export/maa.rs` | MAA JSON 导出 |

---

## 9. Agent 提示

- **跑模拟** → 默认 `plan`，需要纯排班时才用 `layout team-rotation`；`layout test` 只做单班探测。
- **改中枢轮休 / 补位** → `schedule/team_rotation.rs` + `layout/assign.rs` + `search/control.rs`；不要把中枢重新放回 `pinned_assignment`。
- **改迷迭香/黑键同休** → `shift_bind.rs` + `team_rotation.rs`。
- **改但书 / 可露希尔 / 龙巫 meta 取舍** → `trade_segments.json` 的 `roles` + `search/role_pick.rs`；不要把 core priority 写回 fixed registry。
- **改 peak 编制** → 编排层 / `assign_shift`，见 [ORCHESTRATION_LAYER.md](ORCHESTRATION_LAYER.md)。

γ 半班生产搜索使用实际共存上下文：H1 合并 β 与当前 γ partial，H2 合并 α 与当前 γ partial；每个贸易/制造房落位前重新 resolve。中枢 seed 按实际 `shift_bind` 的生产半区过滤：正在被 γ 接替的半区处于休息态，其绑定 producer 不进入候选上下文；另一活跃半区的绑定 producer 保留，并在该 producer 活跃的 half 屏蔽新增未绑定目标标签。不能复用静态 peak `production_layout` 或 peak 全中枢代替当班共存成员。

未形成 peak bind 的可选动态 producer 不会被轮换层强塞，只在普通中枢搜索实际选中时受 presence 约束。调度器为 α、β、γ 和两个 γ half 计算三班 presence：无 bind producer 只能与所有实际目标标签贸易干员完全不相交，不能在 roster 阶段后置新建 cohort；已有 bind 的 pair 才允许向量完全相同，其他未绑定标签仍只能出现在 producer 的休息班。找不到合法 control team 时该 producer 本轮不可用。最终导出前统一校验每班中枢恰好 5 人、显式 bind 同上同下且满足上 2 休 1，以及 producer 只按中枢、consumer 只按贸易站统计的逐班 presence 关系。
