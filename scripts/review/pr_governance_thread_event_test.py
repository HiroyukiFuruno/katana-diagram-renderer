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

    def test_out_of_scope_prs_are_skipped_before_the_polling_job(self) -> None:
        """The sensor scope must exactly match the dispatcher's local default-base domain."""

        self.assertNotRegex(
            self.sensor,
            r"(?ms)^  review-latch:\n.*?^    if:",
        )
        reject = re.search(
            r"(?ms)^      - name: Reject out-of-scope PR\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.sensor,
        )
        self.assertIsNotNone(reject)
        assert reject is not None
        reject_condition = re.search(r"(?m)^        if: (?P<value>.+)$", reject.group("body"))
        self.assertIsNotNone(reject_condition)
        assert reject_condition is not None
        for clause in (
            "github.event.pull_request.draft == false",
            "github.event.pull_request.base.repo.full_name == github.repository",
            "github.event.pull_request.head.repo.full_name == github.repository",
            "github.event.pull_request.base.ref == github.event.repository.default_branch",
        ):
            self.assertIn(clause.replace(" == ", " != "), reject_condition.group("value"))
        self.assertIn("||", reject_condition.group("value"))
        self.assertIn("exit 1", reject.group("body"))
        rerun_step = re.search(
            r"(?ms)^      - name: Reject sensor reruns\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.sensor,
        )
        self.assertIsNotNone(rerun_step)
        assert rerun_step is not None
        self.assertRegex(rerun_step.group("body"), r'\[\[ "\$\{RUN_ATTEMPT\}" != 1 \]\]')
        await_step = re.search(
            r"(?ms)^      - name: Await matching trusted governance Check Run\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.sensor,
        )
        self.assertIsNotNone(await_step)
        assert await_step is not None
        await_condition = re.search(r"(?m)^        if: (?P<value>.+)$", await_step.group("body"))
        self.assertIsNotNone(await_condition)
        assert await_condition is not None
        self.assertIn("github.run_attempt == 1", await_condition.group("value"))
        for clause in (
            "github.event.pull_request.draft == false",
            "github.event.pull_request.base.repo.full_name == github.repository",
            "github.event.pull_request.head.repo.full_name == github.repository",
            "github.event.pull_request.base.ref == github.event.repository.default_branch",
        ):
            self.assertIn(clause, await_condition.group("value"))
        self.assertIn("DEFAULT_BRANCH: ${{ github.event.repository.default_branch }}", self.sensor)
        self.assertIn("or pr_base_ref != default_branch", self.sensor)

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
                "DEFAULT_BRANCH": "master",
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
        self.assertIn("max_latch_api_reads = 300", self.sensor)
        self.assertNotIn('"--paginate"', self.sensor)
        self.assertIn("def check_run_page(", self.sensor)
        self.assertIn("check_run_page(2, terminal_page=True)", self.sensor)
        self.assertIn("Trusted governance Check Run response changed during pagination.", self.sensor)
        self.assertIn("terminal_page: bool = False", self.sensor)
        self.assertIn("timeout=20", self.sensor)
        self.assertIn("except subprocess.TimeoutExpired:", self.sensor)
        self.assertEqual(self.sensor.count("subprocess.run("), 1)
        self.assertIn('rel="next"', self.sensor)
        self.assertEqual(600 * 8.1, 4860)
        self.assertLessEqual((1 + (5400 + 60 - 1) // 60) * 3 + 3, 300)

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
                "DEFAULT_BRANCH": "master",
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
        repository_identity = (101, "repo", "https://api.github.com/repos/owner/repo")
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
            "repository": {"id": 101, "name": "repo", "url": repository_identity[2]},
            "pull_requests": [{
                "number": 72,
                "base": {"sha": base, "ref": "master", "repo": {"id": 101, "name": "repo", "url": repository_identity[2]}},
                "head": {"sha": head, "repo": {"id": 101, "name": "repo", "url": repository_identity[2]}},
            }],
        }
        assert callable(source_matches) and callable(writer_matches)
        self.assertTrue(source_matches(source, repository_identity))
        for changed in (
            {**source, "id": 18},
            {**source, "head_sha": "c" * 40},
            {**source, "repository": {"id": 101, "name": "repo", "url": repository_identity[2], "full_name": "other/repo"}},
            {**source, "status": "completed", "conclusion": "cancelled"},
            {**source, "pull_requests": [{**source["pull_requests"][0], "number": 73}]},
        ):
            self.assertFalse(source_matches(changed, repository_identity))

        writer = {
            "id": 91,
            "name": "PR governance status writer",
            "event": "workflow_dispatch",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "repository": {"id": 101, "name": "repo", "url": repository_identity[2]},
            "run_attempt": 1,
            "status": "in_progress",
        }
        self.assertTrue(writer_matches(writer, "91", repository_identity))
        self.assertFalse(writer_matches({**writer, "id": 92}, "91", repository_identity))
        self.assertFalse(writer_matches({**writer, "repository": {"id": 101, "name": "repo", "url": repository_identity[2], "full_name": "other/repo"}}, "91", repository_identity))
        self.assertFalse(writer_matches({**writer, "status": "completed", "conclusion": "failure"}, "91", repository_identity))

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
                "DEFAULT_BRANCH": "master",
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
            anchor_ids: list[int] | None = None,
            second_ids: list[int] | None = None,
            second_total: int | None = None,
            second_failure: bool = False,
            link_overflow: bool = False,
            timeout_page: int | None = None,
        ) -> tuple[object, list[list[str]]]:
            first = first_ids if first_ids is not None else list(range(1, min(total, 100) + 1))
            anchor = anchor_ids if anchor_ids is not None else first
            second = second_ids if second_ids is not None else list(range(101, total + 1))
            calls: list[list[str]] = []
            page_calls: dict[int, int] = {1: 0, 2: 0}

            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                self.assertEqual(_kwargs.get("timeout"), 20)
                calls.append(arguments)
                endpoint = arguments[-1]
                page = 1 if endpoint.endswith("page=1") else 2 if endpoint.endswith("page=2") else 0
                if page == 0:
                    raise AssertionError(f"Unexpected endpoint: {endpoint}")
                page_calls[page] += 1
                if timeout_page == page:
                    raise subprocess.TimeoutExpired(arguments, 20)
                def output(payload: object) -> str:
                    raw = json.dumps(payload)
                    if "--include" not in arguments:
                        return raw
                    link = 'Link: <https://api.github.com/next>; rel="next"\n' if link_overflow else ""
                    return f"HTTP/2 200\n{link}\n{raw}"
                if endpoint.endswith("page=1"):
                    values = first if page_calls[1] == 1 else anchor
                    return subprocess.CompletedProcess(arguments, 0, output({"total_count": total, "check_runs": [{"id": value} for value in values]}), "")
                if endpoint.endswith("page=2"):
                    if second_failure:
                        return subprocess.CompletedProcess(arguments, 1, "", "denied")
                    return subprocess.CompletedProcess(arguments, 0, output({"total_count": total if second_total is None else second_total, "check_runs": [{"id": value} for value in second]}), "")

            assert callable(reader)
            with patch.object(subprocess, "run", side_effect=fake_run):
                try:
                    return reader(), calls
                except SystemExit as error:
                    return error, calls

        for total, expected_calls in ((100, 2), (101, 3), (200, 3)):
            with self.subTest(total=total):
                result, calls = run_case(total)
                self.assertIsInstance(result, list)
                self.assertEqual(len(result), total)
                self.assertEqual(len(calls), expected_calls)
                self.assertTrue(all("--paginate" not in arguments for arguments in calls))
                self.assertTrue(all(arguments[-1].endswith(("page=1", "page=2")) for arguments in calls))
                self.assertTrue(all(arguments.count("--include") <= 1 for arguments in calls))

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

        result, calls = run_case(101, anchor_ids=list(range(1, 100)))
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 3)

        result, calls = run_case(101, anchor_ids=list(range(2, 102)))
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 3)

        result, calls = run_case(101, link_overflow=True)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 2)

        result, calls = run_case(101, timeout_page=2)
        self.assertIsInstance(result, SystemExit)
        self.assertEqual(len(calls), 2)


if __name__ == "__main__":
    unittest.main()
