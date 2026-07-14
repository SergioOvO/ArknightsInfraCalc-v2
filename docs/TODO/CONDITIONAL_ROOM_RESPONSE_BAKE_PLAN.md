# 条件化单房响应 Bake 计划

> 状态：ready-on-request，尚未实现。
> 目标：允许离线花费数分钟到数十分钟，完整预计算“单房组合在中枢有效效果下的真实
> solver 响应”，换取运行时简单、稳定且可验证的联合选型。
> 关联计划：[动态 Producer A+](DYNAMIC_PRODUCER_BAKED_SEARCH_PLAN.md)。本文只负责
> Bake 的物化边界、生成流程和运行时查询，不重新定义 producer 业务规则与 comparator。

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
