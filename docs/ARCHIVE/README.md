# 归档

> 文档角色：current-reference
> 生命周期状态：current
> 当前真源：docs/文档生命周期.md
> 复核触发：scripts/codex/docs_inventory.py
> 转换自：docs/ARCHIVE/done/README.md；docs/ARCHIVE/plans/README.md；docs/ARCHIVE/superseded/README.md
> 转换处置：delete-after-absorb
> 事实映射：docs/文档生命周期.md#6-目录规则；docs/ARCHIVE/README.md#归档
> 摘要：说明归档目录角色和当前入口
> 源摘要：8a82e00570d4a91f0233e5503f314746e345dbd6f935e79be39206a78ec609b7
> 文档摘要：ef00ef896f204a391f7f7b2b3099e03cb5032b5485a86a31f5f14616bee2f1da
> 复核原因：source-change
> 复核结论：updated
> 稳定事实：说明归档目录角色和当前入口
> 证据引用：tracked:docs/ARCHIVE/README.md

本目录保存已完成、被替代和历史快照；归档材料不得作为 current truth。完整关闭与 transition record 规则见 [文档生命周期](../文档生命周期.md)。

| 子目录 | 内容 |
|---|---|
| `done/` | 已完成的 change 和实施记录 |
| `superseded/` | 被新方案替代或明确不采用的文档 |
| `plans/` | 历史设计和从根 `plans/` 迁入的计划 |
| `audits/` | 已被 current owner 取代的历史审计快照 |

归档文档只保留追溯价值。纯重复且没有独立决策、审计或证据价值的文件应在事实吸收和引用闭合后删除，而不是复制一份归档摘要。
