# 练卡推荐规则与 RAG 实施计划

> 文档角色：active-change
> 生命周期状态：in-progress
> 当前真源：docs/练卡推荐规则.md
> 摘要：按人工规则表、确定性过滤、RAG 解释三层落地基建练卡推荐

## 0. 目标

给定一个 `operbox`，输出结构化练卡建议：

```text
now          当前可练
conditional  获取后可练
blocked      因核心未齐暂缓
ready        已达标
review       待人工复核
```

再把这份结果与体系分析文档原文交给 RAG，组织用户可读回答。

成功标准：

1. 规则表只保存结构化规则，不承担最终用户文案。
2. 程序只根据 `operbox` 与规则做确定性判定。
3. RAG 只能解释过滤结果，不能新增干员、改目标或改优先级。
4. 四类规则、核心准入、低星获取例外和多规则合并语义与 canonical 一致。

非目标：

- 设施数量 / 布局 / 生产目标条件过滤
- 战斗培养优先级
- 前端页面
- 完整 embedding 向量库
- 反向修改 solver 候选集合

## 1. 已冻结裁决

| 项 | 裁决 |
|---|---|
| 规则类型 | `system` / `combo` / `standalone` / `soft_combo` |
| 作用范围 | `same_station` / `cross_station` / `control_center` / `independent` |
| 输入 | 只看 `operbox` 现有字段 |
| 目标练度 | 规则表显式保存 `elite` / `level`；`skill_id` 只做来源核对 |
| 硬核心准入 | 必需核心已拥有才准入；未达目标只是训练缺口 |
| 核心未齐 | 该规则不产生 `train`；挂件也暂缓 |
| 核心已齐 | 推荐核心与已有挂件 |
| 低门槛获取 | 2/3/4 星默认可建议获取；指定赠送五星白名单（含苍苔） |
| 多规则命中 | 一条干员记录 + 全部 `matches` + 最高 `display_priority` |
| RAG 时机 | 过滤器先产出清单，RAG 再回答 |
| 待复核 | 进入 `review`，不伪装成确定推荐 |
| 文案 | 体系文档负责社区表达；规则表不写最终 `message` |

## 2. 架构

```text
体系分析文档 + 技能资料
        ↓
人工维护 training_recommendations.json (v2)
        ↓
规则校验
        ↓
operbox + rules
        ↓
确定性过滤器
        ↓
账号专属推荐包
        ↓
按 evidence 检索体系原文
        ↓
RAG 生成回答
```

## 3. 分阶段实施

### Phase A — 规格冻结（本切片）

状态：进行中

交付：

- [x] 更新 `docs/练卡推荐规则.md` 为当前合同
- [x] 将本 active change 改为实施计划
- [ ] 确认旧 `message` 字段在 v2 中删除，不做兼容层
- [ ] 确认无外部消费者依赖旧 advice JSON schema；有则单独记兼容输出

验收：

- 每个术语只有一个定义
- “核心不齐是否练已有核心”明确为否
- `acquire_then_train` 是正式动作类型

### Phase B — 规则 schema v2 与加载器

交付：

- 新 `TrainingRecommendationRules` 类型
- 加载 / 校验 `version: 2`
- 全局 `acquisition_policy`
- 四类 `kind` 与 `admission` / `members` / `evidence` / `review`
- 拒绝非法组合：standalone 硬核心、hanger 进 required core、空 evidence 等

验收：

- 合法样例可加载
- 非法样例确定性失败

### Phase C — 确定性过滤器垂直切片

先不批量迁移全表。用 5 条代表规则贯通：

1. `standalone` 独立效率散件
2. `system` 带挂件的大体系
3. `combo` 硬核心小组合
4. `soft_combo` 弱绑定组合
5. 未拥有但允许获取的低星或赠送五星

过滤器输出：

```json
{
  "schema_version": 1,
  "operbox_label": "...",
  "now": [],
  "conditional": [],
  "blocked": [],
  "ready": [],
  "review": []
}
```

干员记录至少含：

```text
operator
action
display_priority
current
target
matches[]
source_refs[]
needs_review
```

验收矩阵见第 5 节。

### Phase D — CLI 输出切换

- `infra-cli advice --operbox` 输出 v2 推荐包
- 不再依赖面向用户的规则 `message`
- 保留 pretty JSON 供人工核对

### Phase E — 全量规则迁移

迁移顺序：

1. 大体系
2. 硬核心小组合
3. 独立散件效率档
4. 弱绑定组合
5. 获取后培养名单

每条规则只记结构化字段和 evidence，不写最终文案。

### Phase F — RAG 输入协议与伪 RAG

- 由过滤结果生成 `source_refs`
- 按 path/heading/operator 检索体系文档片段
- 固定事实骨架 + RAG 解释
- 硬约束：不新增候选、不改 priority/target、术语优先原文

### Phase G — 收尾

- 删除旧 schema 与冲突路径
- 更新 gongsun 验收 skill / render 脚本
- 用 evidence 工具记录 test / CLI / format
- 吸收确认事实到 canonical，拆剩余开放项，归档本计划

## 4. 规则表 v2 最小样例

```json
{
  "version": 2,
  "acquisition_policy": {
    "default_rarity_le": 4,
    "named_exceptions": ["苍苔"]
  },
  "rules": [
    {
      "id": "standalone_clear_stream",
      "kind": "standalone",
      "scope": "independent",
      "label": "清流赤金散件",
      "admission": {
        "required_core": [],
        "pick_one_core": []
      },
      "members": [
        {
          "operator": "清流",
          "role": "independent",
          "target": { "elite": 1, "level": null, "skill_name": "水清则无鱼" },
          "priority": "P0",
          "acquisition": "suggest_acquire",
          "benefit": {
            "facility": "manufacture",
            "product": "gold",
            "note": "收益随贸易站数量变化，但不依赖硬队友"
          }
        }
      ],
      "evidence": [
        { "path": "docs/练卡推荐规则.md", "heading": "人工规则表" }
      ],
      "review": { "status": "confirmed", "conflicts": [] }
    }
  ]
}
```

## 5. 回归矩阵

| 场景 | 期望 |
|---|---|
| 核心齐、核心未达标、挂件已拥有 | 核心与挂件进入 `now.train` |
| 只有挂件、核心不齐 | 挂件不进 `now.train` |
| 部分核心拥有、部分缺失且不可获取 | 规则 `blocked`，无 `train` |
| 缺失核心可获取 | `conditional.acquire_then_train` + 后续计划；无 `now.train` |
| 独立散件已拥有未达标 | `now.train` |
| 独立散件未拥有但 2-4 星 | `conditional.acquire_then_train` |
| soft_combo 缺队友 | 已有成员仍可独立推荐 |
| 同一干员命中体系+散件 | 一条记录，全部 matches 保留，display_priority 取最高 |
| needs_review 规则 | 进入 `review`，不伪装确定推荐 |
| RAG 输入 | 不得包含候选包外干员 |

## 6. 当前进度

- 2026-07-19：用户确认产品逻辑与四类规则、核心准入、低星获取例外、RAG 边界。
- 2026-07-19：canonical `docs/练卡推荐规则.md` 按新合同重写。
- 下一步：Phase B 规则 schema v2 类型与加载器，并准备 5 条代表规则样例。

## 7. 历史说明

早期章节中的旧 schema 示意、`message` 字段和“未拥有一律不生成练卡任务”口径已被 canonical 取代。实现时只以 `docs/练卡推荐规则.md` 与本计划第 1–5 节为准。
