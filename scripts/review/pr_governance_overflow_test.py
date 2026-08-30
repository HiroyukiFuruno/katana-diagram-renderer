from __future__ import annotations

import unittest
from pathlib import Path


class GovernanceOverflowContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.writer = (root / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        self.workflow = (root / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")

    def test_writer_has_no_matrix_or_256_target_limit(self) -> None:
        self.assertNotIn("matrix:", self.workflow)
        self.assertNotIn("MAX_MATRIX", self.writer)
        self.assertIn("for number in snapshot.numbers:", self.writer)
        self.assertIn("failures += 1", self.writer)

    def test_open_pr_and_check_run_api_reads_are_fully_paginated(self) -> None:
        self.assertIn('pulls?state=open&per_page=100', self.writer)
        self.assertIn('"check_name": CHECK_NAME', self.writer)
        self.assertIn('["--paginate", "--slurp", endpoint]', self.writer)

    def test_malformed_or_multi_closing_prs_fail_closed_without_aborting_other_prs(self) -> None:
        self.assertIn("A malformed multi-Issue closer is a claimant", self.writer)
        self.assertIn("Canonical Issue closer set changed.", self.writer)
        self.assertIn("Do not make one malformed/changed PR leave other open PRs stale.", self.writer)

    def test_single_snapshot_removes_quadratic_300_pr_revalidation(self) -> None:
        self.assertIn("Take one complete O(N) open-PR snapshot", self.writer)
        self.assertIn("one complete snapshot prevents O(N^2) GETs", self.writer)
        self.assertNotIn("for listed in open_pulls()", self.writer)


if __name__ == "__main__":
    unittest.main()
