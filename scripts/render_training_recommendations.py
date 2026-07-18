#!/usr/bin/env python3
"""Render training recommendation rules as a deterministic Chinese review draft."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_INPUT = REPO_ROOT / "data" / "training_recommendations.json"
PRIORITIES = ("P0", "P1", "P2", "Info")


class RuleFormatError(ValueError):
    pass


def load_rules(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise RuleFormatError(f"无法读取规则文件 {path}: {error}") from error
    if not isinstance(value, dict):
        raise RuleFormatError("规则文件顶层必须是 JSON object")
    if not is_int(value.get("version")):
        raise RuleFormatError("version 必须是整数")
    for key in ("system_rules", "standalone_rules"):
        if not isinstance(value.get(key), list):
            raise RuleFormatError(f"{key} 必须是数组")
    return value


def is_int(value: Any) -> bool:
    return isinstance(value, int) and not isinstance(value, bool)


def require_object(value: Any, context: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise RuleFormatError(f"{context} 必须是 object")
    return value


def optional_string(value: dict[str, Any], key: str, context: str) -> str | None:
    result = value.get(key)
    if result is None:
        return None
    if not isinstance(result, str):
        raise RuleFormatError(f"{context}.{key} 必须是字符串")
    return result.strip() or None


def optional_bool(value: dict[str, Any], key: str, context: str) -> bool:
    result = value.get(key, False)
    if not isinstance(result, bool):
        raise RuleFormatError(f"{context}.{key} 必须是布尔值")
    return result


def require_string(value: dict[str, Any], key: str, context: str) -> str:
    result = value.get(key)
    if not isinstance(result, str) or not result.strip():
        raise RuleFormatError(f"{context}.{key} 必须是非空字符串")
    return result.strip()


def format_target(target: Any, context: str) -> str:
    target = require_object(target, context)
    name = require_string(target, "name", context)
    elite = target.get("elite")
    if not is_int(elite) or elite < 0:
        raise RuleFormatError(f"{context}.elite 必须是非负整数")
    level = target.get("level")
    if level is not None and (not is_int(level) or level < 1):
        raise RuleFormatError(f"{context}.level 必须是正整数")

    if elite == 0:
        threshold = f"{level}级" if level is not None else "精英0"
    else:
        threshold = {1: "精一", 2: "精二"}.get(elite, f"精英{elite}")
        if level is not None:
            threshold += f" {level}级"
    return f"{name}（{threshold}）"


def format_paths(paths: Any, context: str) -> str:
    if not isinstance(paths, list) or not all(
        isinstance(path, str) and path.strip() for path in paths
    ):
        raise RuleFormatError(f"{context} 必须是非空字符串数组")
    return "、".join(f"`{path}`" for path in paths) if paths else "未登记"


def render_system_rule(rule: dict[str, Any], index: int) -> list[str]:
    context = f"system_rules[{index}]"
    rule_id = require_string(rule, "id", context)
    label = require_string(rule, "label", context)
    priority = require_string(rule, "priority_ready_after_training", context)
    blocked_priority = rule.get("priority_blocked", "Info")
    if priority not in PRIORITIES or blocked_priority not in PRIORITIES:
        raise RuleFormatError(f"{context} 使用了未知优先级")

    core = rule.get("core")
    picks = rule.get("pick_one_core", [])
    if not isinstance(core, list) or not isinstance(picks, list):
        raise RuleFormatError(f"{context} 的 core/pick_one_core 必须是数组")

    lines = [f"### {label}（`{rule_id}`）", ""]
    lines.append(f"- 核心齐全但练度不足：**{priority}**")
    lines.append(f"- 缺少核心：**{blocked_priority}，暂缓培养该组合成员**")
    lines.append(
        "- 必需核心："
        + ("、".join(format_target(target, f"{context}.core") for target in core) or "无")
    )
    if picks:
        lines.append("- 任选核心：")
        for pick_index, pick in enumerate(picks):
            pick_context = f"{context}.pick_one_core[{pick_index}]"
            pick = require_object(pick, pick_context)
            pick_label = require_string(pick, "label", pick_context)
            candidates = pick.get("candidates")
            if not isinstance(candidates, list) or not candidates or not all(
                isinstance(candidate, str) and candidate.strip() for candidate in candidates
            ):
                raise RuleFormatError(f"{pick_context}.candidates 必须是非空字符串数组")
            target_suffix = format_target(
                {
                    "name": "候选",
                    "elite": pick.get("elite"),
                    "level": pick.get("level"),
                },
                pick_context,
            ).removeprefix("候选")
            lines.append(f"  - {pick_label}{target_suffix}：{' / '.join(candidates)}")
    else:
        lines.append("- 任选核心：无")

    source_system_id = optional_string(rule, "source_system_id", context)
    lines.append(f"- 来源体系 ID：`{source_system_id}`" if source_system_id else "- 来源体系 ID：未登记")
    lines.extend(render_common_rule_fields(rule, context))
    return lines


def render_common_rule_fields(rule: dict[str, Any], context: str) -> list[str]:
    reason_code = require_string(rule, "reason_code", context)
    message = optional_string(rule, "message", context) or "未填写"
    notes = optional_string(rule, "source_notes", context) or "未填写"
    source_repo = optional_string(rule, "source_repo", context)
    needs_review = optional_bool(rule, "needs_review", context)
    conflicts = rule.get("conflicts", [])
    if not isinstance(conflicts, list) or not all(isinstance(item, str) for item in conflicts):
        raise RuleFormatError(f"{context}.conflicts 必须是字符串数组")
    review_required = needs_review or bool(conflicts)
    return [
        f"- 验收状态：{'**待人工复核**' if review_required else '已登记'}",
        f"- 原因码：`{reason_code}`",
        f"- 玩家说明：{message}",
        f"- 维护说明：{notes}",
        f"- 规则来源仓库：`{source_repo}`" if source_repo else "- 规则来源仓库：使用顶层来源",
        f"- 来源：{format_paths(rule.get('source_paths', []), f'{context}.source_paths')}",
        "- 已知冲突：" + ("；".join(conflicts) if conflicts else "无"),
        "",
    ]


def render_standalone_rule(rule: dict[str, Any], index: int) -> list[str]:
    context = f"standalone_rules[{index}]"
    rule_id = require_string(rule, "id", context)
    label = require_string(rule, "label", context)
    priority = require_string(rule, "priority", context)
    if priority not in PRIORITIES:
        raise RuleFormatError(f"{context} 使用了未知优先级 {priority}")
    targets = rule.get("targets")
    if not isinstance(targets, list) or not targets:
        raise RuleFormatError(f"{context}.targets 必须是非空数组")

    lines = [f"### {label}（`{rule_id}`）", "", f"- 推荐优先级：**{priority}**"]
    lines.append(
        "- 培养目标："
        + "、".join(format_target(target, f"{context}.targets") for target in targets)
    )
    lines.extend(render_common_rule_fields(rule, context))
    return lines


def render_rules(rules: dict[str, Any], source: str) -> str:
    rules = require_object(rules, "规则文件顶层")
    if not is_int(rules.get("version")):
        raise RuleFormatError("version 必须是整数")
    systems = rules.get("system_rules")
    standalone = rules.get("standalone_rules")
    if not isinstance(systems, list) or not isinstance(standalone, list):
        raise RuleFormatError("system_rules 和 standalone_rules 必须是数组")
    review_states = []
    standalone_targets = 0
    for kind, collection in (("system_rules", systems), ("standalone_rules", standalone)):
        for index, raw_rule in enumerate(collection):
            context = f"{kind}[{index}]"
            rule = require_object(raw_rule, context)
            if kind == "standalone_rules":
                targets = rule.get("targets")
                if not isinstance(targets, list) or not targets:
                    raise RuleFormatError(f"{context}.targets 必须是非空数组")
                standalone_targets += len(targets)
            conflicts = rule.get("conflicts", [])
            if not isinstance(conflicts, list) or not all(isinstance(item, str) for item in conflicts):
                raise RuleFormatError(f"{context}.conflicts 必须是字符串数组")
            review_states.append(optional_bool(rule, "needs_review", context) or bool(conflicts))
    needs_review = sum(review_states)
    source_repo = rules.get("source_repo")
    if source_repo is not None and not isinstance(source_repo, str):
        raise RuleFormatError("source_repo 必须是字符串")

    lines = [
        "# 基建练卡推荐规则验收稿",
        "",
        "> 本文由脚本从结构化规则确定性生成，仅用于人工验收。",
        "> `docs/练卡推荐规则.md` 与用户当前裁决仍是业务真源；不要直接编辑本文。",
        "",
        "## 摘要",
        "",
        f"- 输入：`{source}`",
        f"- 规则版本：{rules['version']}",
        f"- 顶层来源仓库：`{source_repo}`" if source_repo else "- 顶层来源仓库：未登记",
        f"- 体系规则：{len(systems)} 条",
        f"- 散件规则：{len(standalone)} 条，覆盖 {standalone_targets} 名干员",
        f"- 标记待复核：{needs_review} 条",
        "",
        "## 验收原则",
        "",
        "1. 未拥有干员不产生练卡任务。",
        "2. 组合缺任一必需核心时，该组合全部成员暂缓培养。",
        "3. 星级不直接决定目标练度，逐项核对实际技能门槛。",
        "4. 搜索剪枝池收录不等于值得培养。",
        "5. `needs_review` 或已知冲突必须由人工裁决，不能由生成脚本消解。",
        "",
        "## 组合规则",
        "",
    ]
    if systems:
        for index, rule in enumerate(systems):
            if not isinstance(rule, dict):
                raise RuleFormatError(f"system_rules[{index}] 必须是 object")
            lines.extend(render_system_rule(rule, index))
    else:
        lines.extend(["当前没有组合规则。", ""])

    lines.extend(["## 散件规则", ""])
    if standalone:
        for index, rule in enumerate(standalone):
            if not isinstance(rule, dict):
                raise RuleFormatError(f"standalone_rules[{index}] 必须是 object")
            lines.extend(render_standalone_rule(rule, index))
    else:
        lines.extend(["当前没有散件规则。", ""])

    lines.extend(
        [
            "## 公孙长乐验收结论",
            "",
            "请逐条填写：`通过`、`修改` 或 `待裁决`，并说明目标练度、组合角色或优先级的具体调整。",
            "",
        ]
    )
    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output", type=Path, help="输出 Markdown；省略时写 stdout")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        rules = load_rules(args.input)
        resolved_input = args.input.resolve()
        try:
            source = resolved_input.relative_to(REPO_ROOT).as_posix()
        except ValueError:
            source = str(resolved_input)
        rendered = render_rules(rules, source)
        if args.output:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(rendered, encoding="utf-8")
            print(args.output)
        else:
            sys.stdout.write(rendered)
    except (OSError, RuleFormatError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
