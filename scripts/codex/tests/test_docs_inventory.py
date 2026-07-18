from __future__ import annotations

import tempfile
import subprocess
import unittest
from pathlib import Path

from scripts.codex import docs_inventory


def write_doc(root: Path, path: str, metadata: str, body: str = "Body.\n") -> Path:
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(f"# Title\n\n{metadata}\n\n{body}", encoding="utf-8")
    return target


class DocsInventoryTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.repo = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def canonical(self, path: str = "docs/A.md", domain: str = "a") -> docs_inventory.Document:
        source = self.repo / "src/a.rs"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text("a\n", encoding="utf-8")
        target = write_doc(
            self.repo,
            path,
            "> 文档角色：canonical\n"
            "> 生命周期状态：current\n"
            f"> 领域键：{domain}\n"
            "> 当前真源：self\n"
            "> 复核触发：src/**\n"
            "> 摘要：canonical fixture",
        )
        return docs_inventory.parse_document(self.repo, target)

    def test_parses_and_validates_canonical(self) -> None:
        document = self.canonical()
        self.assertEqual(document.role, "canonical")
        self.assertEqual(document.domains, ["a"])
        self.assertEqual(docs_inventory.validate_inventory(self.repo, [document], final=False), [])

    def test_rejects_duplicate_domain_and_illegal_todo_status(self) -> None:
        first = self.canonical("docs/A.md", "same")
        second = self.canonical("docs/B.md", "same")
        todo = write_doc(
            self.repo,
            "docs/TODO/X.md",
            "> 文档角色：active-change\n"
            "> 生命周期状态：paused\n"
            "> 当前真源：docs/A.md\n"
            "> 复核触发：src/**\n"
            "> 摘要：invalid fixture",
        )
        errors = docs_inventory.validate_inventory(
            self.repo, [first, second, docs_inventory.parse_document(self.repo, todo)], final=False
        )
        self.assertTrue(any("exactly one canonical owner" in error for error in errors))
        self.assertTrue(any("invalid lifecycle status" in error for error in errors))

    def test_final_check_rejects_in_progress(self) -> None:
        canonical = self.canonical()
        todo = write_doc(
            self.repo,
            "docs/TODO/X.md",
            "> 文档角色：active-change\n"
            "> 生命周期状态：in-progress\n"
            "> 当前真源：docs/A.md\n"
            "> 复核触发：src/**\n"
            "> 摘要：work fixture",
        )
        errors = docs_inventory.validate_inventory(
            self.repo, [canonical, docs_inventory.parse_document(self.repo, todo)], final=True
        )
        self.assertTrue(any("cannot pass final check" in error for error in errors))

    def test_document_digest_excludes_review_record(self) -> None:
        document = self.canonical()
        first = docs_inventory.document_digest(document)
        changed = document.text.replace("> 摘要：", "> 复核原因：migration\n> 摘要：")
        reparsed = docs_inventory.parse_document(self.repo, self.repo / document.path, changed)
        self.assertEqual(first, docs_inventory.document_digest(reparsed))

    def test_source_digest_is_stable_and_changes_with_source(self) -> None:
        document = self.canonical()
        first = docs_inventory.source_digest(self.repo, document)
        (self.repo / "src/a.rs").write_text("b\n", encoding="utf-8")
        self.assertNotEqual(first, docs_inventory.source_digest(self.repo, document))

    def test_source_digest_ignores_untracked_files_in_git_repo(self) -> None:
        document = self.canonical()
        subprocess.run(["git", "init", "-q"], cwd=self.repo, check=True)
        subprocess.run(["git", "add", "docs/A.md", "src/a.rs"], cwd=self.repo, check=True)
        first = docs_inventory.source_digest(self.repo, document)
        (self.repo / "src/untracked.pyc").write_bytes(b"generated")
        self.assertEqual(first, docs_inventory.source_digest(self.repo, document))

    def test_source_coverage_requires_owner(self) -> None:
        document = self.canonical()
        self.assertEqual(
            docs_inventory.source_coverage_errors(["src/a.rs"], [document], []), []
        )
        self.assertTrue(
            docs_inventory.source_coverage_errors(["other/new.rs"], [document], [])
        )

    def test_review_record_is_bound_to_current_digests(self) -> None:
        document = self.canonical()
        docs_inventory.write_review_record(self.repo, document, cause="lifecycle-migration")
        refreshed = docs_inventory.parse_document(self.repo, self.repo / document.path)
        self.assertEqual(docs_inventory.review_errors(self.repo, refreshed), [])
        (self.repo / "src/a.rs").write_text("changed\n", encoding="utf-8")
        self.assertTrue(any("source digest drift" in error for error in docs_inventory.review_errors(self.repo, refreshed)))
        entry = docs_inventory.docs_impact_entry(refreshed)
        self.assertEqual(entry["path"], "docs/A.md")
        self.assertEqual(entry["source_digest"], refreshed.metadata["源摘要"])
        self.assertEqual(entry["disposition"], "updated")

    def test_generated_tables_are_deterministic(self) -> None:
        first = self.canonical("docs/B.md", "z")
        second = self.canonical("docs/A.md", "a")
        table = docs_inventory.render_canonical_table([first, second])
        self.assertLess(table.index("`a`"), table.index("`z`"))

    def test_transition_fields_are_paired(self) -> None:
        document = self.canonical()
        text = document.text.replace("> 摘要：", "> 转换自：docs/OLD.md\n> 摘要：")
        reparsed = docs_inventory.parse_document(self.repo, self.repo / document.path, text)
        self.assertTrue(any("must appear together" in error for error in docs_inventory.validate_document(reparsed, final=False)))

    def test_owner_must_be_governed_current_document(self) -> None:
        canonical = self.canonical()
        (self.repo / "plain.txt").write_text("not a document\n", encoding="utf-8")
        reference = write_doc(
            self.repo,
            "docs/R.md",
            "> 文档角色：current-reference\n"
            "> 生命周期状态：current\n"
            "> 当前真源：plain.txt\n"
            "> 复核触发：src/**\n"
            "> 摘要：reference with an invalid non-document owner",
        )
        errors = docs_inventory.validate_inventory(
            self.repo, [canonical, docs_inventory.parse_document(self.repo, reference)], final=False
        )
        self.assertTrue(any("governed current document" in error for error in errors))

    def test_role_path_matrix_is_bidirectional(self) -> None:
        target = write_doc(
            self.repo,
            "docs/WRONG.md",
            "> 文档角色：decision\n"
            "> 生命周期状态：accepted\n"
            "> 当前真源：docs/A.md\n"
            "> 摘要：decision deliberately placed outside the ADR directory",
        )
        errors = docs_inventory.validate_document(
            docs_inventory.parse_document(self.repo, target), final=False
        )
        self.assertTrue(any("only allowed under docs/ADR" in error for error in errors))

    def test_invalid_blockquote_inside_metadata_fails(self) -> None:
        target = self.repo / "docs/BAD.md"
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            "# Bad\n\n> 文档角色：canonical\n> not key value\n> 生命周期状态：current\n",
            encoding="utf-8",
        )
        with self.assertRaises(docs_inventory.InventoryError):
            docs_inventory.parse_document(self.repo, target)

    def test_markdown_anchor_validation_preserves_chinese(self) -> None:
        self.assertIn("6-目录规则", docs_inventory.markdown_anchors("## 6. 目录规则\n"))


if __name__ == "__main__":
    unittest.main()
