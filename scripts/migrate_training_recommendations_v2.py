#!/usr/bin/env python3
"""Migrate training_recommendations.json from v1 schema to v2."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# Heuristic: known small combos vs large systems from current data.
COMBO_IDS = {
    "lungmen_manu_pair",
    "penguin_exusiai_lemuen",
    "penguin_texlap_e0",
    "penguin_texangel_e2",
    "snhunt_monhun_control",
    "human_fireworks_pure",
    "ling_jie_karlan",
    "vina_lungmen",
    "docus_core",
}

SOFT_STANDALONE_IDS = {
    "standalone_hongyun_recycle",
}

# Rarity hints for acquisition policy on common low-star operators.
RARITY = {
    "Castle-3": 1,
    "夜刀": 2,
    "巡林者": 2,
    "杜林": 2,
    "12F": 2,
    "芬": 3,
    "香草": 3,
    "翎羽": 3,
    "玫兰莎": 3,
    "米格鲁": 3,
    "克洛丝": 3,
    "安德切尔": 3,
    "炎熔": 3,
    "芙蓉": 3,
    "安赛尔": 3,
    "史都华德": 3,
    "梓兰": 3,
    "空爆": 3,
    "月见夜": 3,
    "泡普卡": 3,
    "斑点": 3,
    "黑角": 2,
    "慕斯": 4,
    "缠丸": 4,
    "安比尔": 4,
    "夜烟": 4,
    "古米": 4,
    "清流": 4,
    "砾": 4,
    "石英": 4,
    "断罪者": 4,
    "霜叶": 4,
    "白雪": 4,
    "红豆": 4,
    "食铁兽": 4,
    "红云": 4,
    "杰西卡": 4,
    "调香师": 4,
    "罗比菈塔": 4,
    "可颂": 5,
    "拜松": 5,
    "苍苔": 5,
    "槐琥": 5,
    "空弦": 6,
    "吉星": 5,
    "海蒂": 5,
    "能天使": 6,
    "雪雉": 5,
    "衡沙": 5,
    "瑰盐": 5,
    "深巡": 5,
    "裂响": 6,
    "酒神": 6,
    "弑君者": 5,
    "圣约送葬人": 6,
    "怒潮凛冬": 6,
    "巫恋": 5,
    "龙舌兰": 5,
    "卡夫卡": 5,
    "柏喙": 5,
    "明椒": 5,
    "折光": 5,
    "但书": 5,
    "可露希尔": 5,
    "黑键": 6,
    "斩业星熊": 6,
    "诗怀雅": 5,
    "蕾缪安": 6,
    "德克萨斯": 5,
    "拉普兰德": 5,
    "空": 5,
}


def evidence_from_paths(paths: list[str]) -> list[dict]:
    out = []
    for p in paths or []:
        out.append({"path": p})
    return out


def member(
    name: str,
    role: str,
    elite: int,
    level,
    priority: str,
    rarity: int | None = None,
    acquisition: str = "policy",
) -> dict:
    m = {
        "operator": name,
        "role": role,
        "target": {"elite": elite},
        "priority": priority,
        "acquisition": acquisition,
    }
    if level is not None:
        m["target"]["level"] = level
    r = rarity if rarity is not None else RARITY.get(name)
    if r is not None:
        m["rarity"] = r
        if acquisition == "policy" and r <= 4:
            m["acquisition"] = "policy"
        elif acquisition == "policy" and name == "苍苔":
            m["acquisition"] = "policy"
        elif acquisition == "policy" and r >= 5 and name != "苍苔":
            m["acquisition"] = "owned_only"
    return m


def migrate_system(rule: dict) -> dict:
    rid = rule["id"]
    kind = "combo" if rid in COMBO_IDS else "system"
    if rid == "snhunt_monhun_control" or "control" in rid:
        scope = "control_center"
    elif kind == "combo":
        scope = "same_station"
    else:
        scope = "cross_station"

    required = [c["name"] for c in rule.get("core", [])]
    pick_one = []
    members = []

    prio_core = rule.get("priority_ready_after_training", "P1")
    prio_hang = rule.get("priority_hangers", "P2")

    for c in rule.get("core", []):
        members.append(
            member(c["name"], "core", c["elite"], c.get("level"), prio_core)
        )

    for slot in rule.get("pick_one_core", []):
        pick_one.append(
            {"label": slot["label"], "candidates": list(slot["candidates"])}
        )
        for cand in slot["candidates"]:
            if any(m["operator"] == cand for m in members):
                continue
            members.append(
                member(cand, "core", slot["elite"], slot.get("level"), prio_core)
            )

    for t in rule.get("important", []):
        members.append(
            member(t["name"], "important", t["elite"], t.get("level"), prio_core)
        )

    for t in rule.get("hangers", []):
        members.append(
            member(t["name"], "hanger", t["elite"], t.get("level"), prio_hang)
        )

    review_status = "needs_review" if rule.get("needs_review") else "confirmed"
    conflicts = list(rule.get("conflicts") or [])
    if review_status == "needs_review" and not conflicts:
        conflicts = ["migrated from v1 needs_review without explicit conflicts"]

    return {
        "id": rid,
        "kind": kind,
        "scope": scope,
        "label": rule.get("label", rid),
        "source_system_id": rule.get("source_system_id"),
        "admission": {
            "required_core": required,
            "pick_one_core": pick_one,
        },
        "members": members,
        "evidence": evidence_from_paths(rule.get("source_paths") or []),
        "review": {
            "status": review_status,
            "conflicts": conflicts,
        },
    }


def migrate_standalone(rule: dict) -> dict:
    rid = rule["id"]
    kind = "soft_combo" if rid in SOFT_STANDALONE_IDS else "standalone"
    prio = rule.get("priority", "P2")
    members = []
    for t in rule.get("targets", []):
        members.append(
            member(
                t["name"],
                "independent",
                t["elite"],
                t.get("level"),
                prio,
            )
        )

    review_status = "needs_review" if rule.get("needs_review") else "confirmed"
    conflicts = list(rule.get("conflicts") or [])
    if review_status == "needs_review" and not conflicts:
        conflicts = ["migrated from v1 needs_review without explicit conflicts"]

    return {
        "id": rid,
        "kind": kind,
        "scope": "independent",
        "label": rule.get("label", rid),
        "source_system_id": None,
        "admission": {"required_core": [], "pick_one_core": []},
        "members": members,
        "evidence": evidence_from_paths(rule.get("source_paths") or []),
        "review": {
            "status": review_status,
            "conflicts": conflicts,
        },
    }


def migrate(v1: dict) -> dict:
    rules = []
    for r in v1.get("system_rules") or []:
        rules.append(migrate_system(r))
    for r in v1.get("standalone_rules") or []:
        rules.append(migrate_standalone(r))
    return {
        "version": 2,
        "acquisition_policy": {
            "default_rarity_le": 4,
            "named_exceptions": ["苍苔"],
        },
        "rules": rules,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--input",
        type=Path,
        default=REPO / "data" / "training_recommendations.json",
    )
    ap.add_argument(
        "--output",
        type=Path,
        default=REPO / "data" / "training_recommendations.json",
    )
    args = ap.parse_args()
    v1 = json.loads(args.input.read_text(encoding="utf-8"))
    if v1.get("version") == 2 and "rules" in v1:
        print("already v2, skip")
        return
    v2 = migrate(v1)
    args.output.write_text(
        json.dumps(v2, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print(f"wrote {args.output} rules={len(v2['rules'])}")


if __name__ == "__main__":
    main()
