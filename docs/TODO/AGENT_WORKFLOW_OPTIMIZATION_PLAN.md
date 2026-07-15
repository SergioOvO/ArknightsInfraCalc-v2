# Agent 工作流优化实施计划

> 状态：ready-on-request
> 适用范围：本仓库的 Codex / subagent 开发、维护、审阅与验证工作流
> 目标读者：下一位负责实施工作流优化的主 Agent
> 性质：流程与工具建设计划，不是业务功能计划；用户已明确要求下一会话开始实施
> 业务边界：不得借本计划修改明日方舟机制、评分口径、编排语义或当前进行中的业务实现

## 1. 为什么需要这次优化

本项目的现有工作流已经能有效阻止多类高风险错误：把 `shift_bind` 当成进编保证、依赖当前 top hit 或房间顺序、为单个 operbox 写特判、让旧测试反向定义业务语义、只跑单元测试就宣称排班已修复等。这些门禁来自真实问题，不能为了追求速度而删除。

当前瓶颈不是缺少规则，而是事故恢复期积累的规则尚未经过第二次整理：

1. 相同的业务真源、四项审计、生命周期、验证留痕和 Git 纪律分散在多个首读文档中。
2. 永久工作流文档混入了某一时刻的进度、性能数字和未提交工作区快照，已经出现失真。
3. `run_logged`、full-suite 失败集合比较和最终证据链接仍依赖每个 Agent 手工复制、执行和整理。
4. 普通 bug、体系 bug、正式逐项审计和大型功能建设之间缺少显式模式切换，导致审批和 subagent 使用规则存在重叠。
5. 默认测试长期存在既有失败，每次 full suite 都要人工证明失败集合没有变化。
6. 只读调查、结构化提取、实现、验证和审阅的责任边界已有雏形，但尚未由项目 Skills 与机械工具串成稳定闭环。

这次优化的目标是降低上下文、验证和交接成本，同时保持现有领域正确性门禁不变。

## 2. 审计快照

以下数据只是 2026-07-15 的审计快照，用于解释本计划的优先级，不得复制到新的永久工作流中作为长期事实：

| 项目 | 审计时状态 |
|------|------------|
| 根 `AGENTS.md` | 约 25 KiB / 356 行 |
| `MAINTENANCE_MODE.md` | 约 14 KiB / 314 行 |
| `QUALITY_AND_AUDIT.md` | 约 10 KiB / 184 行 |
| `SYSTEM_AUDIT_WORKFLOW.md` | 约 19 KiB / 244 行 |
| `PROJECT_MAP.md` | 约 25 KiB / 341 行 |
| 上述五份首读或高频文档合计 | 约 94 KiB |
| `target/codex-logs/` | 578 个 `.log`、365 个 `.status` |
| `out/` | 55 个文件 |
| 已保存的 full-suite baseline 示例 | 426 passed、20 failed、约 94 秒 |
| 审计时未提交业务改动 | 18 个文件，约 `+1278/-134` |

审计过程中工作区仍在被其他任务修改：开始时 `crates/infra-core/src/export/maa.rs` 为 modified，结束时已经恢复；`team_rotation.rs` 的 diff 也发生变化。这说明共享目录并发写入不是理论风险。实施本计划前必须重新读取现场状态，不得把上表当作当前工作区清单。

已确认的文档漂移包括：

- `PROJECT_MAP.md` 仍把已经移除的 `layout rotation` 列为 `layout.rs` 子命令。
- `PROJECT_MAP.md` 仍使用 `Cursor` 作为 Agent / 数据维护者称呼。
- `PROJECT_MAP.md` 把 `data/` 称作“运行时真相源”，容易与“领域 Markdown 是业务真源”混淆。
- `SYSTEM_AUDIT_WORKFLOW.md` 的 Git 交接节记录了某次工作区只有两个未提交文件，当前已经失真。
- 根 `AGENTS.md` 允许用户明确要求直接实现时在四项审计后编辑；`SYSTEM_AUDIT_WORKFLOW.md` 则对所有体系任务要求两次等待和必须调用 subagent，适用范围没有区分。

## 3. 必须保留的高价值门禁

实施者不得把下列内容当作“流程冗余”删除。

### 3.1 领域业务真源

业务语义的裁决顺序保持为：

1. 用户在当前对话中的明确裁决。
2. 当前领域的规范性 Markdown。
3. 实现载体、代码、注释、测试、fixture 和历史输出。

需要补充的只是“真源类型”区分：

| 真源类型 | 负责内容 | 首选证据 |
|----------|----------|----------|
| 领域规范 | 干员机制、体系硬核心、作用域、降级、互斥、班次语义 | 用户裁决与对应领域 Markdown |
| 实现事实 | 当前 CLI 是否存在、类型签名、模块位置、实际调用链 | 当前代码、生成的 `--help`、构建产物 |
| 流程规范 | Agent 如何复现、修改、验证、审阅和提交 | `AGENTS.md`、维护与质量工作流 |
| 运行时载体 | 技能、实例、规则、fixture 和 Bake 数据 | JSON / CSV / 二进制数据及其校验 |

代码不能推翻领域规范，但可以证明描述性项目地图已经过时。描述性 Markdown 与当前代码冲突时，应修正文档，而不是要求代码迁就过时地图。

### 3.2 体系 / 编排修改前四项审计

编辑体系、跨设施、编排或轮换代码前必须说明：

1. 领域不变量。
2. 违规生命周期和具体位置。
3. 修复后的单一责任边界。
4. 将删除或改写的冲突路径。

这一门禁防止“最小修复”退化成最下游条件分支，必须保留。

### 3.3 完整生命周期和反例

涉及体系的规则必须继续沿以下生命周期检查：

```text
select -> plan -> execute -> fill -> resolve -> rotation -> export
```

必须继续使用删核心、最低人数、竞争候选、房间重排、跨站容量、休息班关闭等反例证明结构保证，不能只看标准全精二 top hit。

### 3.4 用户真实入口与不变量回归

排班和导出问题必须继续至少运行一次用户实际入口。单元测试不能替代 `plan` 或 `layout team-rotation`。回归必须断言激活、关闭、成员、配方、作用域、轮换和导出关键字段，而非只有最终效率快照。

### 3.5 Git、隐私和工作区隔离

必须继续遵守：

- 开始和结束检查工作区。
- 用户既有改动默认属于用户。
- 不使用 `git add .`。
- 不提交私有 operbox、xlsx 或 debug bundle。
- 只提交本任务拥有的文件。
- 无法可靠拆分同文件改动时不强行提交。

### 3.6 Bake 同语义回退

Bake 只是加速载体。schema、catalog、context 或内容不兼容时，只能回到同一候选集和同一语义的 live 求值，不得把 cache miss 变成更小池、固定 top-K 或旧 pipeline。

## 4. 明确不在本计划范围内

- 不修改任何明日方舟领域规则、效率公式、评分 policy 或体系定义。
- 不继续历史 Phase 或 `QUALITY_90_TO_95_PLAN.md`。
- 不趁机拆分 `team_rotation.rs`、`rules.rs` 等大文件；文件大不是本次流程优化的充分理由。
- 不引入重型项目管理系统、长期 Agent memory、CODEOWNERS 或复杂 PR 仪式。
- 不一次安装大量第三方 Agent、Skill、Hook 或插件。
- 不让 Hook、脚本或 subagent 自动裁决 Markdown 冲突和业务语义。
- 不要求所有普通修改都使用 formal audit、两个审批点或独立 reviewer。
- 不在本计划第一批实施中顺手清理现有 full-suite 业务失败；失败债务需要单独审计。

## 5. 目标工作流

### 5.1 任务模式

每个任务开始时由主 Agent 选择一种模式，并在 commentary 或任务简报中明确：

| 模式 | 触发条件 | 默认动作 |
|------|----------|----------|
| `research` | 解释、调查、比较、审计，但未授权修改 | 只读；可并行 explorer / extractor；不创建实现计划 |
| `maintenance` | 普通 bug、结果不对、CLI / 数据 / solver 局部修复 | 复现、缩层、最小责任边界、定向回归、真实入口 |
| `system-fix` | 体系、跨设施、编排、轮换 bug，且领域 Markdown 已清楚 | 四项审计后实施；只有真实语义未知时询问用户 |
| `formal-audit` | 用户明确要求逐项严格审计，或当前 Markdown 互相冲突 | 审计报告、用户裁决、计划、用户批准、实施和主审 |
| `feature` | 用户明确恢复功能建设、TODO 或大型重构 | 任务简报 / spec、计划、分批实施和阶段验收 |

模式只决定流程半径，不改变业务真源。普通 `system-fix` 不应自动升级为 formal audit。

### 5.2 优化后的主路径

```text
用户目标
  -> 主 Agent 选择任务模式并定义完成标准
  -> 必要时让只读 Agent 调查或提取
  -> 主 Agent 汇总事实、未知项和决策边界
  -> 只有业务语义冲突才请求用户裁决
  -> 一个写入者实施一个独立责任边界
  -> 自动化证据工具运行定向验证
  -> 高风险任务交给只读 reviewer 检查真实 diff 与证据
  -> 主 Agent 运行或确认用户真实入口
  -> 自动生成证据清单并提交本任务文件
```

### 5.3 向用户升级的事项

主 Agent只应因以下问题暂停并请求用户：

- 两个当前领域 Markdown 互相冲突。
- 需要决定业务硬核心、作用域、降级、互斥或班次语义。
- 会改变用户可见产品行为或长期 scoring policy。
- 会造成不可逆兼容性变化、隐私风险或现实责任变化。

代码组织、局部接口、测试选择、日志命名和可逆实现细节由主 Agent 自主决定。

### 5.4 防止过度设计的判断门槛

工作流中增加两条通用判断：

1. 引入新抽象前，必须指出第二个当前真实用例；只有一个用例时优先直接表达。
2. 反例只有在可能改变当前选择、责任边界或完成标准时展开；真实但当前无关的风险记录为未知项，不罗列成阻塞理由。

## 6. 文档职责重整

### 6.1 根 `AGENTS.md`

目标：只保留每个任务都必须加载的内容，不再充当所有领域和验证细节的全文副本。

应保留：

- 维护期默认状态与非目标。
- 首读路由。
- 四项审计的短定义和触发条件。
- 业务真源的短定义。
- 核心模块硬边界。
- Git / 隐私 / destructive command 规则。
- 验证必须留痕的硬要求和唯一工具入口。
- 当前少量无法通过路由可靠触发的最高风险不变量。

应迁出或改为链接：

- 完整 `run_logged` Bash 实现。
- 各验证矩阵的重复命令。
- formal audit 的完整生命周期问题表。
- 具体体系的详细硬核心、producer 和降级规则。
- 当前进度、性能快照、失败数量和交接时工作区状态。

不以行数作为验收目标；以“删除重复后仍能阻止已知高风险错误”为准。

### 6.2 `docs/MAINTENANCE_MODE.md`

唯一负责：

- 收集 bug 输入。
- 用户入口复现。
- CLI → layout → search → solver → data 分层定位。
- 与改动半径匹配的回归选择。
- 普通 maintenance / system-fix 的完成路径。

不再全文重复验证日志 schema，只链接质量文档和证据工具。

### 6.3 `docs/QUALITY_AND_AUDIT.md`

作为验证与完成证明的单一文字真源，唯一负责：

- 回归分层。
- build / targeted / full / CLI / 性能 / JSON 的证据要求。
- full-suite baseline 与失败集合语义。
- Bake 安全门禁。
- 完成证明表。
- 证据工具使用说明。

其他文档不再复制这些段落。

### 6.4 `docs/SYSTEM_AUDIT_WORKFLOW.md`

限定为 `formal-audit` 模式，唯一负责：

- 一次一个体系的正式审计。
- 两个用户等待点。
- 完整生命周期问题表。
- 审计报告和修改计划格式。
- formal audit 中 subagent / reviewer 的使用方式。

应移出或删除：

- 当前工作区未提交文件清单。
- 易变化的测试数量和性能数字。
- 已完成体系的实时进度表。
- 普通 bug 也必须两次审批或必须 subagent 的暗示。

若需要保留体系审计进度，可放入独立状态文档；若只是历史交接和案例，应移入 `docs/ARCHIVE/`。不要让 evergreen workflow 同时承担工作流、状态表、案例库和交接快照。

### 6.5 `docs/PROJECT_MAP.md`

目标：描述当前实现事实，不裁决业务语义。

第一批必须修正：

- 删除已移除的 `layout rotation`。
- 把 `Cursor` 改为中性的 Agent / 维护者描述，或删除易过时的“谁维护”列。
- 把 `data/` 的“运行时真相源”改成“运行时实现载体”。
- 检查命令列表与实际 `--help` 一致。

未来新增 docs lint 或 CLI help 检查，防止相同漂移再次出现。

## 7. 验证证据工具

这是本计划最高收益的实现部分。不要先创建 `luna-verifier`；机械验证应优先由脚本保证。

### 7.1 统一命令包装器

建议创建可直接执行的仓库脚本，而不是要求 Agent 每次复制 shell function：

```text
scripts/codex/run_evidence.sh
```

建议接口：

```bash
scripts/codex/run_evidence.sh \
  --task <task-slug> \
  --category targeted-test \
  --stem <short-name> \
  --inputs '<可复现输入说明>' \
  -- <command> [args...]
```

每次调用必须：

- 使用参数数组执行命令，不通过 `eval` 重新解析。
- 生成唯一目录或文件名，不覆盖旧结果。
- 保存完整 stdout + stderr。
- 记录 cwd、完整 shell-escaped 命令、输入、开始和结束时间、耗时、exit code、PASS / FAIL。
- 在终端只打印简短状态与证据路径。
- 返回被执行命令的原始 exit code。
- 生成机器可读的 manifest 条目。

建议目录：

```text
target/codex-runs/<task-slug>/<run-id>/
  manifest.json
  commands/
    <timestamp>-<category>-<stem>.log
    <timestamp>-<category>-<stem>.status
  reports/
  artifacts.json
```

为了兼容现有绝对链接和忽略规则，也可以继续把 `.log` 放在 `target/codex-logs/<task-slug>/`；关键是由脚本统一生成并提供 manifest，不要求实施者为了目录美观同时迁移全部历史证据。

### 7.2 失败集合比较

建议创建：

```text
scripts/codex/compare_test_failures.py
```

职责：

- 从 Cargo test 完整日志提取测试全名集合。
- 输出 baseline、current、added、removed、unchanged。
- 只比较数量不算通过。
- 写出机器可读 JSON 和人类可读 Markdown / text。
- 新增失败时返回非零；集合相同或只减少时按明确 policy 返回结果。
- 日志格式无法解析时硬失败，不返回空集合假装成功。

脚本本身需要 fixture 覆盖：无失败、同集合、增加、减少、重复名称、日志截断和格式不匹配。

### 7.3 证据清单生成器

建议创建：

```text
scripts/codex/render_evidence.py
```

从 manifest 生成最终回复可直接使用的 Markdown：

```markdown
### 验证证据

- Build：[日志](...)
- 定向测试：[日志](...)
- Full suite：[日志](...)；[失败集合比较](...)
- 真实 CLI：[日志](...)
- 生成 JSON：[profile](...)；[MAA](...)
- 性能：未跑（原因）
```

要求：

- 类别没有运行时显式生成“未跑”，不能静默缺失。
- 检查链接目标实际存在。
- 检查日志 exit code 与 manifest 一致。
- 支持登记 `out/` 中的 profile、MAA 和其他任务产物。
- 不根据文件名猜测通过，必须读取 manifest / status。

### 7.4 证据生命周期

需要明确当前规则未定义的结束点：

- 证据至少保留到任务交付完成。
- 交付后可以按任务目录归档或清理，不要求 `target/` 永久保存。
- 如果某份证据具有长期审计价值，应保存摘要或稳定 artifact，而不是依赖永远不会被 `cargo clean` 删除的假设。
- 不在这次优化中批量删除现有日志和 `out/` 文件。

## 8. 项目 Skills

项目 Skills 使用 Codex 项目级发现目录：

```text
.agents/skills/
```

首批只创建三个，不安装大合集。

### 8.1 `arknights-maintenance`

触发：bug、结果不对、跑一下、CLI / solver / layout / schedule / export 修复。

负责：

- 选择 `maintenance` 或 `system-fix`。
- 收集输入并复现。
- 按症状缩小责任层。
- 选择最小回归和用户真实入口。
- 路由到维护、项目地图和相关领域文档。

不负责：

- 自动修改业务 Markdown。
- 自动裁决体系语义。
- 启动历史 TODO。

### 8.2 `arknights-system-audit`

触发：体系、硬核心、producer、同房 / 跨站、required anchor、轮换绑定、正式逐项审计。

负责：

- 加载四项门禁。
- 路由当前体系对应的领域 Markdown。
- 生成不变量表、生命周期违规表、单一责任边界和删除清单。
- 区分 `system-fix` 与 `formal-audit`。

不负责：

- 在 Markdown 冲突时选择一种解释。
- 根据当前 top hit 反推业务规则。
- 自动决定 full suite 失败中的旧测试是否错误。

### 8.3 `arknights-evidence`

触发：任何需要用 build、test、CLI、benchmark、格式或结构校验支持结论的任务。

负责：

- 调用统一证据工具。
- 登记输入与产物。
- 比较 full-suite 失败集合。
- 生成最终验证证据 Markdown。

不负责：

- 判断业务输出是否符合领域 Markdown。
- 决定某个测试是否应删除。
- 用“脚本成功”替代主 Agent 对实际结果的审阅。

Skills 应采用渐进加载：`SKILL.md` 保持触发、流程和边界清楚，长表格、命令说明和脚本协议放在 references / scripts 中。不要把现有四份文档原样复制进 Skill。

## 9. Agent 角色与模型路由

现有项目级 Agent：

| Agent | 保留理由 | 使用边界 |
|-------|----------|----------|
| `terra-explorer` | 适合未知调用链、责任层和大范围只读扫描 | 返回文件级证据；不实现、不裁决业务语义 |
| `luna-extractor` | 适合日志、表格、文档差异和稳定字段提取 | 输入输出必须明确；不做开放式架构判断 |
| `sol-reviewer` | 适合高风险跨生命周期变更的最终反方审阅 | 只读检查真实 diff 和证据；不为了批判罗列通用风险 |

首批不新增：

- `luna-verifier`：验证执行和证据整理应先脚本化。
- 大量语言 / 框架专家：本项目瓶颈是领域边界，不是缺少“Rust 专家”人设。
- 常驻 PM / Architect Agent：产品和领域裁决仍由用户与主 Agent承担。

实现任务默认可使用 Codex 内置 `worker`。只有内置 worker 在多个真实任务中反复违反本项目边界，才新增 `terra-implementer`。届时它也必须：

- 一次只拥有一个清晰任务和文件边界。
- 先读任务简报与适用 Skill。
- 在共享目录中不得与另一个写入者并发。
- 遇到语义冲突返回 `NEEDS_CONTEXT`，不自行猜测。
- 输出修改摘要、验证证据、疑虑和状态，不用自我评价代替主审。

建议路由：

| 任务 | 推荐 |
|------|------|
| 大文件扫描、调用链和支持材料 | Terra / high |
| 日志、失败列表和文档矩阵 | Luna / medium |
| 普通边界明确实现 | worker 或 Terra / high |
| 普通代码审阅 | Terra / high 或主 Agent |
| 体系、跨站、轮换、公开 API 高风险审阅 | Sol / xhigh |

不要让未指定模型的 subagent 自动继承当前最昂贵配置；每次派发都应显式选择已有 Agent 或模型档位。

## 10. Subagent 协作协议

### 10.1 适合并行的任务

- 不同文档或模块的只读调查。
- 日志和失败集合提取。
- 测试缺口、维护性和领域不变量的独立审阅。
- 互不依赖的外部一手资料核对。

### 10.2 不适合共享工作区并行写入的任务

- 同时修改 `team_rotation.rs`、`rules.rs`、`orchestration_rules.json` 等共享热点。
- 一个 Agent 改 plan schema，另一个 Agent 同时改 execute / rotation 消费者。
- 多个 Agent 同时更新同一领域 Markdown 或 fixture。
- 未确认当前 dirty files 所有权时继续在主工作区写入。

### 10.3 写入任务隔离

- 工作区干净且只有一个写入者：可以在当前分支实施。
- 工作区已有其他任务改动：优先建立独立 worktree，从明确 `base_sha` 开始。
- 无法使用 worktree：停止新增写入者，由当前主 Agent完成或等现有任务提交。
- reviewer 默认只读，读取任务简报、base/head diff package 和验证 manifest。

### 10.4 任务简报

较大任务在派发实现者前应有最小简报，保存位置可以是任务证据目录；只有跨会话任务才需要提交进仓库。

必须包含：

- 目标和用户可观察结果。
- 已确认事实、当前假设和关键未知项。
- 领域不变量。
- 文件与权限边界。
- 明确非目标。
- 完成标准、回归与真实入口。
- 实现者报告格式。
- `base_sha` 和任务 slug。

## 11. 默认测试恢复绿色

现有 full suite 长期红色使每次验证都需要人工 baseline 集合比较。此问题收益很高，但业务风险也高，应作为独立任务实施，不与文档收敛或证据脚本混在同一提交。

### 11.1 先分类，不直接批量修改

对每个既有失败记录：

| 字段 | 含义 |
|------|------|
| 测试全名 | 稳定标识 |
| 责任层 | data / solver / search / layout / rotation / export |
| 失败类型 | 真 bug / 错误旧语义 / 尚未实现 / fixture 污染 / flaky / 未知 |
| 领域依据 | 对应 Markdown 与用户裁决 |
| 处置 | 修复实现 / 改写测试 / 删除测试 / `#[ignore]` / 隔离 suite |
| 关闭证据 | 定向测试、full suite、真实入口 |

### 11.2 目标状态

- 默认 `cargo test -p infra-core` 退出 0。
- 尚未实现但仍有价值的规范测试明确 ignored，并包含原因或跟踪入口。
- 需要特定环境或重型数据的测试进入独立 suite。
- 不再依靠“固定有 20 项失败”证明没有回归。
- full suite 恢复绿色前继续使用自动 failure-set compare，不降低现有门禁。

## 12. 最小机械检查与 CI

仓库当前没有 `.github/workflows`。首批无需建立重型 CI；应在证据工具和默认绿色子集稳定后增加最小检查：

1. `cargo fmt --all -- --check`。
2. `cargo test -p infra-core --no-run` 或等价 compile gate。
3. 已确认稳定且快速的 smoke / data consistency tests。
4. Markdown 链接与文档状态字段检查。
5. CLI help 与项目地图中的命令名漂移检查。

Hook 只用于机械规则，且应在脚本稳定后再考虑：

- 阻止 `git add .`。
- 对裸跑 `cargo test` 给出证据包装器提示或门禁。
- 阻止明显 destructive git 命令。

Hook 不得判断业务语义、选择测试期待或决定某个实现是否过度设计。

## 13. 实施批次

每一批形成独立提交，完成后再进入下一批。不要在一个超大提交中同时重写文档、引入工具、迁移日志、建立 Skills、清失败测试和增加 CI。

### 批次 A：修正文档事实与流程冲突

范围：

- 修正 `PROJECT_MAP.md` 的已移除命令、`Cursor` 和数据真源表述。
- 删除 `SYSTEM_AUDIT_WORKFLOW.md` 的瞬时工作区快照。
- 明确 `system-fix` 与 `formal-audit` 的审批差异。
- 不移动大量文档，不改变业务规则。

验收：

- 文档与当前 CLI / 代码事实一致。
- 用户明确要求直接修复且 Markdown 无歧义时，不再被要求两次等待。
- formal audit 仍保留两个用户裁决点。

### 批次 B：证据工具

范围：

- 实现统一命令包装器。
- 实现失败集合比较。
- 实现证据 Markdown 生成器。
- 给脚本增加自测 fixture。
- 使用旧 `run_logged` 为工具首轮验证留痕，再用新工具完成一次自举验证。

验收：

- PASS、FAIL、信号退出、命令不存在都保留正确 exit code。
- 并发运行不覆盖文件。
- 含空格、中文和特殊字符的命令 / 输入可以被准确记录。
- failure compare 不会把截断日志解析成空集合成功。
- evidence renderer 能发现缺失文件、状态不一致和未跑类别。

### 批次 C：单一文档真源与 Skills

范围：

- 收敛根 `AGENTS.md`。
- 让维护、质量、formal audit 文档各自只承担一个职责。
- 创建三个项目 Skills。
- 更新文档路由。

验收：

- 验证 schema 和完整命令模板只有一个文字真源或脚本真源。
- formal audit 的进度和历史案例不再与工作流混写。
- 普通 bug 不会加载完整体系审计材料。
- Skill description 能区分 maintenance、system audit 和 evidence。
- 删除重复后，四项审计、真实入口、Git / 隐私、Bake fallback 等硬门禁仍可从根规则明确到达。

### 批次 D：默认失败债务

作为单独领域审计任务执行：

- 固化当前失败集合和对应领域依据。
- 逐项分类、裁决和关闭。
- 恢复默认绿色 suite。

不得为了得到绿色退出码批量放宽断言、删除未知失败或给全部失败加 `ignore`。

### 批次 E：最小 CI / Hook

只有批次 B、C 稳定，且默认绿色子集明确后再实施。CI 和 Hook 的目标是机械执行已稳定规则，不引入新的审批层。

## 14. 每批实施者的修改前门禁

即使本计划已获用户批准，每批开始前仍必须输出简短审计：

1. 本批唯一目标。
2. 会修改的文件和不修改的业务层。
3. 将删除的重复或过时路径。
4. 本批验证与回滚方式。

这不是要求再次等待批准；只有发现本计划与当前用户指令、当前业务 Markdown 或现场代码发生实质冲突时才暂停。

## 15. 完成标准

整个工作流优化只有在以下条件满足后才算完成：

- [ ] 根 `AGENTS.md` 不再复制完整验证脚本和 formal audit 全文。
- [ ] 维护、质量、formal audit 和项目地图各有清晰且不重叠的责任。
- [ ] 永久工作流中不存在某次工作区未提交文件快照。
- [ ] `PROJECT_MAP.md` 的 CLI 列表与实际入口一致。
- [ ] 业务规范、实现事实、流程规范和运行时载体的真源类型已区分。
- [ ] 普通 `system-fix` 与 `formal-audit` 的等待点已明确区分。
- [ ] 验证命令不再要求 Agent 复制 `run_logged` 函数。
- [ ] full-suite failure set 可以自动比较并生成可追溯报告。
- [ ] 最终验证证据 Markdown 可以从 manifest 自动生成。
- [ ] 三个项目 Skill 可被 Codex 发现，并具有清晰触发边界。
- [ ] read-only explorer / extractor / reviewer 与写入者的职责不重叠。
- [ ] 一个共享工作区同一时间最多一个写入者；并行写入使用独立 worktree。
- [ ] 默认 full suite 的既有失败已被逐项分类；恢复绿色属于完成目标，但可作为单独批准的最后批次交付。
- [ ] 所有改动只改变协作与验证方式，不改变任何业务结果。

## 16. 完成后的理想日常形态

普通 bug：

```text
用户报告
  -> 主 Agent 使用 maintenance Skill
  -> 必要时 Terra 定位
  -> 一个 worker 修复
  -> evidence Skill 跑定向测试与用户入口
  -> 主 Agent审阅并提交
```

体系 bug：

```text
用户报告
  -> system audit Skill 提取四项门禁
  -> Markdown 清楚：直接实施
  -> Markdown 冲突：只把冲突升级给用户
  -> worker 实现
  -> Sol reviewer 检查真实 diff
  -> 主 Agent确认证据与真实入口
```

大型功能：

```text
用户明确恢复 feature / TODO
  -> 任务简报与阶段计划
  -> 只读调查并行
  -> 独立 worktree 中单写入者分批实现
  -> 每批 reviewer + evidence
  -> 主 Agent最终验收
```

最终目标不是让流程看起来更复杂，而是让主 Agent把上下文留给领域判断，把机械工作交给脚本，把噪声交给只读 subagent，把业务裁决只交给用户。
