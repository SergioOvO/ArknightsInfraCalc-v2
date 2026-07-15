# AGENTS.md 路由器精简交接

> 状态：待实施
> 创建日期：2026-07-15
> 用户决定：先写交接文档，由用户更新项目文件后在下一次对话继续
> 范围：只精简根 `AGENTS.md` 的职责与篇幅；不修改业务规则、项目 Skills、验证脚本或代码
> 历史背景：完整工作流优化过程见 [`docs/ARCHIVE/done/AGENT_WORKFLOW_OPTIMIZATION_PLAN.md`](../ARCHIVE/done/AGENT_WORKFLOW_OPTIMIZATION_PLAN.md)，该文件是历史记录，不是本任务的实施清单

## 1. 目标

将根 `AGENTS.md` 从“维护、审计、验证、项目地图和业务提示的全文合集”收敛为：

```text
AGENTS.md = 任务路由器 + 所有任务都必须遵守的硬约束
项目 Skill = 可执行工作流
领域 Markdown = 业务语义真源
维护 / 质量 / 项目地图文档 = 详细规范与当前实现事实
```

不以机械行数作为唯一验收，但期望从当前约 200 行压缩到约 60～90 行。判断标准是：删除重复后，高风险门禁更醒目，Agent 仍能稳定找到详细流程。

## 2. 当前现场快照

下一次对话开始时必须重新检查现场；下面只记录本次交接时的状态：

- 当前 HEAD：`a21567e`。
- `main` 相对 `origin/main` ahead 17；尚未完成此前约定的一次性推送。
- `AGENTS.md` 当前为 `MM`，同时存在已暂存和未暂存的外部改动；这些改动不属于本交接任务，禁止直接覆盖。
- 已跟踪项目 Skills：
  - `.agents/skills/arknights-maintenance/`
  - `.agents/skills/arknights-system-audit/`
  - `.agents/skills/arknights-evidence/`
- 本次原计划使用的 `.agents/skills/refresh-agent-workflow/` 在执行前从共享工作区消失，当前不可用。用户会先更新项目文件；下一次对话应重新发现 Skill，不能依赖本次快照。

如果下一次对话发现 `AGENTS.md` 仍有无法归属的同文件改动，应先让用户确认或从明确 `base_sha` 建独立 worktree；不要把精简任务与他人的 AGENTS 修改强行混合。

## 3. 根 AGENTS.md 应保留什么

只保留每一个任务都必须立即看到、且不能可靠依赖后续路由才发现的内容。

### 3.1 项目状态

- 项目处于正常维护期。
- 默认目标是复现、缩层、修正最小责任边界、补回归并保持业务口径稳定。
- 不默认恢复历史 TODO、质量冲刺或完整心情排班。

### 3.2 真源优先级

用一个短列表保留：

1. 用户当前裁决优先。
2. 领域业务语义以对应 canonical Markdown 为准。
3. 代码和生成 help 证明当前实现事实。
4. JSON / CSV / fixture / Bake 是运行时载体，不裁决业务语义。
5. 两份当前领域 Markdown 冲突时停止并请用户裁决。

### 3.3 任务路由

根文件只说明触发条件和入口，不复制 Skill 内容：

| 任务 | 必须使用 / 首读 |
|------|-----------------|
| 普通 bug、CLI、solver、数据、局部排班修复 | `arknights-maintenance` |
| 体系、跨设施、required anchor、作用域、轮换绑定 | `arknights-system-audit` |
| build、test、CLI、benchmark、格式或结构验证 | `arknights-evidence` |
| 当前代码地图和命令事实 | `docs/PROJECT_MAP.md` |
| 当前文档路由 | `docs/INDEX.md` |

如果保留外部 Agent / `$HOME/AgentDocs` 研究入口，根文件最多保留一行路由；完整外部研究职责应进入对应 workflow Skill 或独立说明，不应在根文件展开主 Agent、项目 Skill、subagent、确定性工具的整张责任表。

### 3.4 所有任务通用的硬约束

建议保留为不超过 12 条的短列表：

- 未授权修改时只读；诊断不自动等于实现。
- 修改前先确认工作区，用户既有改动默认属于用户。
- 同一共享工作区同时最多一个写入者；其他 Agent 只读。
- 体系 / 编排修改前必须完成四项审计：不变量、违规位置、单一责任边界、删除清单。
- 不用 `shift_bind`、tag、priority 或 shortcut 代替 required admission。
- 不为 operbox、room id、班次下标、fixture 或当前 top hit 写特判。
- 新抽象没有第二个当前真实用例时默认不引入。
- 每个写入单元只有一个主不变量；旁支发现默认 deferred。
- 所有用于结论的验证必须通过统一证据工具留痕。
- 达到停止条件后停止扩张，不顺手重构或恢复历史 TODO。
- 只 stage / commit 本任务文件，不使用 `git add .`，不执行 destructive Git 操作。
- `target/codex-runs/`、兼容日志与 `out/` 证据不提交但保留到交付。

### 3.5 极短的硬模块边界

只保留容易造成严重跨层错误的五条，不再保留完整模块表：

- L1 interpreter 只认 `buff_id`，不认识干员名。
- L2 处理机制域，L3 shortcut 只结算组合，不负责体系选型或进编。
- Layout 主路径是 `build_plan -> execute_plan -> fill -> resolve`，排班不得绕过 Plan 修 admission。
- 生产搜索按各域 `final_efficiency`；中枢 heuristic 必须具名。
- CLI / export 不写机制、公式或重新选型。

其余模块和命令全部路由到 `PROJECT_MAP.md`、`MAINTENANCE_MODE.md` 和领域文档。

## 4. 应从根 AGENTS.md 迁出什么

下列内容不应继续全文保留在根文件：

| 当前内容类型 | 唯一去向 |
|--------------|----------|
| 普通 bug 的七步流程、复现矩阵、层级定位细节 | `arknights-maintenance` + `MAINTENANCE_MODE.md` |
| 四项审计的完整解释、生命周期问题表、formal-audit 等待点 | `arknights-system-audit` + `SYSTEM_AUDIT_WORKFLOW.md` |
| 日志 schema、full-suite 集合策略、manifest、完成证明表 | `arknights-evidence` + `QUALITY_AND_AUDIT.md` |
| 具体 CLI 命令和 `plan` 示例 | `MAINTENANCE_MODE.md` + `scripts/codex/README.md` |
| Bug 路由大表、模块职责、数据文件地图 | `INDEX.md` + `PROJECT_MAP.md` |
| 迷迭香、龙巫、叙拉古和动态 producer 的详细不变量 | 对应领域 Markdown；根文件只保留体系任务必须路由到 system-audit |
| 机制 / 排班 / 输出的详细改动策略 | maintenance / system-audit Skill |
| Rust 格式化展开说明 | 保留一句“修改 Rust 后运行 `cargo fmt --all`”，细节迁入维护 Skill |
| 外部 AgentDocs、研究 Agent 和项目角色的详细责任表 | 对应 workflow Skill 或独立研究说明 |
| 历史状态、当前失败数、性能快照、具体工作区交接 | 任务 manifest、交接文档或归档，不进入 evergreen 根规则 |

## 5. 建议的新 AGENTS.md 骨架

下一位实施者应按当前项目文件重新措辞，不要机械复制旧段落。推荐骨架：

```markdown
# Agent 引导

## 1. 当前模式
- 正常维护期、默认目标、非目标。

## 2. 真源与任务路由
- 用户 / 领域 Markdown / 实现事实 / 数据载体优先级。
- maintenance / system-audit / evidence Skill 路由。
- INDEX / PROJECT_MAP 入口。

## 3. 全局硬门禁
- 工作区所有权、单写入者、体系四项审计。
- 禁止特判、禁止 bind 代替 admission、单一不变量与停止条件。
- 验证留痕唯一入口。

## 4. 核心分层边界
- L1/L2/L3、Plan、搜索、CLI/export 的五条短规则。

## 5. Git 与交付
- status、显式 stage、独立 commit、证据不提交、destructive Git 禁止。
```

不要在新骨架后重新追加详细命令、体系摘要或完成证明全文，否则会恢复当前问题。

## 6. 实施步骤

1. 重新运行 `git status --short`，确定 `AGENTS.md` 当前 staged / unstaged 改动的所有权。
2. 完整读取三个项目 Skill、`MAINTENANCE_MODE.md`、`QUALITY_AND_AUDIT.md`、`SYSTEM_AUDIT_WORKFLOW.md`、`INDEX.md` 和 `PROJECT_MAP.md` 的相关责任段。
3. 建立“当前 AGENTS 段落 → 保留 / 路由目标 / 删除理由”矩阵；确保没有唯一硬规则被误删。
4. 只修改 `AGENTS.md`。如果发现目标文档确实缺少被迁出的唯一规则，应先报告 scope expansion，不默认顺手重写多个工作流文档。
5. 运行格式、链接、Skill 路径、文档影响和 task scope 检查。
6. 让只读 reviewer 检查：
   - 是否仍有重复大段；
   - 每个被删除段落是否有稳定路由；
   - 是否把关键门禁错误地下沉到可能不会触发的文档；
   - 是否新增了没有第二个真实用例的流程抽象。
7. 只提交本任务拥有的文件；如果 `AGENTS.md` 的外部改动无法可靠拆分，保持未提交并交回用户。

## 7. 验收标准

- 根文件显著缩短，且没有通过压缩句式把同样的信息密度藏在更长段落中。
- 普通维护、体系审计、证据验证三类任务均有唯一 Skill 入口。
- 四项审计、业务真源、单写入者、验证留痕、禁止样例特判、Git 所有权仍在首层可见。
- 根文件不再复制命令模板、完整生命周期问题表、完成证明表、bug 路由大表或具体体系规则。
- 所有链接和 Skill 路径存在；外部可选资产缺失时不阻塞本项目维护。
- `check_docs_impact.py`、`check_task_scope.py` 和最小文档 / 链接检查通过，并生成任务证据清单。
- reviewer 能用一段话回答：根文件负责什么，三个 Skill 各负责什么，详细业务规则在哪里。

## 8. 明确非目标

- 不修改任何明日方舟业务语义、代码、数据、fixture 或测试期待。
- 不顺便解决 full-suite 既有失败。
- 不重新设计 Agent 市场、模型路由、plugin 或外部研究系统。
- 不把三个项目 Skill 合并为一个万能 Skill。
- 不因为根文件变短而删除高风险门禁。
- 不处理上一轮发现的菲亚优先级、MAA displaced 宿舍强塞或动态 bind 问题；它们属于独立业务修复任务。

## 9. 下一次对话建议起始指令

用户可直接说：

> 按 `docs/TODO/AGENTS_ROUTER_SIMPLIFICATION_HANDOFF.md` 继续。先核对当前 `AGENTS.md` 的 staged/unstaged 所有权和项目 Skills，再只精简根 AGENTS；不要修改业务代码或顺手扩展工作流文档。
