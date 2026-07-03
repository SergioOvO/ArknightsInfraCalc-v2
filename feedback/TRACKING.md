# Production Feedback Tracking

> Scope: feedback imported from production beta/test service after launch.
> Tracking started: 2026-07-03.
> Raw folders are evidence; update this ledger instead of editing bundled JSON.

## Current Buckets

| Bucket | Count | Main risk |
|--------|------:|-----------|
| Manufacture recommendation | 8 | Local search picks plausible but expert-worse teams |
| Trade recommendation / station semantics | 4 | Priority, warmup, or forbidden filler mistakes |
| Control center / global injection | 5 | Duplicate effects, missing conditions, bad fillers |
| Power plant | 3 | Candidate shortage, weak robots, linked producer rules |
| Dormitory / layout metadata | 4 | Wrong dorm levels or meaningless dorm occupants |
| Side facility fill | 2 | Required rooms left empty |
| Layout metadata / account analysis | 1 | Missing level distribution |

## Priority Guide

| Priority | Meaning |
|----------|---------|
| `P0` | Blocks export or creates structurally invalid assignment |
| `P1` | Clear wrong recommendation in a production room |
| `P2` | Polish, filler quality, or explanation issue |

## Ledger

### P0: Export Or Structural Failures

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260628-160411 | intake | power | [folder](2026-06-28/160411-发电站候选不足-当前布局有-3-个发电站-但-infra-cli-只生成了-2-组发电站排班-cli-日志显示当前-box-筛选后只有-2-名可用于发电站的候选) | `pool/power` + layout assignment fallback | 3 power rooms but only 2 power assignments; first reproduce from debug bundle and decide whether to allow fallback fillers or fail earlier with actionable layout/account analysis. |
| FB-20260628-170847 | intake | meeting | [folder](2026-06-28/170847-空置了-未放人) | side-facility fill | Meeting room left empty. Reproduce with bundle and check whether side rooms are intentionally optional or missing fill policy. |
| FB-20260628-170850 | intake | office | [folder](2026-06-28/170850-空置了-未放人) | side-facility fill | Office left empty. Track with FB-20260628-170847 unless reproduction shows a distinct office path. |

### P1: Manufacture Recommendation Cases

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260627-085944 | intake | manufacture/gold | [folder](2026-06-27/085944-褐果可以替换成夜烟30-效率) | manufacture search or data | Report says Chestnut can be replaced by Night Smoke 30%. Verify raw `prod_total` and candidate availability before changing priority. |
| FB-20260628-162229 | intake | manufacture/record | [folder](2026-06-28/162229-更建议使用红云-红云酒神克里斯汀的组合会比当前组合高1) | manufacture search / system combo | Vermeil + Vulcan + Christine-like record combo reportedly beats selected by 1%. Needs local eval of selected vs expected team. |
| FB-20260628-162304 | intake | manufacture/record | [folder](2026-06-28/162304-断罪者35-无法完整触发槐琥40-效率-推荐槐琥进入3级站-或者二级站搭配迷迭香等40-以上的角色) | manufacture room compatibility | Conviction 35% cannot fully trigger Waai Fu 40%. Check level/room constraints and whether level-3 placement should be preferred. |
| FB-20260628-170245 | duplicate | manufacture/record | [folder](2026-06-28/170245-红云酒神克里斯汀) | manufacture search / system combo | Same family as FB-20260628-162229; keep separate until reproduced because layout state may differ. |
| FB-20260628-170315 | intake | manufacture/gold + power link | [folder](2026-06-28/170315-推荐调整成清流温蒂冬时-挂钩发电承曦格雷伊-130) | manufacture system trace + linked producer | Purestream + Weedy + Windflit with Greyy the Lightningbearer link. Existing trace-only pilot may help, but still needs repro and regression from this bundle. |
| FB-20260630-052233 | intake | manufacture/gold | [folder](2026-06-30/052233-铅踝工作时间过长) | schedule duration / mood boundary | Work time too long. Verify whether this is outside current non-goal mood scheduling or a wrong exported duration. |
| FB-20260630-052353 | intake | manufacture/gold | [folder](2026-06-30/052353-那斯提挂件没满) | manufacture system completeness | Nasti support not fully satisfied. Needs operator availability and selected-room comparison. |
| FB-20260630-052442 | intake | manufacture/gold | [folder](2026-06-30/052442-蛇屠箱可以换成豆苗) | manufacture search or data | Bubble can be replaced by Beanstalk. Check raw score, room constraints, and whether Bubble is being used for an unintended side effect. |

### P1: Trade Recommendation Cases

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260628-160142 | intake | trade | [folder](2026-06-28/160142-维娜维多利亚在这里只有30-效率-不如任意的35-贸易效率-或者将隔壁伺夜跨站放过来) | trade priority / orchestrate claim | Vina Victoria only gives 30% without Siege group; should lose to generic 35% or pull Vigil cross-station. |
| FB-20260628-170234 | duplicate | trade | [folder](2026-06-28/170234-维娜维多利亚在没有推王组的情况下只有30效率-降低优先级) | trade priority / orchestrate claim | Same Vina priority issue as FB-20260628-160142. Keep as second repro fixture candidate. |
| FB-20260628-170457 | intake | trade + schedule | [folder](2026-06-28/170457-巫恋组不应变更贸易站-否则会重复暖机) | `schedule/shift_bind` or station binding | Shamare group should remain bound to a trade station to avoid repeated warmup. Likely schedule/station identity, not single-station solver. |
| FB-20260629-031259 | intake | trade | [folder](2026-06-29/031259-载入全精二样例-不应该会出现尤里卡-u-official) | trade pool filtering / filler priority | Full E2 sample should not recommend U-Official. Verify pool filter and whether this is a fallback leak. |

### P1: Control Center / Global Injection Cases

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260628-131551 | intake | control | [folder](2026-06-28/131551-中枢制造加2重复) | control global injection dedup | Manufacturing +2 appears duplicated. Check global injection aggregation and display split. |
| FB-20260628-170359 | intake | control | [folder](2026-06-28/170359-m3-凯尔希技能重复-m3严格大于凯尔希) | control candidate dedup / precedence | Mon3tr and Kal'tsit skill duplicate; Mon3tr should strictly dominate Kal'tsit. |
| FB-20260628-170424 | intake | control | [folder](2026-06-28/170424-斩业星熊需要有陈-诗怀雅同在中枢-现在缺少陈) | control condition / selector | Hoshiguma the Breacher requires Chen or Swire in control; selected team is missing Chen. |
| FB-20260630-052525 | intake | control | [folder](2026-06-30/052525-望和诗怀雅技能冲突-都是-7) | control stacking / max-of-same-effect | Wang and Swire both +7 conflict. Check whether same-effect max rule is missing or only display is wrong. |
| FB-20260630-052809 | intake | control | [folder](2026-06-30/052809-mon3tr换成其他的) | control filler / opportunity cost | Mon3tr should be replaced by another operator. Reproduce after FB-20260628-170359 because the root may be the same precedence issue. |

### P1/P2: Power Plant Cases

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260628-170330 | intake | power + linked producer | [folder](2026-06-28/170330-替换成绑定森蚺的承曦格雷伊) | power search / system link | Replace with Greyy the Lightningbearer bound to Eunectes. Needs linked-producer trace, not just raw power score. |
| FB-20260628-170710 | intake | power | [folder](2026-06-28/170710-小车类基础只有10-发电-优先级降低-最好是除非被唤起否则不用-凯尔希-f3的15-发电-仍然不如直接放一个常见的15-或者20-角色) | power priority / control synergy | Robots are weak by default and should not appear unless activated by synergy. Compare raw charge speed and opportunity cost. |

### P1/P2: Dormitory And Layout Metadata

| ID | Status | Area | Source | Suspected layer | Tracking note |
|----|--------|------|--------|-----------------|---------------|
| FB-20260627-090047 | intake | layout/trade | [folder](2026-06-27/090047-252缺少等级分布) | layout metadata / profile output | 252 lacks level distribution. Check frontend layout import and CLI profile JSON shape. |
| FB-20260628-132114 | intake | dormitory | [folder](2026-06-28/132114-宿舍会塞一些乱七八糟的干员) | dorm fill policy | Dorms get meaningless operators. Current non-goal excludes real dorm optimization, but filler should not imply false synergy. |
| FB-20260628-170828 | intake | dormitory/layout | [folder](2026-06-28/170828-3发电-宿舍应该是5级满级) | layout template / dorm level inference | 3-power layout dorms should be level 5. Verify layout generator and debug-bundle layout. |
| FB-20260628-171219 | intake | dormitory/layout | [folder](2026-06-28/171219-252-32贸易-33222制造的四间宿舍-等级应当是2级-1级-1级-1级) | layout template / dorm level inference | 252 3/2 trade and 3/3/2/2/2 factory dorm levels should be 2/1/1/1. |
| FB-20260628-171254 | intake | dormitory | [folder](2026-06-28/171254-乌尔比安此时完全无用-没有用深巡或者深海-宿舍不用塞乌尔比安) | dorm fill policy / synergy guard | Ulpianus is useless without Deep Sea context; dorm filler should not consume him as if active. |

## Closure Template

When closing a case, append the following to its tracking note:

```text
Closed by <commit>. Regression: <test or command>. Layer: <confirmed layer>.
```

If no code change is needed, write:

```text
Closed as expected behavior. Evidence: <command/output>. User-facing follow-up: <doc/output text if any>.
```
