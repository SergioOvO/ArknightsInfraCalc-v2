---
name: gongsun-training-review
description: 验收 training_recommendations.json、公孙长乐练卡推荐、散件清单或组合核心规则时使用；先生成确定性中文验收稿，再逐条输出通过、修改或待裁决结论。
---

# 公孙长乐练卡推荐验收

只验收基建练卡**规则表**，不接管机制公式、搜索剪枝或排班选型。结构化规则决定当前规则层事实，`docs/练卡推荐规则.md` 与用户当前裁决决定规则应该是什么。

账号过滤结果（`advice` CLI 的 `now`/`conditional`/`blocked`）验收走 `docs/练卡推荐规则.md` 的 Agent 入口与夹具，不在本 skill 内改写过滤器。

## 生成验收稿

在仓库根目录运行：

```bash
python3 scripts/render_training_recommendations.py \
  --output out/training_recommendations_review.md
```

若用户提供另一份规则草稿，使用 `--input <path>`。生成稿是可丢弃产物，不提交，也不得直接编辑后反向覆盖 JSON。

需要对照账号结果时另跑：

```bash
cargo run -p infra-cli -- advice --operbox <operbox.json> --pretty
# 或夹具
cargo run -p infra-cli -- advice --operbox data/fixtures/training_advice/witch_only_tequila.json --pretty
```

## 必读来源

1. 完整读取 `docs/练卡推荐规则.md`。
2. 完整读取生成的验收稿。
3. 只在具体条目需要时读取其 `evidence.path` / 对应体系 canonical 和技能数据，不默认通读全仓。
4. `data/standalone_roster.json` 只证明候选进入搜索缩池，不证明值得培养。
5. 开放实现项见 `docs/TODO/TRAINING_RECOMMENDER_RAG_PLAN.md`；代码 owner 见 `docs/PROJECT_MAP.md` 的 `training_advice` / `advice`。

## 逐条门禁

- 规则表是 v2 结构化索引：`kind` / `scope` / `admission` / `members` / `evidence` / `review`；不应再依赖面向用户的 `message`。
- 核对目标是实际技能门槛，不按星级机械套精一或精二。
- 核对 `system`/`combo` 的必需核心和 `pick_one` 槽；缺核心时该规则下核心、重要成员和挂件全部暂缓。
- 2/3/4 星与指定赠送五星白名单可进入 `acquire_then_train`；其他未拥有干员不生成培养建议。
- 同一干员可因独立散件或另一个完整组合获得建议，不能被残缺组合全局禁推。
- 区分组合角色与 P0/P1/P2 行动优先级。
- 第一版不根据设施、配方、布局做过滤；若规则写了这类条件，标记待裁决。
- `review.status=needs_review` 或 `conflicts` 非空必须标记待裁决，不能自行猜测。

## 输出格式

先列 findings，再给统计摘要。每条使用：

```text
[通过/修改/待裁决] rule_id 或 干员名
当前规则：目标练度、优先级、组合角色
依据：canonical 路径与具体规则
意见：无，或一条可执行修改
```

最终汇总：通过数、修改数、待裁决数，以及本轮明确不处理的 RAG、前端或 solver 项。

## 修改边界

默认只读验收。只有用户明确要求落实裁决后才修改 `data/training_recommendations.json`、canonical 和回归；不得为了让验收通过而修改 `data/standalone_roster.json` 或 solver 候选集合。
