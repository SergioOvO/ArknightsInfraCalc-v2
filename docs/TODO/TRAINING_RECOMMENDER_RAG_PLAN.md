# 练度比对与练卡推荐 RAG 企划

> 状态：proposal / waiting-for-review  
> 日期：2026-07-10  
> 来源：QQ 讨论：基于玩家 box 识别当前练度，对照基建组合与散件工具人，输出“哪些组能用、完成度如何、哪些已拥有但没练、哪些缺关键搭档所以暂缓”的建议。

## 0. 结论

建议采用“双层大脑”：

```text
OperBox / 未来森空岛导入
  -> 确定性规则层：拥有、练度、组合完成度、推荐优先级
  -> RAG 解释层：检索体系文档/规则说明，生成用户可读报告
```

规则层负责“判定”，RAG 负责“解释”。不要让 LLM 直接决定谁该练、优先级是多少，否则结果会飘，且不利于回归。

第一版可以先做本地伪 RAG：按 `operator/system/reason_code` 从 Markdown / JSON 片段检索，不急着上 embedding 和向量库。等规则与报告形态稳定后，再替换为真正的向量检索。

## 1. 用户目标

用户上传练度盒后，工具输出：

1. 当前已经具备哪些基建体系 / 组合。
2. 每个体系的完成情况：可用、可练后可用、缺关键搭档、仅散件可用。
3. 已拥有但未达标的练卡清单，按优先级排序。
4. 对“为什么要练 / 为什么暂不推荐练”的自然语言说明。
5. 未来前端可接森空岛；当前后端 MVP 只要求接受现有 `OperBox` JSON，必要时由外部转换器把 MAA / 森空岛格式转成 `OperBox`。

关键产品口径：

- 散件类角色：抽到且未达标，直接给练卡建议。
- 组合类角色：先看核心搭档是否凑齐；只拥有半套时不强推练卡，而是标记“缺关键搭档，暂缓”。
- 最高优先级：高效率散件工具人、体系已齐但核心未练。
- 中低优先级：纯效率 40% 类、替补型、锦上添花型。

## 2. 现有项目可复用资产

| 资产 | 路径 | 用法 |
|------|------|------|
| 练度输入 | `crates/infra-core/src/operbox/mod.rs` | 当前支持 `OperBox` JSON 数组和一图流 xlsx；字段含 `id/name/elite/level/own/potential/rarity` |
| 练度画像 | `crates/infra-core/src/box_profile/` | 已能产出分域差距、参考组合、基础 actions；但不是完整“组合完成度/练卡推荐” |
| 前端主入口 | `crates/infra-cli/src/commands/plan.rs` | 现在写 `--profile-out` 和 `--maa-out`；后续可加 `--recommend-out` |
| 干员归属真源 | `data/operator_instances.json` | 判断干员是否有基建建模、设施归属、tier_up buff |
| 技能机制真源 | `data/skill_table.json` | 机制解释依据；推荐层一般不直接解释公式 |
| 体系目录 | `data/base_systems.json` | 已有体系、slots、elite 要求、priority、label，可作为组合规则种子 |
| 贸易核心角色 | `data/trade_segments.json` | 但书、可露希尔、巫恋等 role pick 与 shortcut 入口 |
| shortcut | `data/trade_shortcuts.json` | 固定最优 / 难 atom 化组合依据 |
| 体系文档 | `docs/SYSTEM_CHAINS.md`、`docs/公孙长乐的体系分析文档/` | RAG 解释材料，按体系选择性检索 |
| 前端契约 | `docs/FRONTEND_CLI.md` | `profile_out` 现有 JSON 契约；新推荐报告不要破坏 schema v2 |

注意：项目里的 “MAA JSON” 主要指排班导出/导入，不是练度盒。练度输入当前应称为 `OperBox` JSON；MAA / 森空岛入口作为转换层处理。

## 3. 非目标

第一版不要做：

- 不新增排班优化目标，不改 `schedule_team_rotation`。
- 不改贸易 / 制造公式，不把推荐权重塞进 solver。
- 不强行推荐“未拥有干员必须抽”；缺人只做组合状态说明。
- 不让 RAG 覆盖规则层的优先级和判定。
- 不恢复 `90 -> 95` 质量提升历史计划。

## 4. 推荐结果模型

建议新增独立报告，不直接改 `BoxProfile.schema_version = 2`：

```json
{
  "schema_version": 1,
  "operbox_label": "data/fixtures/243/operbox_full_e2.json",
  "summary": {
    "owned": 418,
    "modelled_owned": 165,
    "ready_systems": 8,
    "blocked_systems": 3,
    "trainable_recommendations": 12
  },
  "recommendations": [
    {
      "priority": "P0",
      "kind": "train",
      "operator": "巫恋",
      "target": { "elite": 2 },
      "current": { "elite": 1, "level": 70 },
      "reason_code": "system_core_undertrained",
      "system_id": "witch_long_beta",
      "message": "巫恋组核心已齐，但巫恋未达到体系要求。"
    }
  ],
  "systems": [
    {
      "id": "witch_long_beta",
      "label": "巫恋组",
      "status": "ready_after_training",
      "owned_core": ["巫恋", "龙舌兰", "卡夫卡"],
      "missing_core": [],
      "undertrained_core": ["巫恋"],
      "blocked_reason": null
    }
  ],
  "rag_context": [
    {
      "source": "data/base_systems.json",
      "key": "witch_long_beta",
      "text": "巫恋组（定稿）：巫恋Ⅱ+龙舌兰Ⅱ+裁缝β第三人..."
    }
  ]
}
```

### 4.1 系统状态

| 状态 | 说明 | 是否推荐练卡 |
|------|------|--------------|
| `ready` | 核心拥有且达标 | 否，最多展示“已成型” |
| `ready_after_training` | 核心拥有，但有人未达标 | 是，P0/P1 |
| `partial_blocked` | 只拥有部分关键核心，练了也暂时用不起来 | 否，展示缺谁 |
| `missing` | 关键核心基本缺失 | 否，低噪声隐藏或折叠 |
| `standalone_ready` | 散件拥有且达标 | 否 |
| `standalone_trainable` | 散件拥有但未达标 | 是，按规则优先级 |

### 4.2 推荐优先级

| 优先级 | 典型情况 |
|--------|----------|
| `P0` | 体系核心已齐但未练；高收益散件已拥有未练；常用精一四星必练 |
| `P1` | 体系接近完整、补练后能显著提升；次核心或常见替代位 |
| `P2` | 纯效率散件、40% 类、补位型、低边际收益角色 |
| `Info` | 缺关键搭档、未拥有、当前暂缓，不作为练卡任务 |

建议第一版先允许人工维护优先级，不从效率百分比自动推断。

## 5. 规则数据设计

建议新增 `data/training_recommendations.json`，作为推荐层真源。它不是机制真源，不能替代 `skill_table` / `operator_instances`。

示意：

```json
{
  "version": 1,
  "standalone_rules": [
    {
      "id": "e1_four_star_must_train",
      "label": "常用精一四星必练",
      "priority": "P0",
      "targets": [
        { "name": "清流", "elite": 1 },
        { "name": "砾", "elite": 1 }
      ],
      "reason_code": "standalone_must_train",
      "docs": ["docs/需要完成的干员建模.md"]
    }
  ],
  "system_rules": [
    {
      "id": "witch_long_beta",
      "label": "巫恋组",
      "source_system_id": "witch_long_beta",
      "priority_ready_after_training": "P0",
      "priority_blocked": "Info",
      "core": [
        { "name": "巫恋", "elite": 2 },
        { "name": "龙舌兰", "elite": 2 }
      ],
      "pick_one_core": [
        {
          "label": "裁缝β第三人",
          "elite": 2,
          "candidates": ["卡夫卡", "柏喙", "明椒", "折光"]
        }
      ],
      "reason_code": "system_core_ready"
    }
  ]
}
```

### 5.1 规则来源

第一批规则建议由 Claude / 人类从以下来源抽取：

1. `data/base_systems.json`：直接抽 slots、elite、pick_one、priority、label。
2. `data/trade_segments.json`：补但书、可露希尔、巫恋等贸易角色链。
3. 现有“必练图片/表格”：人工转成 `standalone_rules`。
4. `docs/SYSTEM_CHAINS.md` 和体系分析文档：只抽解释片段，不直接当判定真源。

### 5.2 规则解释边界

规则层只判断：

- 是否拥有。
- 当前 elite / level 是否满足 target。
- 核心是否凑齐。
- 是否存在可用 `pick_one` 候选。
- 输出固定 reason_code 和 priority。

规则层不判断：

- 是否值得为了基建抽卡。
- 是否应优先于主线/肉鸽/合约培养。
- 多个体系之间的长期账号规划。

这些可以在 RAG 文案里温和说明，但不能改变结构化推荐结果。

## 6. RAG 设计

### 6.1 检索语料

MVP 语料建议：

| 来源 | 粒度 | metadata |
|------|------|----------|
| `training_recommendations.json` | rule | `rule_id/system_id/operator/reason_code` |
| `base_systems.json` | system | `system_id/label/operators/priority` |
| `trade_segments.json` | role / segment | `role_id/segment_id/shortcut_id/operators` |
| `docs/SYSTEM_CHAINS.md` | heading section | `system_id/operators/domain` |
| `docs/公孙长乐的体系分析文档/*.md` | heading section | `chain_name/operators/domain` |
| `docs/MODELLED_OPERATORS.md` | operator group | `operator/domain/status` |

不建议把 `MECHANICS_REGISTRY.csv` 作为第一版 RAG 语料，噪声大且不是当前代码入口。

### 6.2 MVP 检索方式

先做伪 RAG：

1. 从规则层结果收集关键词：`operator`、`system_id`、`reason_code`、`shortcut_id`。
2. 在预切分的本地 JSONL 文档片段里做关键词匹配。
3. 每个推荐最多取 2-3 段短 context。
4. 将结构化 facts 和 context 一起交给 LLM 生成报告。

后续再替换为：

```text
docs/rag_corpus/*.jsonl
  -> embedding
  -> vector store
  -> top-k by operator/system/reason_code
```

### 6.3 LLM 提示约束

LLM 输入应包含：

- `recommendations[]` 和 `systems[]` 的完整结构化事实。
- 检索到的 context。
- 明确要求：不得新增未出现在 facts 里的推荐；不得提高/降低 priority；不知道就说不确定。

建议系统提示核心句：

```text
你是明日方舟基建练度建议解释器。结构化 JSON facts 是唯一判定来源。
你可以解释原因、合并同类项、调整表达顺序，但不能新增推荐、删除推荐、
修改 priority、修改目标练度或把缺搭档的 Info 项说成必练。
```

## 7. 代码落点建议

### 7.1 infra-core

新增模块：

```text
crates/infra-core/src/training_advice/
  mod.rs
  rules.rs       // load training_recommendations.json
  evaluate.rs    // OperBox -> RecommendationReport
  rag_context.rs // 伪 RAG context key 生成；可选
```

公开 API：

```rust
pub fn build_training_advice(
    operbox: &OperBox,
    instances: &OperatorInstances,
    rules: &TrainingRecommendationRules,
    options: &TrainingAdviceOptions,
) -> Result<TrainingAdviceReport>
```

第一版不需要调用 trade/manufacture solver；这是练度比对工具，不是效率重算。

### 7.2 infra-cli

建议两步走：

1. 新增独立命令，便于测试：

```bash
cargo run -q -p infra-cli -- advice \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --json
```

2. 稳定后接入 `plan`：

```bash
cargo run -q -p infra-cli -- plan \
  --operbox <operbox.json> \
  --profile-out out/profile.json \
  --recommend-out out/recommend.json \
  --maa-out out/maa.json
```

不建议第一步就把推荐塞进 `BoxProfile`，以免破坏前端现有 `schema_version: 2` 契约。

### 7.3 前端 / Bot

前端读取 `recommend.json`：

- 练卡任务列表：展示 `P0/P1/P2`。
- 组合完成度：展示 `ready/ready_after_training/partial_blocked`。
- AI 解释：可以由后端生成，也可以前端把 facts + context 发给自己的 LLM 服务。

## 8. 回归与验收

### 8.1 最小 fixture

建议新增 `data/fixtures/training_advice/`：

| fixture | 目的 |
|---------|------|
| `witch_only_tequila.json` | 只有龙舌兰无巫恋：不推荐练龙舌兰，标记缺巫恋 |
| `witch_ready_untrained.json` | 巫恋 + 龙舌兰 + 裁缝均拥有但未达标：P0 练核心 |
| `standalone_e1_four_star.json` | 常用精一四星拥有未练：P0 |
| `closure_partial.json` | 可露希尔 / 黑键 / 吉星缺一：按规则输出 blocked 或 ready_after_training |
| `all_ready.json` | 全部达标：无 train recommendation，只显示 ready systems |

### 8.2 验收命令

```bash
cargo test -p infra-core training_advice --quiet
cargo run -q -p infra-cli -- advice \
  --operbox data/fixtures/training_advice/witch_only_tequila.json \
  --json
cargo run -q -p infra-cli -- advice \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --json
```

如果接入 `plan`：

```bash
cargo run -q -p infra-cli -- plan \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --recommend-out out/243_recommend.json \
  --maa-out out/243_maa.json
```

### 8.3 断言重点

- 只有半套组合时，不产生 `kind=train` 的高优先级建议。
- 核心齐全但未练时，必须产生 P0/P1。
- `pick_one` 有任一候选达标即可视为该槽位满足。
- 同一干员多条建议时，只保留最高优先级，但在 `related_systems` 里记录关联体系。
- RAG 文案不能改变 JSON facts。

## 9. 风险

| 风险 | 处理 |
|------|------|
| MAA / 森空岛练度格式不一致 | 第一版只收 `OperBox`；转换器另做 |
| 推荐规则和现有排班体系漂移 | `training_recommendations.json` 用 `source_system_id` 链到 `base_systems`，加校验脚本 |
| LLM 幻觉 | 结构化 facts 锁死判定；RAG 只解释 |
| 过度推荐未拥有角色 | 未拥有只输出 Info，不进入练卡任务 |
| 文档语料太长 | 先切 heading section，按 system/operator 精确检索 |
| 规则维护成本高 | 第一版人工维护；后续可写脚本从 `base_systems` 生成草稿，再人工确认 |

## 10. 给 Claude 的分析任务

请 Claude 优先分析以下问题：

1. `training_recommendations.json` 的 schema 是否足够表达“散件必练、核心组合、pick_one、缺搭档暂缓”。
2. 能否从 `data/base_systems.json` 自动生成第一批 `system_rules` 草稿；哪些条目必须人工覆写。
3. 第一批 P0/P1/P2 推荐名单如何从现有表格/图片/文档落成结构化规则。
4. `advice` 独立命令与 `plan --recommend-out` 的接入顺序是否合理。
5. RAG 语料应优先切哪些文档，哪些文档噪声太大不应进入第一版。
6. 是否需要在规则里区分 `target.elite` 与 `target.level`，以及 40% 纯效率角色是否需要 level 要求。
7. 如何处理同一干员在多个体系中的重复推荐与解释合并。

建议 Claude 输出：

- 修订后的 schema。
- 首批规则样例，至少覆盖：巫恋组、但书、可露希尔黑键吉星、常用精一四星、40% 效率散件。
- MVP 实现拆分：数据、core、cli、测试、RAG corpus。
- 至少 5 个回归 fixture 的输入/期望。

