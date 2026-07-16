# 设施无关条件化响应 Bake 实施计划

> 状态：in-progress，2026-07-17 用户授权实施。
> 目标：允许离线花费数分钟到数十分钟，按机制依赖完整预计算“设施候选组合在相关外部
> 状态和跨设施摘要下的真实 solver 响应”，将标准全精二 243 warm `team-rotation` 压到
> 200ms 量级，同时保持 cache miss 只变慢、不换答案。
> 关联计划：[动态 Producer A+](DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。本文只负责
> Bake 的物化边界、生成流程和运行时查询，不重新定义 producer 业务规则与 comparator。
> 贸易是首个 vertical slice，制造是第二设施通用性验收，不复制平行 pipeline。

## 0. 2026-07-17 用户裁决与实施顺序

核心模型扩展为：

```text
CandidateRow
+ RelevantExternalStateSignature
+ RelevantCrossFacilitySummary
-> ConditionalResponse
```

外部名单必须按对目标 response 的完全等价状态归一化；不同 logical operator mask 即使响应
相同也只能共享 response，不能删除候选。状态空间通过机制依赖切片、真实可达签名、阈值/
封顶值域和 family 归一化压缩，不使用固定 top-K 或当前 winner 剪枝。

实施批次：

1. **验证控制面**：`BakeMode::Auto/Disabled/Required`；Required miss 硬失败；每次生成后
   严格校验 catalog 并运行现有机制回归；仓库正式生成入口随后运行完整 release test suite。
2. **依赖与规模编译**：建立设施无关 dependency slice，统计 CandidateRow、外部签名、摘要、
   response 数量、磁盘和内存；未测量前不冻结物理格式。
3. **贸易 vertical slice**：覆盖房间局部计数、跨房聚合和跨房阈值，强制 live/baked 差分。
4. **制造复用**：以 full-pool 和中枢到制造的外部影响验证公共 schema，不建第二套引擎。
5. **排班查询**：预排序 response 视图、operator-mask 精确 Join、peak/γ 共存签名和 winner
   完整 live resolve。
6. **发布与性能**：generation-id 临时目录、checksum、原子切换、失败保留旧 generation；
   标准 243 warm `team-rotation` p50 目标不高于 200ms。

当前已进入第 2 项，只实现依赖分析与规模报告，不改变候选集合、主路径 comparator、Plan
admission 或 rotation。

### 2026-07-17 依赖编译阶段

已新增只读 `profile bake-dependencies` 入口。依赖编译器对当前全部 `Selector`、`Condition` 和
`Action` 使用穷举匹配；新增枚举若未分类会导致编译失败。分类先保守区分房内、同设施、
跨设施、全局布局和运行时状态。全局资源 producer 暂时指向 `global_resource` 中间节点，
下一步根据 registry 和实际 consumer 展开多目标传递闭包，不把来源设施误当成最终目标。

首份报告中的具体 external 数量会随 Action 隐式依赖和全局资源边补全而更新；以命令生成的
最新 JSON 为准，不把早期 70 条观测作为冻结规模。
完整 JSON 产物用于下一步提取贸易/制造的最小充分签名和值域；本阶段尚未据此执行状态合并
或修改 Bake schema。

## 1. 核心模型

普通单房 Bake：

```text
RoomTeam -> RoomResult
```

本计划扩展为：

```text
RoomTeam
+ ControlEffectSignature
+ RoomFeatureSignature
+ CrossRoomSummary
-> BakedRoomResponse
```

这是单房 solver 的**条件化物化视图**。Bake 不保存完整中枢五人名单，也不保存完整多房
assignment；它只保存会改变目标房结算的充分状态。

运行时仍负责把多个单房响应连接成一个人员不冲突、跨房统计自洽的完整方案，并对最终
winner 执行一次真实 `resolve_base` 校验。

## 2. 为什么按效果签名 Bake

不同中枢组合可能对贸易结算完全等价。例如阿米娅、诗怀雅等不同名单最终都可能只产生
同族全贸易 `+7%`。若按五人名单 Bake，会重复生成大量完全相同的单房结果。

因此先把中枢组合投影为效果签名：

```text
ControlEffectSignature
  global_trade_flat
  active_deferred_rules
  karlan_precision
  named_trade_components
  semantic_version
```

两个中枢组合只有在对所有受支持单房 response 完全等价时，才能共享一个 signature。
心情、线索等不影响目标 solver 的差异不得进入贸易响应签名。

禁止使用 `package = vina_lungmen` 之类套餐名作为 key。签名必须来自已解析的 atom、
capability 和 resolved rule，不能重新引入 fixed package 选型。

## 3. 首期贸易维度

### 3.1 单房候选行

每个合法贸易房组合生成一条 `BakedTradeRow`：

```text
row_id
logical_operator_mask
variant_ids
room_level / capacity / order_kind
siracusa_count
glasgow_count
karlan_count
presence / shortcut capability
stable_tie_break_id
```

候选行不截固定 top-K。operbox、当前 `used`、plan anchor 和跨房姓名冲突在运行时过滤。

### 3.2 跨房摘要

首期摘要只保存三名 producer 真正需要的有限状态：

```text
siracusa_total
karlan_qualified_room_count
```

戴菲恩使用本房 row 自带的 `glasgow_count`，不把不同房间先求和。凛御银灰的达标数必须
由每房 `karlan_count >= 3` 推导。摘要值必须与最终选中 rows 重新核对，不能只相信查询 key。

### 3.3 响应内容

`BakedRoomResponse` 至少保存：

```text
row_id
control_signature_id
cross_room_summary_id
final_efficiency
unit_output
order_limit
mechanic_equivalent_efficiency
rule_id_id
breakdown_id
```

不得只保存一个匿名“组合生产力”。结构化字段用于 comparator、展示和最终 live 对账；
字符串、`rule_id` 与重复 breakdown 进入去重字典。

## 4. 离线生成

允许生成耗时数分钟到数十分钟。优化目标首先是完整性、确定性和可验证性，不以最短生成
时间为目标。

生成分为四步：

1. 枚举所有受支持 tier、房间等级、订单类型和合法单房成员组合，生成不可变 row。
2. 枚举由当前规则 registry 可达的去重中枢效果签名和有限跨房摘要。
3. 并行计算 `row x control signature x summary` 的真实 solver response。
4. 分片排序、归并、建索引、计算 checksum，完成后原子发布 generation。

每条 response 相互独立，适合使用 Rayon。worker 产生私有 chunk，避免争抢一个全局可变
`HashMap`；最后统一归并并分配稳定 id。

生成器必须记录：

- row、signature、summary 和 response 数量；
- 去重前后中枢组合/效果签名数量；
- 每阶段耗时、线程数、峰值内存和最终磁盘大小；
- solver 调用数、成功数、拒绝数与失败原因；
- 输入 hash、generator fingerprint 和 semantic model version。

## 5. 运行时查询

运行时流程：

```text
合法中枢组合 -> ControlEffectSignature
                          |
枚举有限 CrossRoomSummary |
                          v
各房预排序 BakedRoomResponse
          -> logical mask 互斥 Join
          -> operbox / tier / used / anchor 过滤
          -> 摘要自洽检查
          -> comparator 选择 winner
          -> 完整 assignment live resolve
          -> breakdown / rule_id 对账
          -> 一次 commit
```

八幡海铃和凛御银灰存在循环依赖：贸易成员决定摘要，摘要又改变各房排名。运行时按有限摘要
枚举情景，选出 rows 后检查实际摘要是否等于假设；不自洽的候选直接作废。

戴菲恩的跨 control prefix 比较按已确认口径使用各贸易房动态注入百分点之和。生产房内部
仍由真实 `final_efficiency` 和既定 role comparator 排序，不新增匿名综合分。

## 6. 多线程与文件布局

建议按 room signature 和 control signature 分片：

```text
manifest.json
operators.bin
control_signatures.bin
cross_room_summaries.bin
trade_rows.bin
responses/<room-signature>/<control-signature>.bin
indexes.bin
dictionaries.bin
```

并行任务以 response 分片为所有权边界。每个分片写入 generation 临时目录；全部 bytes、
row count、index checksum 和 catalog hash 校验通过后再原子切换。读取方不得观察到新旧分片
混用。

运行时可以只映射当前 layout、room signature 和 control signature 所需分片，不要求一次
加载全部 response。

## 7. 明确不 Bake 的状态

即使离线时间充足，也不物化以下高维运行时状态：

- 完整中枢五人名单；
- 完整 operbox；
- 任意 `used` 子集；
- 完整多房 assignment；
- alpha/beta/gamma 班次排列；
- 不影响目标 solver 的心情、线索或展示字段；
- 所有设施人物的全局 `2^N` mask。

这些维度缺乏跨账号复用价值，或可由 logical mask 和运行时过滤廉价处理。允许多算不等于
保留完全等价的重复状态。

## 8. Cache gate 与后备

catalog 至少校验：

- schema、semantic model version、generator fingerprint；
- skill table、operator instances、shortcut、segment、standalone roster 和 producer registry；
- operator/variant dictionary；
- room signature、tier、order kind 和 Bake options；
- 每个分片的 bytes、hash、row count 和 index checksum。

任一 row、signature、variant 或 response miss 时，必须用同一规则现场调用真实 solver，并
继续进入同一运行时 Join。miss 只能增加耗时，不能缩小候选集合或改用旧 top-K 路径。

最终 winner 的 live resolve 若与 baked response 任一关键字段不一致，本次查询拒绝整个
catalog generation 并用 live reference 重跑，不能只修正 winner 分数。

## 9. 实施阶段

### Phase 0：规模测量

- 统计完整 trade rows、可达 control signatures 和 summaries；
- 用实际字段宽度估算 response 数、生成时间、磁盘和内存；
- 保存当前 Bake、`plan` 和 joint live reference 基线。

### Phase 1：稳定 row/signature schema

- 建立 operator/variant/signature 字典；
- 保证效果签名由通用 rule/capability 提取；
- 用等价与不等价中枢组合测试签名去重。

### Phase 2：并行 response 生成

- 用真实贸易 solver 生成完整结构化 response；
- 实现 chunk、归并、分片和 generation manifest；
- 不接入默认运行时。

### Phase 3：运行时 Join 与 live 对账

- operbox/used/mask 过滤；
- 摘要情景枚举与自洽检查；
- 多房互斥 Join；
- winner 完整 assignment live resolve。

### Phase 4：差分验证后启用

- 强制 live 与强制 Bake 在最小盒、full-E2、混合 tier、非标准 layout 上逐项一致；
- 注入损坏分片、旧 hash、未知 variant 和 response miss；
- 只有 winner、breakdown、`rule_id` 和 dependency 全部一致后才启用默认快路径。

## 10. 验收门槛

1. Bake 与 live 使用同一候选集合、规则 registry 和 comparator。
2. 三 producer 的 0/1/多 consumer、两两共存、三者共存和全部落败均有差分测试。
3. 戴菲恩 `(3,0)` 与 `(2,1)` 不混淆；八幡海铃总数、凛御银灰逐站阈值均自洽。
4. 任意两个房间、任意中枢与房间之间 logical mask 无交集。
5. cache miss、损坏或不兼容只变慢，不换答案。
6. 最终 live resolve 与 baked `final_efficiency`、unit output、limit、breakdown、`rule_id`
   完全一致。
7. 记录完整生成耗时、线程数、响应数量、磁盘、内存、加载时间和运行时命中率。

## 11. 后续扩展

制造域可复用同一模型：

```text
ManuRoomTeam
+ ControlEffectSignature
+ RoomLocalFactionCount
+ CrossFactorySummary
-> BakedManuRoomResponse
```

涤火杰西卡是首个验证样例，但必须在贸易版稳定后单独审计并实施。灵知的负效率/订单上限
response 也可进入贸易 catalog，但必须先裁决其 comparator。不得因为 schema 能表达，就在
本计划中顺手改变尚未确认的业务选型。

## 12. 非目标

- Bake 完整全基建排班或班次组合；
- 用套餐名、干员名分支替代 effect signature；
- 用 Bake 结果替代最终完整 assignment resolve；
- 为减少生成时间使用固定 top-K 或近似剪枝；
- 在没有规模测量前承诺具体秒数、内存或磁盘上限；
- 同时重做贸易、制造、发电和轮换全部域。
