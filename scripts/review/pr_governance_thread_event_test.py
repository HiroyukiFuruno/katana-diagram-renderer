from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlencode, urlsplit


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

    def test_sensor_accepts_only_current_writer_generation_scoped_check_ids(self) -> None:
        """Keep the sensor bound to an exact writer run and immutable generation."""

        start = self.sensor.index("          check_name =")
        end = self.sensor.index("          deadline =", start)
        program = textwrap.dedent(self.sensor[start:end])
        head = "A" * 40
        with patch.dict(
            os.environ,
            {
                "HEAD_SHA": head,
                "PR_NUMBER": "72",
                "PR_BASE_SHA": "B" * 40,
                "PR_BASE_REF": "master",
                "SOURCE_RUN_ID": "17",
                "CHECK_APP_ID": "4766933",
                "POLL_INTERVAL_SECONDS": "60",
                "POLL_TIMEOUT_SECONDS": "5400",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_SERVER_URL": "https://github.com",
            },
            clear=False,
        ):
            namespace: dict[str, object] = {
                "os": os,
                "parse_qs": parse_qs,
                "re": re,
                "sys": sys,
                "urlencode": urlencode,
                "urlsplit": urlsplit,
            }
            exec(program, namespace)

        matcher = namespace["check_matches_source"]
        self.assertTrue(callable(matcher))

        def check(
            external_id: object,
            *,
            bound_head: str = head,
            details_url: str = "https://github.com/owner/repo/actions/runs/1?source_run_id=17",
        ) -> dict[str, object]:
            return {
                "name": "KRR / PR governance (trusted check)",
                "app": {"id": 4766933},
                "head_sha": bound_head,
                "external_id": external_id,
                "details_url": details_url,
            }

        assert callable(matcher)
        for external_id in (
            f"krr-governance/v1/{head.lower()}/writer-1",
        ):
            self.assertTrue(matcher(check(external_id)), external_id)
        for external_id in (
            f"krr-governance/v1/{head.lower()}",
            f"krr-governance/v1/{head.lower()}/writer-0",
            f"krr-governance/v1/{head.lower()}/dispatcher--1",
            f"krr-governance/v1/{head.lower()}/writer-1-extra",
            f"krr-governance/v1/{head.lower()}/dispatcher-1",
            f"krr-governance/v1/{'b' * 40}/writer-1",
            None,
        ):
            self.assertFalse(matcher(check(external_id)), external_id)
        self.assertFalse(matcher(check(f"krr-governance/v1/{head.lower()}/writer-1", bound_head=head.lower())))
        self.assertFalse(matcher(check(f"krr-governance/v1/{head.lower()}/writer-2")))
        self.assertFalse(matcher(check(
            f"krr-governance/v1/{head.lower()}/writer-1",
            details_url="https://github.com/other/repo/actions/runs/1?source_run_id=17",
        )))
        self.assertFalse(matcher(check(
            f"krr-governance/v1/{head.lower()}/writer-1",
            details_url="https://github.example/owner/repo/actions/runs/1?source_run_id=17",
        )))

    def test_sensor_allows_the_600_head_paced_reconciliation_with_a_bounded_api_budget(self) -> None:
        """The latch covers the complete 600-head generation, not an arbitrary long wait."""

        self.assertIn("timeout-minutes: 95", self.sensor)
        self.assertIn("POLL_INTERVAL_SECONDS: '60'", self.sensor)
        self.assertIn("POLL_TIMEOUT_SECONDS: '5400'", self.sensor)
        self.assertIn("max_reconciliation_heads = 600", self.sensor)
        self.assertIn("check_write_pace_seconds = 8.1", self.sensor)
        self.assertIn("max_latch_timeout_seconds = 5400", self.sensor)
        self.assertIn("max_latch_api_reads = 200", self.sensor)
        self.assertNotIn('"--paginate"', self.sensor)
        self.assertIn("def check_run_page(page: int)", self.sensor)
        self.assertIn("check_run_page(2)", self.sensor)
        self.assertEqual(600 * 8.1, 4860)
        self.assertLessEqual((1 + (5400 + 60 - 1) // 60) * 2 + 3, 200)

    def test_sensor_revalidation_rejects_wrong_source_head_pr_repo_and_writer_run(self) -> None:
        """A completed Check Run cannot bypass the sensor/writer generation fences."""

        start = self.sensor.index("          check_name =")
        end = self.sensor.index("          deadline =", start)
        program = textwrap.dedent(self.sensor[start:end])
        head, base = "a" * 40, "b" * 40
        with patch.dict(
            os.environ,
            {
                "HEAD_SHA": head,
                "PR_NUMBER": "72",
                "PR_BASE_SHA": base,
                "PR_BASE_REF": "master",
                "SOURCE_RUN_ID": "17",
                "CHECK_APP_ID": "4766933",
                "POLL_INTERVAL_SECONDS": "60",
                "POLL_TIMEOUT_SECONDS": "5400",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_SERVER_URL": "https://github.com",
            },
            clear=False,
        ):
            namespace: dict[str, object] = {
                "os": os,
                "parse_qs": parse_qs,
                "re": re,
                "sys": sys,
                "urlencode": urlencode,
                "urlsplit": urlsplit,
            }
            exec(program, namespace)

        source_matches = namespace["sensor_run_matches"]
        writer_matches = namespace["writer_run_matches"]
        self.assertTrue(callable(source_matches))
        self.assertTrue(callable(writer_matches))
        source = {
            "id": 17,
            "name": "PR governance review sensor",
            "event": "pull_request_review",
            "path": ".github/workflows/pr-governance-review-events.yml@master",
            "head_sha": head,
            "status": "in_progress",
            "conclusion": None,
            "run_attempt": 1,
            "run_number": 4,
            "repository": {"full_name": "owner/repo"},
            "pull_requests": [{
                "number": 72,
                "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repo"}},
                "head": {"sha": head, "repo": {"full_name": "owner/repo"}},
            }],
        }
        assert callable(source_matches) and callable(writer_matches)
        self.assertTrue(source_matches(source))
        for changed in (
            {**source, "id": 18},
            {**source, "head_sha": "c" * 40},
            {**source, "repository": {"full_name": "other/repo"}},
            {**source, "status": "completed", "conclusion": "cancelled"},
            {**source, "pull_requests": [{**source["pull_requests"][0], "number": 73}]},
        ):
            self.assertFalse(source_matches(changed))

        writer = {
            "id": 91,
            "name": "PR governance status writer",
            "event": "workflow_dispatch",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "repository": {"full_name": "owner/repo"},
            "run_attempt": 1,
            "status": "in_progress",
        }
        self.assertTrue(writer_matches(writer, "91"))
        self.assertFalse(writer_matches({**writer, "id": 92}, "91"))
        self.assertFalse(writer_matches({**writer, "repository": {"full_name": "other/repo"}}, "91"))
        self.assertFalse(writer_matches({**writer, "status": "completed", "conclusion": "failure"}, "91"))

    def test_sensor_reads_at_most_two_explicit_check_run_pages_and_rejects_bad_boundaries(self) -> None:
        """A poll has a fixed request ceiling even when GitHub reports many Check Runs."""

        start = self.sensor.index("          check_name =")
        end = self.sensor.index("          deadline =", start)
        program = textwrap.dedent(self.sensor[start:end])
        head, base = "a" * 40, "b" * 40
        with patch.dict(
            os.environ,
            {
                "HEAD_SHA": head,
                "PR_NUMBER": "72",
                "PR_BASE_SHA": base,
                "PR_BASE_REF": "master",
                "SOURCE_RUN_ID": "17",
                "CHECK_APP_ID": "4766933",
                "POLL_INTERVAL_SECONDS": "60",
                "POLL_TIMEOUT_SECONDS": "5400",
                "GITHUB_REPOSITORY": "owner/repo",
                "GITHUB_SERVER_URL": "https://github.com",
            },
            clear=False,
        ):
            namespace: dict[str, object] = {
                "json": json,
                "os": os,
                "parse_qs": parse_qs,
                "re": re,
                "subprocess": subprocess,
                "sys": sys,
                "urlencode": urlencode,
                "urlsplit": urlsplit,
            }
            exec(program, namespace)

        reader = namespace["trusted_check_runs"]
        self.assertTrue(callable(reader))

        def run_case(
            total: int,
            *,
            first_ids: list[int] | None = None,
            second_ids: list[int] | None = None,
            second_total: int | None = None,
            second_failure: bool = False,
        ) -> tuple[object, list[list[str]]]:
            first = first_ids if first_ids is not None else list(range(1, min(total, 100) + 1))
            second = second_ids if second_ids is not None else list(range(101, total + 1))
            calls: list[list[str]] = []

            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                calls.append(arguments)
                endpoint = arguments[-1]
                if endpoint.endswith("page=1"):
                    return subprocess.CompletedProcess(arguments, 0, json.dumps({"total_count": total, "check_runs": [{"id": value} for value in first]}), "")
                if endpoint.endswith("page=2"):
                    if second_failure:
                        return subprocess.CompletedProcess(arguments, 1, "", "denied")
                    return subprocess.CompletedProcess(arguments, 0, json.dumps({"total_count": total if second_total is None else second_total, "check_runs": [{"id": value} for value in second]}), "")
                raise AssertionError(f"Unexpected endpoint: {endpoint}")

            assert callable(reader)
            with patch.object(subprocess, "run", side_effect=fake_run):
                try:
                    return reader(), calls
                except SystemExit as error:
                    return error, calls

        for total, expected_calls in ((100, 1), (101, 2), (200, 2)):
            with self.subTest(total=total):
                result, calls = run_case(total)
                self.assertIsInstance(result, list)
                self.assertEqual(len(result), total)
                self.assertEqual(len(calls), expected_calls)
                self.assertTrue(all("--paginate" not in arguments for arguments in calls))

        result, calls = run_case(201)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 1)

        result, calls = run_case(101, second_failure=True)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 2)

        result, calls = run_case(101, second_total=100)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 2)

        result, calls = run_case(100, first_ids=[1] * 100)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 1)

        result, calls = run_case(101, second_ids=[100])
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()
