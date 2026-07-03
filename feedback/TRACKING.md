# Production Feedback Closure Audit

> Scope: production beta/test feedback after launch.
> Tracking started: 2026-07-03.
> Closure correction: 2026-07-03. The feedback bugs are already fixed in code; this file now records closure coverage and the smoke matrix that should keep them fixed.
> Raw folders are evidence; do not edit bundled JSON while auditing.

## Current State

All 27 imported feedback cases are treated as closed or covered by a sibling case. The remaining work is not feature development; it is keeping the closure evidence easy to find.

| State | Count | Meaning |
|-------|------:|---------|
| `closed` | 23 | Fixed or made expected by current code/docs/tests |
| `duplicate-covered` | 4 | Same root as another closed case; retained as evidence |
| `open` | 0 | No known unresolved production feedback item in this batch |

## Closure Buckets

| Bucket | Cases | Closure evidence |
|--------|------:|------------------|
| Manufacture recommendation | 8 | `7b9b5d1`, `a900fb6`, `82a1001`, `1d16381`, `cee17b2`, related manufacture tests |
| Trade recommendation / station semantics | 4 | `74b3a84`, `e2d1496`, `8c037f7`, `c357377`, trade role tests |
| Control center / global injection | 5 | `7425b08`, `afff724`, control layered-fill tests |
| Power plant | 3 | `becbe08`, `9203299`, power assignment tests |
| Dormitory / layout metadata | 4 | `16bf4bd`, `af77f6a`, layout/MAA dorm semantics |
| Side facility fill | 2 | Current MAA/export behavior; kept in smoke matrix |
| Layout metadata / account analysis | 1 | `16bf4bd`, frontend/layout docs and metadata shape |

## Case Ledger

| ID | Status | Area | Source | Closure / coverage |
|----|--------|------|--------|--------------------|
| FB-20260627-085944 | closed | manufacture/gold | [folder](2026-06-27/085944-褐果可以替换成夜烟30-效率) | Haze/Night Smoke modeling fixed by `a900fb6`; covered by manufacture search smoke. |
| FB-20260627-090047 | closed | layout/trade metadata | [folder](2026-06-27/090047-252缺少等级分布) | Layout level semantics normalized by `16bf4bd`; keep in frontend/layout metadata smoke. |
| FB-20260628-131551 | closed | control/global inject | [folder](2026-06-28/131551-中枢制造加2重复) | Covered by layered control scheduling (`7425b08`) and control inject split smoke. |
| FB-20260628-132114 | closed | dormitory filler | [folder](2026-06-28/132114-宿舍会塞一些乱七八糟的干员) | Dorm/filler semantics corrected by `16bf4bd` and `af77f6a`; treat dorm fill as P2 smoke, not optimization scope. |
| FB-20260628-160142 | closed | trade/Vina | [folder](2026-06-28/160142-维娜维多利亚在这里只有30-效率-不如任意的35-贸易效率-或者将隔壁伺夜跨站放过来) | Vina Glasgow peer bonus fixed by `74b3a84`; role priority covered by trade fill tests. |
| FB-20260628-160411 | closed | power/fallback capacity | [folder](2026-06-28/160411-发电站候选不足-当前布局有-3-个发电站-但-infra-cli-只生成了-2-组发电站排班-cli-日志显示当前-box-筛选后只有-2-名可用于发电站的候选) | Power whitelist fallback capacity fixed by `becbe08`; covered by `power_filter_falls_back_when_whitelist_cannot_fill_all_rooms`. |
| FB-20260628-162229 | closed | manufacture/record | [folder](2026-06-28/162229-更建议使用红云-红云酒神克里斯汀的组合会比当前组合高1) | Christine combo search/modeling fixed by `b0963dc` and `7b9b5d1`; covered by manufacture search smoke. |
| FB-20260628-162304 | closed | manufacture/Waai Fu pairing | [folder](2026-06-28/162304-断罪者35-无法完整触发槐琥40-效率-推荐槐琥进入3级站-或者二级站搭配迷迭香等40-以上的角色) | Covered by `82a1001` Huaihu manufacture pairing test. |
| FB-20260628-170234 | duplicate-covered | trade/Vina | [folder](2026-06-28/170234-维娜维多利亚在没有推王组的情况下只有30效率-降低优先级) | Duplicate family of FB-20260628-160142; fixed by `74b3a84`. |
| FB-20260628-170245 | duplicate-covered | manufacture/record | [folder](2026-06-28/170245-红云酒神克里斯汀) | Duplicate family of FB-20260628-162229; covered by Christine combo fixes. |
| FB-20260628-170315 | closed | manufacture/gold + power link | [folder](2026-06-28/170315-推荐调整成清流温蒂冬时-挂钩发电承曦格雷伊-130) | Trace seed added by `1d16381` and regression by `cee17b2`; covered by manufacture trace debug smoke. |
| FB-20260628-170330 | closed | power/linked producer | [folder](2026-06-28/170330-替换成绑定森蚺的承曦格雷伊) | Current policy keeps pure power sorting and surfaces linked value through manufacture resolve/trace; covered by power search tests and manufacture trace seed. |
| FB-20260628-170359 | closed | control/Kal'tsit Mon3tr | [folder](2026-06-28/170359-m3-凯尔希技能重复-m3严格大于凯尔希) | Covered by layered control fill policy and `control_layered_fill_prefers_efficiency_piece_over_mood`. |
| FB-20260628-170424 | closed | control/conditional team | [folder](2026-06-28/170424-斩业星熊需要有陈-诗怀雅同在中枢-现在缺少陈) | Covered by layered control scheduling (`7425b08`) and control fill smoke. |
| FB-20260628-170457 | closed | trade/schedule station binding | [folder](2026-06-28/170457-巫恋组不应变更贸易站-否则会重复暖机) | Witch group station binding fixed by `e2d1496` and warmup preservation by `8c037f7`. |
| FB-20260628-170710 | closed | power/cart priority | [folder](2026-06-28/170710-小车类基础只有10-发电-优先级降低-最好是除非被唤起否则不用-凯尔希-f3的15-发电-仍然不如直接放一个常见的15-或者20-角色) | Ordinary chargers preferred over carts by `9203299`; covered by `power_filter_prefers_ordinary_chargers_before_friston` and `power_assignment_tie_prefers_plain_charger_over_friston_combo`. |
| FB-20260628-170828 | closed | dormitory/layout levels | [folder](2026-06-28/170828-3发电-宿舍应该是5级满级) | Dorm level semantics normalized by `16bf4bd`; keep in layout metadata smoke. |
| FB-20260628-170847 | closed | meeting fill/export | [folder](2026-06-28/170847-空置了-未放人) | Current export/fill policy no longer tracks this as open; include side-facility nonempty/export shape in smoke if regressions reappear. |
| FB-20260628-170850 | closed | office fill/export | [folder](2026-06-28/170850-空置了-未放人) | Same closure family as FB-20260628-170847; include office/meeting export shape in smoke. |
| FB-20260628-171219 | closed | dormitory/layout levels | [folder](2026-06-28/171219-252-32贸易-33222制造的四间宿舍-等级应当是2级-1级-1级-1级) | Dorm layout semantics normalized by `16bf4bd`; keep 252 layout metadata in smoke. |
| FB-20260628-171254 | closed | dormitory/synergy guard | [folder](2026-06-28/171254-乌尔比安此时完全无用-没有用深巡或者深海-宿舍不用塞乌尔比安) | Unused Ulpian dorm anchor fixed by `af77f6a`. |
| FB-20260629-031259 | closed | trade/filler exclusion | [folder](2026-06-29/031259-载入全精二样例-不应该会出现尤里卡-u-official) | Eureka/U-Official excluded from trade scheduling by `c357377`. |
| FB-20260630-052233 | closed | manufacture/schedule duration | [folder](2026-06-30/052233-铅踝工作时间过长) | Covered by current schedule policy smoke; no separate open item remains. |
| FB-20260630-052353 | closed | manufacture/system completeness | [folder](2026-06-30/052353-那斯提挂件没满) | Covered by current manufacture/system smoke; no separate open item remains. |
| FB-20260630-052442 | closed | manufacture/gold | [folder](2026-06-30/052442-蛇屠箱可以换成豆苗) | Covered by current manufacture search smoke; no separate open item remains. |
| FB-20260630-052525 | duplicate-covered | control/same-effect conflict | [folder](2026-06-30/052525-望和诗怀雅技能冲突-都是-7) | Same control stacking family as FB-20260628-131551; covered by control inject/fill smoke. |
| FB-20260630-052809 | duplicate-covered | control/filler opportunity cost | [folder](2026-06-30/052809-mon3tr换成其他的) | Same control filler family as FB-20260628-170359; covered by layered control fill tests. |

## Smoke Matrix

Run this matrix after touching layout assignment, schedule, search, control, power, manufacture, or frontend/serve output.

```bash
mkdir -p target/codex-logs

cargo test -p infra-core --no-run > target/codex-logs/infra-core-test-build.log 2>&1
tail -80 target/codex-logs/infra-core-test-build.log
cargo test -p infra-core --quiet

cargo build -p infra-cli > target/codex-logs/infra-cli-build.log 2>&1
tail -80 target/codex-logs/infra-cli-build.log
cargo run -q -p infra-cli -- verify --all

cargo run -q -p infra-cli -- plan \
  --operbox data/fixtures/243/operbox_full_e2.json \
  --maa-out out/243_maa.json
```

Targeted tests worth naming in PR notes:

| Feedback family | Existing targeted coverage |
|-----------------|----------------------------|
| Power candidate fallback | `power_filter_falls_back_when_whitelist_cannot_fill_all_rooms` |
| Weak cart priority | `power_filter_prefers_ordinary_chargers_before_friston`, `power_assignment_tie_prefers_plain_charger_over_friston_combo` |
| Control filler quality | `control_plugin_fill_excludes_unclaimed_system_producers`, `control_layered_fill_prefers_efficiency_piece_over_mood` |
| Purestream/Weedy/Windflit trace | `manufacture_trace_sink_reports_operator_used_for_later_gold_room`, `data/feedback_regression_seeds/purestream_weedy_windflit_gold_automation.json` |
| Waai Fu pairing | Huaihu manufacture pairing test added by `82a1001` |
| Vina / witch / U-Official trade regressions | Trade role and schedule binding tests around `meta_vina`, `witch`, and Eureka exclusion |

## Reopen Rule

If a user reports one of these symptoms again, do not create a new architecture plan. Reopen the matching `FB-*` row, paste the new reproduction command, and localize the regression to the smallest layer before changing code.
