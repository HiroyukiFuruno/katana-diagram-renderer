from __future__ import annotations

import unittest
from pathlib import Path


class GovernanceReviewSensorContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.sensor = (root / ".github/workflows/pr-governance-review-events.yml").read_text(encoding="utf-8")
        self.writer = (root / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")

    def test_unsupported_review_thread_webhook_is_not_an_actions_trigger(self) -> None:
        self.assertNotIn("pull_request_review_thread:", self.sensor)
        self.assertNotIn("pull_request_review_thread", self.writer)

    def test_all_supported_review_sensor_events_are_discovered_and_bound(self) -> None:
        for event in ("pull_request", "pull_request_review", "pull_request_review_comment"):
            self.assertIn(f'"{event}"', self.writer)
        for contract in (
            'workflow_path_matches(run.get("path"), ".github/workflows/pr-governance-review-events.yml")',
            'run.get("run_attempt") == 1', 'len(pulls) != 1',
            'run_base.get("sha") == base', 'run_head.get("sha") == head',
        ):
            self.assertIn(contract, self.writer)

    def test_success_is_bound_to_review_sensor_and_later_event_pending_fence(self) -> None:
        self.assertIn("source_run_id", self.writer)
        self.assertIn("check_changed_since(head, pending)", self.writer)
        self.assertIn("return", self.writer[self.writer.index("def process"):])


if __name__ == "__main__":
    unittest.main()
