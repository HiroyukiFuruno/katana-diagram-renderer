from __future__ import annotations

import os
import re
import sys
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit


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

    def test_sensor_accepts_only_generation_scoped_trusted_check_ids(self) -> None:
        """Keep the sensor bound to writer/dispatcher immutable generations."""

        start = self.sensor.index("          check_name =")
        end = self.sensor.index("          deadline =", start)
        program = textwrap.dedent(self.sensor[start:end])
        head = "A" * 40
        with patch.dict(
            os.environ,
            {
                "HEAD_SHA": head,
                "SOURCE_RUN_ID": "17",
                "CHECK_APP_ID": "4766933",
                "POLL_INTERVAL_SECONDS": "5",
                "POLL_TIMEOUT_SECONDS": "840",
            },
            clear=False,
        ):
            namespace: dict[str, object] = {
                "os": os,
                "parse_qs": parse_qs,
                "re": re,
                "sys": sys,
                "urlsplit": urlsplit,
            }
            exec(program, namespace)

        matcher = namespace["check_matches_source"]
        self.assertTrue(callable(matcher))

        def check(external_id: object, *, bound_head: str = head) -> dict[str, object]:
            return {
                "name": "KRR / PR governance (trusted check)",
                "app": {"id": 4766933},
                "head_sha": bound_head,
                "external_id": external_id,
                "details_url": "https://github.com/owner/repo/actions/runs/17?source_run_id=17",
            }

        assert callable(matcher)
        for external_id in (
            f"krr-governance/v1/{head.lower()}/writer-1",
            f"krr-governance/v1/{head.lower()}/dispatcher-9",
        ):
            self.assertTrue(matcher(check(external_id)), external_id)
        for external_id in (
            f"krr-governance/v1/{head.lower()}",
            f"krr-governance/v1/{head.lower()}/writer-0",
            f"krr-governance/v1/{head.lower()}/dispatcher--1",
            f"krr-governance/v1/{head.lower()}/writer-1-extra",
            f"krr-governance/v1/{'b' * 40}/writer-1",
            None,
        ):
            self.assertFalse(matcher(check(external_id)), external_id)
        self.assertFalse(matcher(check(f"krr-governance/v1/{head.lower()}/writer-1", bound_head=head.lower())))


if __name__ == "__main__":
    unittest.main()
