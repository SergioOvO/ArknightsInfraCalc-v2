import importlib.util
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "render_training_recommendations.py"
SKILL = (
    Path(__file__).resolve().parents[2]
    / ".agents"
    / "skills"
    / "gongsun-training-review"
    / "SKILL.md"
)
SPEC = importlib.util.spec_from_file_location("render_training_recommendations", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class RenderTrainingRecommendationsTests(unittest.TestCase):
    def test_current_rules_render_review_relevant_fields(self) -> None:
        rules = MODULE.load_rules(MODULE.DEFAULT_INPUT)
        rendered = MODULE.render_rules(rules, "data/training_recommendations.json")

        self.assertIn("# 基建练卡推荐规则验收稿", rendered)
        self.assertIn("缺少核心：**Info，暂缓培养该组合成员**", rendered)
        self.assertIn("裁缝β第三人（精二）：卡夫卡 / 柏喙 / 明椒 / 折光", rendered)
        self.assertIn("来源体系 ID：`witch_long_beta`", rendered)
        self.assertIn("顶层来源仓库：`ArknightsInfraCalc-v2`", rendered)
        self.assertIn("石英（精一）", rendered)
        self.assertIn("Castle-3（30级）", rendered)
        self.assertIn("标记待复核：1 条", rendered)
        self.assertIn("vault docs system_id=closure_special_order", rendered)

    def test_invalid_target_elite_is_rejected(self) -> None:
        rules = {
            "version": 1,
            "system_rules": [],
            "standalone_rules": [
                {
                    "id": "bad",
                    "label": "bad",
                    "priority": "P0",
                    "targets": [{"name": "干员", "elite": "1"}],
                    "reason_code": "bad",
                }
            ],
        }

        with self.assertRaisesRegex(MODULE.RuleFormatError, "elite"):
            MODULE.render_rules(rules, "fixture.json")

    def test_invalid_nested_types_are_rejected_without_coercion(self) -> None:
        base = {
            "version": 1,
            "system_rules": [],
            "standalone_rules": [],
        }
        cases = [
            ({**base, "version": True}, "version"),
            (
                {
                    **base,
                    "standalone_rules": [
                        {
                            "id": "bad",
                            "label": "bad",
                            "priority": "P0",
                            "targets": [None],
                            "reason_code": "bad",
                        }
                    ],
                },
                "object",
            ),
            (
                {
                    **base,
                    "standalone_rules": [
                        {
                            "id": "bad",
                            "label": "bad",
                            "priority": "P0",
                            "targets": [{"name": "干员", "elite": True}],
                            "reason_code": "bad",
                        }
                    ],
                },
                "elite",
            ),
            (
                {
                    **base,
                    "standalone_rules": [
                        {
                            "id": "bad",
                            "label": "bad",
                            "priority": "P0",
                            "targets": [{"name": "干员", "elite": 1}],
                            "reason_code": "bad",
                            "needs_review": "false",
                        }
                    ],
                },
                "needs_review",
            ),
            (
                {
                    **base,
                    "standalone_rules": [
                        {
                            "id": "bad",
                            "label": "bad",
                            "priority": "P0",
                            "targets": None,
                            "reason_code": "bad",
                        }
                    ],
                },
                "targets",
            ),
            (
                {
                    **base,
                    "system_rules": [
                        {
                            "id": "bad",
                            "label": "bad",
                            "priority_ready_after_training": "P0",
                            "core": [],
                            "pick_one_core": [
                                {"label": "空候选", "elite": 1, "candidates": []}
                            ],
                            "reason_code": "bad",
                        }
                    ],
                },
                "candidates",
            ),
        ]

        for rules, message in cases:
            with self.subTest(message=message):
                with self.assertRaisesRegex(MODULE.RuleFormatError, message):
                    MODULE.render_rules(rules, "fixture.json")

    def test_conflict_forces_manual_review_and_render_is_deterministic(self) -> None:
        rules = {
            "version": 1,
            "system_rules": [],
            "standalone_rules": [
                {
                    "id": "conflict",
                    "label": "冲突规则",
                    "priority": "P0",
                    "targets": [{"name": "干员", "elite": 1}],
                    "reason_code": "conflict",
                    "needs_review": False,
                    "conflicts": ["来源冲突"],
                }
            ],
        }

        first = MODULE.render_rules(rules, "fixture.json")
        second = MODULE.render_rules(rules, "fixture.json")
        self.assertEqual(first, second)
        self.assertIn("验收状态：**待人工复核**", first)

    def test_cli_schema_error_has_no_traceback(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bad.json"
            path.write_text(
                '{"version": 1, "system_rules": [], "standalone_rules": [{"id": "bad"}]}',
                encoding="utf-8",
            )
            result = subprocess.run(
                [sys.executable, str(SCRIPT), "--input", str(path)],
                check=False,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

        self.assertEqual(result.returncode, 2)
        self.assertIn("error:", result.stderr)
        self.assertNotIn("Traceback", result.stderr)

    def test_review_skill_has_discoverable_frontmatter_and_command(self) -> None:
        text = SKILL.read_text(encoding="utf-8")

        self.assertTrue(text.startswith("---\n"))
        self.assertIn("\nname: gongsun-training-review\n", text)
        self.assertIn("\ndescription:", text)
        self.assertIn("python3 scripts/render_training_recommendations.py", text)


if __name__ == "__main__":
    unittest.main()
