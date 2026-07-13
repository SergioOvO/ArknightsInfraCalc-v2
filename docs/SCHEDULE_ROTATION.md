# 排班轮换（Schedule Rotation）

> **现行且唯一**：αβγ **ABC 三队轮换**（Agent 默认经 `plan` 触发；仅排班入口为 `layout team-rotation`；核心 API 为 `schedule_team_rotation`）。A-B-A 的 CLI、core API 与 MAA 导出已移除。

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

1. `docus`：精二但书是全部金单贸易站的第一核心；252 等存在空二级金单贸易站时优先进二级站，再从所有可用干员中按最终效率选择队友。叙拉古跨站体系固定八幡海铃中枢；伺夜、贝洛内是两个不要求同站的贸易搜索成员，但书站选择后再补齐尚未入站者。shortcut 仅按最终组合自然结算。
2. `closure`：可露希尔优先；有黑键时可命中 `gsl_blackkey_closure`，缺黑键时仍上可露。
3. `witch`：巫恋 + 龙舌兰 + 裁缝 β/α；普通白板不得进入自动龙巫站。
4. `meta_vina`：戴菲恩 producer 激活时，推王 + 摩根 + 维娜优先于灵知孑，也优先于无龙舌兰巫恋兜底。
5. `witch_fallback`：无龙舌兰时的巫恋兜底。
5. `karlan` / `penguin` / plain：灵知孑、企鹅、散件工具人。

full E2 / 公孙高配上下文中，常见长班形态仍可能是：

| 队 | 贸易站 | 班次 |
|----|--------|------|
| α | 但书 + 当前最终效率最高的两名可用队友 | S1 + S3，共 18h |
| β | 可露希尔 + 黑键 + 吉星 | S1 + S2，共 18h |

但这只是 role 搜索在完整账号下的自然结果，不是“可露固定黑键吉星”或“巫恋固定裁缝 β”。单测：

- `team_rotation_keeps_docus_and_syracusa_cross_station_members`
- `team_rotation_partial_trade_meta_keeps_docus_closure_and_witch`

---

## 5. 班次绑定（shift_bind）

部分干员须 **同上同下、上 N 休 M**，在 schedule 层处理（非编排层、非 global effect）。

| 绑定 ID | 干员 | 规则 | 模块 |
|---------|------|------|------|
| `rosemary_blackkey` | 迷迭香、黑键 | 同队；αβγ 周期内上岗 2 班、休息 1 班 | `schedule/shift_bind.rs` |
| `human_fireworks_*` | 乌有、实际入选的重岳/令；纯分支另含桑葚 | 同队、上 2 休 1；桑葚休息班办公室合法补位 | `AssignmentPlan` + `team_rotation.rs` |

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

`schedule_team_rotation` 在最高效率 peak assignment 生成后立即调用 mood ETA 内核，
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
5. 被换下的干员离开当班 assignment，并在 MAA 宿舍列表中优先获得床位；
6. 重新计算该班直接效率、加权效率与全日 totals；
7. MAA 只在实际发生回岗的 plan 中输出 `Fiammetta.enable=true`。

每个 24 小时 αβγ 周期当前只安排一次回岗。布局动态优先级、龙巫成组服务、
菲亚实时心情与多次使用序列仍属于后续完整心情排班器。

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

γ 半班制造搜索使用实际共存上下文：H1 合并 production seed、β 与当前 γ partial；H2 合并 production seed、α 与当前 γ partial。每个制造房落位前重新 resolve，不能复用静态 peak `production_layout` 代替最终共存成员。共存生产成员真实合并，但 control 仍沿用 peak control seed heuristic，并非全班次联合全局最优。
