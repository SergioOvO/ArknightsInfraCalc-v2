from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


CODEX_DIR = Path(__file__).resolve().parents[1]
FIXTURES = Path(__file__).resolve().parent / "fixtures"
sys.path.insert(0, str(CODEX_DIR))

import check_docs_impact  # noqa: E402
import check_task_scope  # noqa: E402
import compare_test_failures  # noqa: E402
import docs_inventory  # noqa: E402
import render_evidence  # noqa: E402


RUNNER = CODEX_DIR / "run_evidence.sh"
COMPARE = CODEX_DIR / "compare_test_failures.py"


class FailureParserTests(unittest.TestCase):
    def test_no_failures(self) -> None:
        self.assertEqual(compare_test_failures.parse_log(FIXTURES / "cargo_ok.txt"), set())

    def test_exact_names_and_duplicates(self) -> None:
        self.assertEqual(
            compare_test_failures.parse_log(FIXTURES / "cargo_fail_ab.txt"),
            {"suite::alpha", "suite::beta"},
        )
        self.assertEqual(
            compare_test_failures.parse_log(FIXTURES / "cargo_duplicate.txt"),
            {"shared::same_name"},
        )

    def test_same_added_and_removed_policy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            same = subprocess.run(
                [
                    sys.executable,
                    str(COMPARE),
                    "--baseline",
                    str(FIXTURES / "cargo_fail_ab.txt"),
                    "--current",
                    str(FIXTURES / "cargo_fail_ab_reordered.txt"),
                    "--json-out",
                    str(root / "same.json"),
                ],
                check=False,
            )
            self.assertEqual(same.returncode, 0)
            added = subprocess.run(
                [
                    sys.executable,
                    str(COMPARE),
                    "--baseline",
                    str(FIXTURES / "cargo_fail_ab.txt"),
                    "--current",
                    str(FIXTURES / "cargo_fail_abc.txt"),
                ],
                check=False,
                stdout=subprocess.DEVNULL,
            )
            self.assertEqual(added.returncode, 1)
            reduced = subprocess.run(
                [
                    sys.executable,
                    str(COMPARE),
                    "--baseline",
                    str(FIXTURES / "cargo_fail_ab.txt"),
                    "--current",
                    str(FIXTURES / "cargo_fail_a.txt"),
                ],
                check=False,
                stdout=subprocess.DEVNULL,
            )
            self.assertEqual(reduced.returncode, 0)

    def test_truncated_and_malformed_logs_fail_closed(self) -> None:
        for fixture in ("cargo_truncated.txt", "cargo_malformed.txt"):
            with self.subTest(fixture=fixture):
                with self.assertRaises(compare_test_failures.ParseError):
                    compare_test_failures.parse_log(FIXTURES / fixture)


class EvidenceRunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def run_evidence(
        self,
        task: str,
        command: list[str],
        *,
        inputs: str = "self-test input",
        artifacts: list[str] | None = None,
        metadata: Path | None = None,
    ) -> subprocess.CompletedProcess[str]:
        args = [
            "bash",
            str(RUNNER),
            "--task",
            task,
            "--category",
            "targeted-test",
            "--stem",
            "case",
            "--inputs",
            inputs,
        ]
        for artifact in artifacts or []:
            args.extend(["--artifact", artifact])
        if metadata is not None:
            args.extend(["--metadata", str(metadata)])
        args.extend(["--", *command])
        return subprocess.run(args, cwd=self.root, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)

    def manifest(self, task: str) -> dict[str, object]:
        path = self.root / "target/codex-runs" / task / "manifest.json"
        return json.loads(path.read_text(encoding="utf-8"))

    def test_pass_fail_command_not_found_and_signal_exit_codes(self) -> None:
        cases = [
            ("pass", [sys.executable, "-c", "print('ok')"], 0),
            ("fail", ["bash", "-c", "exit 7"], 7),
            ("missing", ["codex-command-that-does-not-exist-9f91"], 127),
            ("signal", ["bash", "-c", "kill -TERM $$"], 143),
        ]
        for task, command, expected in cases:
            with self.subTest(task=task):
                result = self.run_evidence(task, command)
                self.assertEqual(result.returncode, expected, result.stderr)
                run = self.manifest(task)["runs"][0]
                self.assertEqual(run["exit_code"], expected)

    def test_special_characters_are_preserved(self) -> None:
        inputs = "中文 空格 $HOME 'quote'\nsecond line"
        argument = "参数 with spaces $() 'quoted'"
        result = self.run_evidence(
            "special",
            [sys.executable, "-c", "import sys; assert sys.argv[1] == sys.argv[2]", argument, argument],
            inputs=inputs,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        run = self.manifest("special")["runs"][0]
        self.assertEqual(run["inputs"], inputs)
        self.assertEqual(run["command"][-2:], [argument, argument])

    def test_command_arguments_after_double_dash_are_preserved(self) -> None:
        command = [
            sys.executable,
            "-c",
            "import sys; assert sys.argv[1:] == ['--', '--check']",
            "--",
            "--check",
        ]
        result = self.run_evidence("double-dash", command)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.manifest("double-dash")["runs"][0]["command"], command)

    def test_manifest_update_failure_returns_nonzero(self) -> None:
        metadata = self.root / "invalid-metadata.json"
        metadata.write_text("not json\n", encoding="utf-8")

        result = self.run_evidence(
            "manifest-failure",
            [sys.executable, "-c", "pass"],
            metadata=metadata,
        )

        self.assertEqual(result.returncode, 70)
        self.assertIn("manifest_update=FAIL", result.stderr)

    def test_concurrent_runs_do_not_overwrite(self) -> None:
        base = [
            "bash",
            str(RUNNER),
            "--task",
            "parallel",
            "--category",
            "targeted-test",
            "--stem",
            "case",
            "--inputs",
            "parallel fixture",
            "--",
            "bash",
            "-c",
        ]
        first = subprocess.Popen([*base, "printf first"], cwd=self.root)
        second = subprocess.Popen([*base, "printf second"], cwd=self.root)
        self.assertEqual(first.wait(), 0)
        self.assertEqual(second.wait(), 0)
        runs = self.manifest("parallel")["runs"]
        self.assertEqual(len(runs), 2)
        self.assertEqual(len({run["log"] for run in runs}), 2)
        self.assertTrue(all(Path(run["log"]).is_file() for run in runs))

    def test_renderer_fails_on_missing_or_inconsistent_evidence(self) -> None:
        result = self.run_evidence("render", [sys.executable, "-c", "print('ok')"])
        self.assertEqual(result.returncode, 0)
        manifest = self.manifest("render")
        render_evidence.validate_manifest(manifest)
        rendered = render_evidence.render(manifest)
        self.assertIn("Build：未跑", rendered)
        self.assertIn("定向测试：", rendered)

        bad_status = copy.deepcopy(manifest)
        status_path = Path(bad_status["runs"][0]["status_file"])
        original = status_path.read_text(encoding="utf-8")
        status_path.write_text(original.replace("exit_code=0", "exit_code=9"), encoding="utf-8")
        with self.assertRaises(render_evidence.ManifestError):
            render_evidence.validate_manifest(bad_status)

        status_path.write_text(original, encoding="utf-8")
        Path(manifest["runs"][0]["log"]).unlink()
        with self.assertRaises(render_evidence.ManifestError):
            render_evidence.validate_manifest(manifest)

    def test_renderer_rejects_missing_registered_artifact(self) -> None:
        result = self.run_evidence(
            "artifact",
            [sys.executable, "-c", "print('ok')"],
            artifacts=["json=out/missing.json"],
        )
        self.assertEqual(result.returncode, 0)
        with self.assertRaises(render_evidence.ManifestError):
            render_evidence.validate_manifest(self.manifest("artifact"))

    def test_renderer_wraps_links_with_spaces(self) -> None:
        path = self.root / "evidence with spaces.log"
        path.write_text("ok\n", encoding="utf-8")
        self.assertIn(f"(<{path.resolve()}>)", render_evidence._link("evidence", str(path)))


class DocsImpactTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name)
        (self.repo / "docs").mkdir()
        (self.repo / "src").mkdir()
        (self.repo / "docs/A.md").write_text(
            "# A\n\n"
            "> 文档角色：canonical\n"
            "> 生命周期状态：current\n"
            "> 领域键：test.a\n"
            "> 当前真源：self\n"
            "> 复核触发：src/**\n"
            "> 摘要：documents the stable application fixture\n\n"
            "Current application behavior.\n",
            encoding="utf-8",
        )
        (self.repo / "src/app.py").write_text("print('ok')\n", encoding="utf-8")
        subprocess.run(["git", "init", "-q"], cwd=self.repo, check=True)
        subprocess.run(["git", "add", "docs/A.md", "src/app.py"], cwd=self.repo, check=True)
        document = docs_inventory.parse_document(self.repo, self.repo / "docs/A.md")
        docs_inventory.write_review_record(self.repo, document, cause="lifecycle-migration")
        self.config = {
            "schema_version": 2,
            "ignore_globs": [],
        }

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def manifest(self, status: str = "updated") -> dict[str, object]:
        document = docs_inventory.parse_document(self.repo, self.repo / "docs/A.md")
        entries = []
        if status == "updated":
            entries = [docs_inventory.docs_impact_entry(document)]
        return {
            "docs_impact": {
                "status": status,
                "entries": entries,
                "reason": "checked the current behavior and documentation contract",
            }
        }

    def test_updated_and_not_needed(self) -> None:
        self.assertEqual(
            check_docs_impact.run_checks(
                self.repo, self.config, self.manifest("updated"), {"src/app.py", "docs/A.md"}, []
            ),
            [],
        )
        self.assertEqual(
            check_docs_impact.run_checks(
                self.repo, self.config, self.manifest("not-needed"), set(), []
            ),
            [],
        )

    def test_changed_markdown_requires_exact_review_entry(self) -> None:
        manifest = self.manifest("updated")
        manifest["docs_impact"]["entries"] = []
        errors = check_docs_impact.run_checks(
            self.repo, self.config, manifest, {"docs/A.md"}, []
        )
        self.assertTrue(any("missing changed or triggered documents" in error for error in errors))

    def test_generator_change_requires_review_entry(self) -> None:
        generator = self.repo / "scripts/gen.py"
        generator.parent.mkdir()
        generator.write_text("print('generate')\n", encoding="utf-8")
        generated = self.repo / "docs/GENERATED.md"
        generated.write_text(
            "# Generated\n\n"
            "> 文档角色：generated-reference\n"
            "> 生命周期状态：generated\n"
            "> 当前真源：docs/A.md\n"
            "> 生成器：scripts/gen.py\n"
            "> 摘要：generated fixture for dependency coverage\n\n"
            "Generated output.\n",
            encoding="utf-8",
        )
        subprocess.run(["git", "add", "scripts/gen.py", "docs/GENERATED.md"], cwd=self.repo, check=True)
        document = docs_inventory.parse_document(self.repo, generated)
        docs_inventory.write_review_record(self.repo, document, cause="source-change")
        document = docs_inventory.parse_document(self.repo, generated)
        manifest = self.manifest("updated")
        manifest["docs_impact"]["entries"] = []
        errors = check_docs_impact.run_checks(
            self.repo, self.config, manifest, {"scripts/gen.py"}, []
        )
        self.assertTrue(any("missing changed or triggered documents" in error for error in errors))

    def test_blocked_uncovered_missing_and_false_updated_fail(self) -> None:
        valid_manifest = self.manifest()
        blocked = check_docs_impact.run_checks(
            self.repo, self.config, self.manifest("blocked"), {"src/app.py", "docs/A.md"}, []
        )
        self.assertTrue(any("blocked" in error for error in blocked))

        uncovered = check_docs_impact.run_checks(
            self.repo, self.config, valid_manifest, {"other/file.rs"}, []
        )
        self.assertTrue(any("no document owner" in error for error in uncovered))

        (self.repo / "docs/A.md").unlink()
        missing = check_docs_impact.run_checks(
            self.repo, self.config, valid_manifest, {"src/app.py"}, []
        )
        self.assertTrue(any("governed current document" in error or "metadata" in error for error in missing))

        (self.repo / "docs/A.md").write_text("# A\n", encoding="utf-8")
        false_updated = check_docs_impact.run_checks(
            self.repo, self.config, valid_manifest, {"src/app.py"}, []
        )
        self.assertTrue(any("not updated" in error or "lifecycle metadata" in error for error in false_updated))

    def test_generated_link_and_status_checks(self) -> None:
        original = (self.repo / "docs/A.md").read_text(encoding="utf-8")
        (self.repo / "docs/A.md").write_text(original + "\n[missing](missing.md)\n", encoding="utf-8")
        document = docs_inventory.parse_document(self.repo, self.repo / "docs/A.md")
        docs_inventory.write_review_record(self.repo, document, cause="document-change")
        errors = check_docs_impact.run_checks(
            self.repo,
            self.config,
            self.manifest("updated"),
            {"src/app.py", "docs/A.md"},
            ["doc-status"],
        )
        self.assertTrue(any("broken Markdown link" in error for error in errors))

    def test_markdown_link_with_spaces(self) -> None:
        (self.repo / "docs/target file.md").write_text("# Target\n", encoding="utf-8")
        original = (self.repo / "docs/A.md").read_text(encoding="utf-8")
        (self.repo / "docs/A.md").write_text(original + "\n[target](<target file.md>)\n", encoding="utf-8")
        self.assertEqual(check_docs_impact.check_markdown_links(self.repo, ["docs/A.md"]), [])

    def test_repository_cli_map_matches_dispatch(self) -> None:
        repo = CODEX_DIR.parents[1]
        self.assertEqual(check_docs_impact.check_cli_help_map(repo), [])


class TaskScopeTests(unittest.TestCase):
    def manifest(self) -> dict[str, object]:
        invariant = "all evidence commands remain inside the declared task boundary"
        return {
            "change_scope": {
                "invariant": invariant,
                "root_cause_layer": "scripts/codex",
                "required_paths": ["src/app.py"],
                "allowed_consumers": [],
                "proof_paths": ["tests/**"],
                "explicitly_deferred": ["src/deferred.py"],
            },
            "scope_expansions": [],
            "side_findings": [],
            "docs_impact": {"entries": []},
            "reviewer": {
                "status": "reviewed",
                "scope_invariant": invariant,
                "changed_paths": ["src/app.py"],
                "scope_expansion_ids": [],
            },
        }

    def test_valid_scope(self) -> None:
        self.assertEqual(check_task_scope.run_checks(self.manifest(), {"src/app.py"}), [])

    def test_undeclared_and_deferred_paths_fail(self) -> None:
        manifest = self.manifest()
        manifest["reviewer"]["changed_paths"] = ["other.py"]
        errors = check_task_scope.run_checks(manifest, {"other.py"})
        self.assertTrue(any("outside declared scope" in error for error in errors))

        manifest = self.manifest()
        manifest["reviewer"]["changed_paths"] = ["src/deferred.py"]
        errors = check_task_scope.run_checks(manifest, {"src/deferred.py"})
        self.assertTrue(any("explicitly deferred" in error for error in errors))

    def test_expansion_requires_reason_and_reviewer_history(self) -> None:
        manifest = self.manifest()
        manifest["scope_expansions"] = [
            {"id": "more", "paths": ["src/more.py"], "reason": "short", "evidence": "line 1"}
        ]
        manifest["reviewer"]["changed_paths"] = ["src/app.py", "src/more.py"]
        errors = check_task_scope.run_checks(manifest, {"src/app.py", "src/more.py"})
        self.assertTrue(any("reason must explain" in error for error in errors))
        self.assertTrue(any("scope_expansion_ids" in error for error in errors))

    def test_deferred_side_finding_cannot_be_implemented(self) -> None:
        manifest = self.manifest()
        manifest["change_scope"]["allowed_consumers"] = ["src/adjacent.py"]
        manifest["side_findings"] = [
            {
                "summary": "adjacent issue",
                "disposition": "deferred",
                "paths": ["src/adjacent.py"],
            }
        ]
        manifest["reviewer"]["changed_paths"] = ["src/adjacent.py"]
        errors = check_task_scope.run_checks(manifest, {"src/adjacent.py"})
        self.assertTrue(any("deferred side finding" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
