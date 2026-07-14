# 编排层重构路线图（Orchestration Layer）

> **状态**：**Phase 0–3 / 5 已落地**（2026-06）；Phase 4 global effect 收拢进行中  
> **目标**：把硬体系的「谁必须上哪个岗位」从 search/solve 评分里拆出来，统一到 **System → Plan → Execute**；普通制造软同房组合不由 System 声明，仍由全合法普通候选池 + L1 solver 按实际效率发现。
> **背景讨论**：同房组合、跨房体系、global effect 三套入口搅在 `assign_shift` 里是乱源，不是机制太多。  
> **旧稿参考**：[plans/orchestration_engine_design.md](../plans/orchestration_engine_design.md)（阶段二/三「穷举硬体系方案 + DailyTotals 裁决」**不采用**；硬核心由 System 声明，普通制造软同房组合由 search 自然发现）

---

## 1. 问题陈述

历史上 `layout/assign.rs` 的 `assign_shift` 同时承担了多层职责；当前主路径已收敛为 `build_plan -> execute_plan -> fill_greedy`，下表记录边界：

| 职责 | 应在哪 | 当前边界 |
|------|--------|------|
| 选跨站体系 / 固定 bond 进编 | **编排** | `layout/orchestrate::{build_plan, execute_plan}`；`claim_base_systems` 仅兼容 / 测试辅助 |
| 贸易核心优先 / 填散件第三人 | **role policy + 搜索（子集）** | `trade_segments.roles` + `search/role_pick.rs`；`assign_trade_remainder` / 制造 / 发电贪心填空房 |
| 算全局池 / 中枢注入 | **resolve** | `resolve_base` + `cross_facility` / `global_resource` |
| 算同房效率 / 产量 | **L1–L3 solve** | 不负责保证硬体系进编；普通制造在全合法候选池中用同房结算与 `final_efficiency` 排序，自然发现软组合 |

该重构要避免的后果：组合表化后又被 search 打散（如黑键可露链 vs 但书链）、`flat_eff_hint` / 并站 solve 分等补丁继续扩散。

---

## 2. 三种耦合范围 → 三种职责（不混层）

游戏机制（见 `data/MECHANICS_REGISTRY.csv`）可按**耦合半径**分类：

| 范围 | CSV 特征 | 编排 / 运行时 |
|------|----------|----------------|
| **硬同房 bond** | 用户确认必须同房 / 固定核心 | System 的 `bond` / `fixed` slot + L3 `shortcut` 回归锚点 |
| **普通制造软同房** | 标准化、仓容、莱茵技能等效率耦合 | 不注册 System；全合法普通制造池做 `C(n,k)`，L1 solver 按 `final_efficiency` 发现 |
| **跨房体系** | 中枢/宿舍 producer → 贸易/制造 consumer | **同一个 System**，多 `facility` slot + `trade_segments` producer 前提 |
| **全基建池** | 感知、人间烟火、木天蓼、虚拟电站 | **不是组合**；`resolve_base` → `GlobalResourcePool` + `cross_facility` |

**与 `OperatorTier` 枚举的对应关系**（`crates/infra-core/src/layout/tier.rs`）：

| 耦合范围 | OperatorTier | 分配阶段 |
|----------|-------------|----------|
| 跨房体系 | `CrossStation` | 第 1 轮 `select_registry_systems` |
| 同房 bond | `SameStation` | 第 2 轮 `select_registry_systems` |
| 普通制造候选 | `Standalone` | 不在 registry；排班使用全部合法普通制造池做 `C(n,k)`，由 solver 自然排序 |

`base_systems.json` 中每个 System 通过 `"tier"` 字段声明归属，`select_registry_systems` 据此分两阶段贪心：
先跨站、后同站，`exclusive_group` 互斥态跨轮共享。

**原则**

- **编排不算效率**：不调用 `solve_trade_with_shift` 决定 meta 组合。
- **硬体系与普通制造分界**：L3 / System 可承载已确认硬组合；普通制造软同房不表化，search 对全合法普通候选自然排序。
- **global effect 不参与进编**：编制定完后再 `resolve` 写池。

---

## 3. 目标流水线

```
assign_shift()
  └─ orchestrate::build_plan(operbox, blueprint, mode) → AssignmentPlan
       └─ orchestrate::execute_plan(plan) → BaseAssignment
            └─ fill_greedy(remaining empty rooms)   // 仅 Plain 贸易 / 制造 / 发电

resolve_base(assignment)   // 与 assign 分离，CLI 评分 / verify 才走
  ├─ control → GlobalInjectManifest
  ├─ cross_facility → GlobalResourcePool
  └─ per-room solve（L1 → L3 → L2 → production）
```

模块位置（当前）：

```
crates/infra-core/src/layout/orchestrate/
  mod.rs
  plan.rs      # AssignmentPlan, SlotFill, ActivatedSystem
  select.rs    # System 选型、tier 降级、exclusive_group
  execute.rs   # 落位：fixed / bond / core+segment / pick_one
```

`assign_shift` 当前主线为：seed → `build_plan` → `execute_plan` → producer/resolve → 发电 → 贸易 core role / plain → 制造贪心填充。贸易早于制造填充，避免但书、可露希尔、巫恋等核心和其工具人被制造站提前占用。

---

## 4. System 统一 schema（扩 `base_systems.json`）

**同站组合与跨站体系都是 System**，差别只在 `slots` 数量与 `prerequisites`。

### 4.1 `tier` — 三层分配优先级

`base_systems.json` 中每个 System 必须声明 `"tier"` 字段：

| 值 | 含义 | 分配阶段 |
|----|------|----------|
| `cross_station` | 跨站体系（slot 跨多设施） | `select_registry_systems` 第 1 轮 |
| `same_station` | 同站组合（slot 在同一设施内） | `select_registry_systems` 第 2 轮 |

两轮共享 `exclusive_group` 互斥态。未注册的普通制造干员不走 registry；排班不套 standalone 名录，而从全部合法普通制造池做 `C(n,k)`。

对应 Rust 枚举：`crates/infra-core/src/layout/tier.rs` 的 `OperatorTier`，同时标注到 `RegistrySystemClaim`、`ActivatedSystem` 和各设施 `PoolEntry` 的 `tier` 字段。

### 4.2 `fill_mode`

| 值 | 含义 | Executor |
|----|------|----------|
| `fixed` | 整包落位 | 直接 `set_room`；不用于但书、伺夜或贝洛内，三者全部由贸易搜索决定 |
| `bond` | 二人锁死 + 第三人 | 固定 A+B，`pick_one` 填第三（例：德 E0+拉普兰德 **或** 能天使+蕾缪安；同干员不同 tier 须分叉 System） |
| `core` | 单人锚 + segment 池补满 | 仅用于未来非感知散件锚点；**黑键不走此路径** |
| `pick_one` | 列表选一 | 第一个可用干员 |
| `greedy` | Plain | 候选池内 `C(n,k)` 或发电 O(n)（制造排班直接使用全部合法普通制造池） |

`pick_one` 候选默认继承 slot 级 `"elite"` 要求；需要按候选区分精英化门槛时可写对象，例如 `{ "name": "海沫", "elite": 2 }`。对象候选还支持 `"max_elite"`，用于保留少量 E0-only / E1-only 历史锚点。

制造站 slot 可写 `"recipe": "gold" | "battle_record" | "originium"` 约束房间产物；用于自动化组这类“必须进赤金线”的体系。前端生成 layout 时房间编号可能与模板不同，优先使用 `recipe` 约束，不要把清流/温蒂等产线体系硬绑到固定 `room_id`。

### 4.3 与现有文件的关系

| 文件 | 角色 |
|------|------|
| `data/base_systems.json` | System 目录：`tier`（`cross_station` / `same_station`）、priority、`exclusive_group`、slots |
| `data/trade_segments.json` | producer 条件 + consumer → `shortcut_id`；`roles` 声明贸易核心优先 fallback 链 |
| `data/trade_shortcuts.json` | L3 组合级 trade%/gold% 锚点 |
| `layout/system_integrity/` | **已迁出编排主路径**；迷迭香感知链待 Phase 4 `cross_facility` + 计算效率后进编 |

### 4.3 贸易 meta：跨站体系、同站锚点与核心优先

来源：`MECHANICS_REGISTRY.csv` 中「当与 X 在同一个贸易站」+ 公孙工具人表。
`base_systems.json` 保留跨站体系和历史同站锚点；当前 `assign_shift` 主路径会跳过 `witch_long_beta`、`blackkey_closure`、企鹅、推王等低优先 registry 抢站条目，贸易余站改由 `data/trade_segments.json` 的 `roles` 执行核心优先。

> **已知实现缺口（2026-07-14）**：以下 `meta_vina`、`vina_lungmen` 和按 producer 分别重跑前缀的描述是当前 legacy 行为，不是业务目标。用户已确认八幡海铃、戴菲恩、凛御银灰应共用一次自然联合搜索；精确不变量以 [CONTROL_CENTER_ASSIGNMENT.md](CONTROL_CENTER_ASSIGNMENT.md) 为准，实施见 [DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md](TODO/DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。

#### 已落地的数据与运行时归属

| id / role | 类型 | L3 shortcut | 当前运行时 |
|-----------|------|-------------|------------|
| 叙拉古动态注入 + role `docus` | 自然中枢/贸易候选（不注册 System） | `gsl_docus_syracusa` / `gsl_docus_solo` | 八幡海铃、伺夜、贝洛内均不固定；普通中枢路径与包含动态贸易 producer 的路径各自完成贸易搜索后按 `ControlInjectRawSumV0` 比较。伺夜/贝洛内不要求同站，也不要求入编 |
| `closure` | 可露希尔核心优先 | `gsl_blackkey_closure` / `gsl_closure_*` | 强制包含可露希尔；优先黑键可露锚点，缺黑键仍保留可露 |
| `witch` / `witch_fallback` | 龙巫 / 巫恋兜底 | `gsl_witch_*` | `witch` 强制包含精二巫恋 + 龙舌兰；无龙舌兰时 `witch_fallback` 低于推王组，只做巫恋兜底 |
| `ling_jie_karlan` | control producer + L1 自然搜索 | `gsl_ling_jie_yaxin` 仅参考 | 只认领灵知 E2 中枢；精1孑由贸易搜索注入 |
| `meta_vina` / `penguin_*` | legacy Vina role / 企鹅 segment | `gsl_vina_lungmen` / `gsl_penguin_*` | `meta_vina` 当前仍把推王组放在第 4 优先，待删除；Vina shortcut 仅可保留为实际组合结算。企鹅逻辑不在本缺口范围 |
| `rosemary_perception*` | **global effect** | — | 已移出编排；`assign_perception_producers` + scope=global |

#### 贸易核心 role 顺序

贸易金单余站按 `pick_trade_meta_then_plain` 尝试：

1. `docus`：拥有精二但书时，无条件作为全部金单贸易站的第一核心；有空二级金单站时优先进二级站，然后一次性搜索所有“必须包含但书”的候选并按 `final_efficiency` 取最高。`gsl_docus_solo` / `gsl_docus_syracusa` 由求解器按实际组合自然命中，不是候选优先级；没有精二但书时不启用该 role。
2. `closure`：`gsl_blackkey_closure` 优先；否则 `gsl_closure_*`；否则包含可露希尔的最高可用三人组。
3. `witch`：必须同时包含精二巫恋、精二龙舌兰和裁缝 β/α；blank shortcut 不进入自动 role。
4. **legacy `meta_vina`（待删除）**：当前由戴菲恩 producer 激活推王 + 摩根 + 维娜固定优先；目标状态让全部格拉斯哥贸易候选按实际 `final_efficiency` 自然竞争。
5. `witch_fallback` / `karlan` / `penguin` / plain：无龙舌兰巫恋兜底、灵知孑、企鹅、散件工具人三人组，且排除黑键与巫恋同房冲突。

除已标记的 legacy `meta_vina` 外，这条顺序表达独立 core / fallback 策略，不是任意固定三人组优先级。八幡海铃、戴菲恩、凛御银灰都只作为可选中枢 producer；叙拉古、格拉斯哥、谢拉格 consumer 的实际组合由合法候选和 solver 决定。伺夜与贝洛内不要求同站，八幡与单个贸易消费者的极端组合同样合法。

#### Phase 2 待建（贸易 bond）

| System id | fill_mode | 同房 bond（CSV） | tier / 备注 | L3 shortcut |
|-----------|-----------|------------------|-------------|-------------|
| `penguin_texlap_e0` | bond | 德克萨斯 + 拉普兰德（恩怨 +65% 贸） | vault 确认德克萨斯 E2 不失去恩怨；不再限制 E0-only，id 保留兼容 | `gsl_penguin_texlap_e0` ✅ |
| `penguin_texangel_e2` | bond | **德克萨斯 E2** + 能天使（默契） | 德须 **精2**；第三人 `pick_one` | `gsl_penguin_texangel_e2` ✅ |
| `penguin_exusiai_lemuen` | bond | 能天使 + 蕾缪安 E2（相伴 +25%） | 第三人 `pick_one` | `gsl_penguin_exusiai_lemuen` ✅ |

**企鹅物流注意**：上是 **两套核**（德狼 vs 能蕾），不是一条 System；243 双贸通常只认领其一。选型用 `exclusive_group: penguin_meta` 或 priority，**不靠 search 发现**。  
德狼路线以 vault 为准：德克萨斯 E2 不会失去「恩怨」，因此德狼不再限制 E0-only；`penguin_texlap_e0` / `gsl_penguin_texlap_e0` 名称暂保留为兼容旧 id。

#### 一般不表化为 System（仍由 Plain search / tag 处理）

| 机制 | CSV | 原因 |
|------|-----|------|
| 新约能天使「同城加急单」 | 同站每名拉特兰 +15% | tag 叠层，非固定二人 bond |
| 维娜「外贸决议」 | 同站 GSG 干员 +10% | L1 tag 搜索自然结算；Vina shortcut 只结算实际组合，不再作为戴菲恩 producer-gated 选型入口 |
| 孑「市井之道」 | 站内含义依赖订单上限与技能顺序 | 灵知线由 L1 搜索自然上浮，非固定 trade slot / active L3 |

当前 legacy producer 前提（跨房，非 global pool；待按统一 deferred rule 改写）：

- 叙拉古：八幡海铃 E2 的动态贸易标签倍率；仅在八幡与实际叙拉古贸易成员自然同时入选时生效，`haru_e2_in_control` 只保留为 L3 链段 producer 事实
- 喀兰：`karlan_precision`（灵知 E2）
- 推王：当前以 `戴菲恩` E2 在中枢激活 legacy role；目标为戴菲恩逐贸易房 Glasgow `+10%/人` 的自然联合搜索

### 4.4 制造同房 bond（自动化链配套，Phase 2+）

`MECHANICS_REGISTRY.csv` 中「在同一个制造站」；与贸易 meta 并列登记，避免只改贸易漏制造。

| 组合 | CSV 技能 | 编排建议 | 状态 |
|------|----------|----------|------|
| 水月 E2 + 同房标准化技能 | 意识协议 / 标准化 α/β | 不注册体系；进入普通制造候选池，由 solver 结算同房技能并按 `final_efficiency` 自然搜索 | ✅ 普通制造搜索 |
| 阿兰娜 E2 + 温米 | 「搭把手！」 | 自动化金线固定 slot 或 `bond` + 第三人贪心 | ⚠️ shortcut 待补；registry 延至 Phase 3（避免抢公孙金线制造位） |
| Miss.Christine E2 + 酒神 | 盛餐的回报 |  niche；低优先 | ❌ |
| 怒潮凛冬 E2 + 乌萨斯学生自治团 | 情同手足 | tag 同房加成；可并进乌萨斯制造 meta | ❌ |

### 4.5 跨设施落位前提（非同房 bond，但影响 Plan）

| 干员 | 条件 | 影响 |
|------|------|------|
| 烈夏 E2 | **古米在贸易站**（患难拍档） | 制造站选人时须保留贸易古米位 |
| 清流 E1 | 每贸易站 → 当前制造站金 +20% | 自动化组已含；非 bond |
| 戴菲恩 E2 | 中枢 producer | 当前仍连接 legacy 推王组 `prerequisites`；目标改为逐房 Glasgow 自然候选 |

---

## 5. 实施阶段

### Phase 0 — 模块边界 + Plan 类型

- [x] 新建 `layout/orchestrate/`，定义 `AssignmentPlan` / `SlotFill`
- [x] `assign_shift` 调用 `build_plan` → `execute_plan`（行为可先等价迁移）
- [x] **验收**：243 E2 现有集成测仍绿

### Phase 1 — System 选型（select）

- [x] 合并 `claim_base_systems` → `select.rs`（`select_registry_systems`）
- [x] 支持 `exclusive_group`、德克萨斯 E0/E2 企鹅物流 tier 分叉
- [x] **迷迭香感知链移出编排**（待 Phase 4 global effect 后按计算效率进编）
- [ ] **不实现**「多方案穷举 + DailyTotals 裁决」

### Phase 2 — 组合数据

- [x] 补 `trade_shortcuts.json` / `trade_segments.json` / `shortcut.rs` consumer
- [x] §4.3 贸易 meta 待建行齐套（企鹅三路 + 推王 shortcut + 黑键 segment）
- [ ] §4.4 制造 bond：阿兰娜+温米（延至 Phase 3 `bond` execute，避免破坏公孙金线回归）
- [x] **验收**：每个 System 一条 golden test（plan → assignment 快照 + 预期 `shortcut_id`）

### Phase 3 — Executor（execute）

- [x] registry `fixed` / `bond` 落位（`execute_plan`）；贸易余站 `assign_trade_remainder` 贪心
- [x] **已移除 / 不再存在**：`apply_blackkey_colocate_rule`、`assign_trade_meta`、`complete_trade_anchor_rooms`（旧黑键贸锚）
- [x] 贸易核心 role：`docus` / `closure` / `witch` / `witch_fallback` 写入 `trade_segments.json`，由 `search/role_pick.rs` 统一执行
- [x] `assign_shift` 主路径跳过 `witch_long_beta`、`blackkey_closure`、企鹅、推王等旧 registry 早占站条目，改由 role policy 选择
- [x] **验收**：缺伺夜/贝洛内时但书仍进站；缺黑键时可露仍进站；缺裁缝 β 时龙巫只允许裁缝 α，否则不启用自动龙巫站

#### 巫恋 role

- **核心**：`witch` 是龙巫，强制包含精二巫恋 + 龙舌兰。
- **fallback**：自动龙巫内部仅裁缝 β → 裁缝 α；`gsl_witch_long_blank` 只保留单站结算兼容。
- **兜底**：无龙舌兰时走 `witch_fallback`，只强制包含巫恋；当前优先级低于 legacy 推王组，该相对顺序随 `meta_vina` 一并待删除。
- **编排**：不再把 `witch_long_beta` 当固定三人组早占站；由 role policy 在贸易余站搜索里强制包含龙巫锚点。

### Phase 4 — global effect 收拢（与编排并行）

- [x] 感知 producer（爱丽丝/车尔尼/絮雨）`scope: global` atom → `cross_facility`
- [x] 删 `resolve.rs` 的 `apply_perception_producers` 硬编码
- [x] 制造房 `room_layout_for_manu` 声明式扣回 scope=Global atom
- [x] assign 层 `assign_perception_producers`：堆感知源后 resolve → 贪心消费 layout
- [ ] `skill_table` 其余干员 atom 标 `scope: global`（乌有人间烟火等）
- [ ] 删 `resolve.rs` 其余 `apply_*` 硬编码
- [ ] `GlobalInject` 留 `control/interpreter`，供 segment producer 读取
- [ ] 见 [INTERNAL/CROSS_FACILITY.md](INTERNAL/CROSS_FACILITY.md)、[EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md) §九
- [x] 迷迭香 `shift_bind`：`schedule/shift_bind.rs` + `team_rotation`（见 [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md)）

### Phase 5 — team-rotation 对齐

- [x] α/β 从 peak 切半保留编排已认领贸易 meta；γ 走 `assign_team_gamma_half`（当前仍使用含 legacy `meta_vina` 的贸易 role 顺序）
- [x] `assign_shift_with_plan` + `TeamRotationReport.peak_plan` 供轮换层只读编排计划

---

## 6. 控 bug 策略

1. **编排单测不调 solve**；shortcut 单测才调 `resolve_trade_shortcut`。
2. **每个 System 一条 golden test**（`operbox_full_e2` 或最小 roster）。
3. 端到端：`layout team-rotation` + [fixtures/243](../data/fixtures/243/README.md)。
4. 迁移期可用环境变量/feature 对比旧 `assign_shift` 总分（短期）。

---

## 7. 明确不做

- 用 `C(n,3)` + solve 分「发现」已知 meta 组合。
- 在编排层重复实现 global pool 逻辑（归 `cross_facility`）。
- 新 repo / 全量 v3 重写 L1–L3（机制回归成本过高）。
- `orchestration_engine_design.md` 中的全编制笛卡尔积 + 评分选优（组合已表化）。

---

## 8. 相关文档

| 文档 | 内容 |
|------|------|
| [BASE_ASSIGNMENT.md](BASE_ASSIGNMENT.md) | 现行单班编制流水线 |
| [SCHEDULE_ROTATION.md](SCHEDULE_ROTATION.md) | αβγ ABC 轮换、shift_bind、现行排班入口 |
| [SYSTEM_CHAINS.md](SYSTEM_CHAINS.md) | 迷迭香/自动化/推王等体系链参考 |
| [INTERNAL/SHORTCUT_MATCHING.md](INTERNAL/SHORTCUT_MATCHING.md) | L3 匹配与 segment 注册表 |
| [INTERNAL/CROSS_FACILITY.md](INTERNAL/CROSS_FACILITY.md) | global atom 编排（resolve 内） |
| [EFFECT_ATOM_DESIGN.md](EFFECT_ATOM_DESIGN.md) §九 | AtomScope Room vs Global |

---

## 9. Agent 协作提示

- **改贸易 meta 进编** → 先读本文 + 改 `base_systems.json` / `trade_segments.json`，**不要**先改 `search/trade.rs` 打分。新增 System 时务必填写 `"tier"` 字段（`cross_station` / `same_station`）。
- **改机制数值** → 仍走 [AGENTS.md](../AGENTS.md) §5 分层（L1/L2/L3）。
- **改 global 池** → `cross_facility/` + `scope: global` atom，不动 assign。
- **验证模拟** → [AGENTS.md](../AGENTS.md) §6.2 `plan` 或 `layout team-rotation`。
