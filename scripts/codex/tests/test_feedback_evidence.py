from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from scripts.codex import check_feedback_evidence


class FeedbackEvidenceTests(unittest.TestCase):
    def test_write_and_verify_local_bundle(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            bundle = repo / "feedback/2026-01-01/000001-case"
            bundle.mkdir(parents=True)
            (bundle / "issue.json").write_text("{}\n", encoding="utf-8")
            tracking = repo / "feedback/TRACKING.md"
            tracking.write_text(
                "# Tracking\n\n"
                "## Closure Buckets\n\n"
                "## Case Ledger\n\n"
                "| ID | Status | Area | Source | Closure / coverage |\n"
                "|---|---|---|---|---|\n"
                "| FB-20260101-000001 | closed | test | [folder](2026-01-01/000001-case) | covered |\n",
                encoding="utf-8",
            )
            check_feedback_evidence.write_tracking(repo)
            self.assertEqual(check_feedback_evidence.check_tracking(repo, verify_local=True), [])
            (bundle / "issue.json").write_text("changed\n", encoding="utf-8")
            self.assertTrue(
                any("digest drift" in error for error in check_feedback_evidence.check_tracking(repo, verify_local=True))
            )


if __name__ == "__main__":
    unittest.main()
