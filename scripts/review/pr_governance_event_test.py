from __future__ import annotations

import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from textwrap import dedent


class PrGovernanceReviewEventTest(unittest.TestCase):
    def setUp(self) -> None:
        self.repository = Path(__file__).parents[2]
        self.publisher = (
            self.repository / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        self.sensor = (
            self.repository / ".github/workflows/pr-governance-review-events.yml"
        ).read_text(encoding="utf-8")
        self.documentation = (
            self.repository / "docs/issue-driven-workflow.md"
        ).read_text(encoding="utf-8")

    def sensor_program(self) -> str:
        match = re.search(
            r"          python3 - <<'PY'\n(.*?)\n          PY",
            self.sensor,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def resolver_program(self) -> str:
        match = re.search(
            r"- name: Resolve PR targets from a trusted event.*?"
            r"          python3 - <<'PY'\n(.*?)\n          PY",
            self.publisher,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def fence_program(self, fence_id: str = "pending-governance-fence") -> str:
        step = self.publisher.index(f"id: {fence_id}")
        start = self.publisher.index("          python3 - <<'PY'\n", step) + len("          python3 - <<'PY'\n")
        end = self.publisher.index("\n          PY", start)
        return dedent(self.publisher[start:end])

    def run_fence(
        self,
        statuses: object,
        creator_id: str = "456",
        own_generation: str = "100",
        fence_id: str = "pending-governance-fence",
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_path = Path(temporary_directory)
            fake_gh = temporary_path / "gh"
            payload_path = temporary_path / "gh-output.json"
            payload_path.write_text(json.dumps(statuses), encoding="utf-8")
            fake_gh.write_text(
                "#!/bin/sh\n"
                "case \"$*\" in\n"
                "  */issues/*) printf '%s' \"${FAKE_GH_ISSUE}\" ;;\n"
                "  *) cat \"${FAKE_GH_FILE}\" ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            environment = os.environ.copy()
            environment.update(
                {
                    "FAKE_GH_FILE": str(payload_path),
                    "GITHUB_REPOSITORY": "owner/repository",
                    "STATUS_CREATOR_ID": creator_id,
                    "PENDING_STATUS_ID": "100",
                    "HEAD_SHA": "a" * 40,
                    "GITHUB_OUTPUT": str(temporary_path / "output"),
                    "PATH": f"{temporary_directory}{os.pathsep}{environment['PATH']}",
                }
            )
            program = self.fence_program(fence_id) + "\nprint('True' if tripped else 'False')\n"
            return subprocess.run(
                [sys.executable, "-c", program],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )

    def run_resolver(
        self,
        event_name: str,
        gh_output: object,
        *,
        issue_number: str = "64",
        issue_pull_request_url: str = "",
        run_id: str = "999",
        action: str = "opened",
        issue_payload: object | None = None,
        issue_api_exit: int = 0,
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_path = Path(temporary_directory)
            fake_gh = temporary_path / "gh"
            payload_path = temporary_path / "gh-output.json"
            payload_path.write_text(json.dumps(gh_output), encoding="utf-8")
            fake_gh.write_text(
                "#!/bin/sh\n"
                "case \"$*\" in\n"
                "  */issues/comments/*) printf '%s' \"${FAKE_GH_COMMENT}\" ;;\n"
                "  */issues/*)\n"
                "    if [ \"${FAKE_GH_ISSUE_EXIT}\" != 0 ]; then exit \"${FAKE_GH_ISSUE_EXIT}\"; fi\n"
                "    printf '%s' \"${FAKE_GH_ISSUE}\"\n"
                "    ;;\n"
                "  *) cat \"${FAKE_GH_FILE}\" ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            output_path = temporary_path / "github-output"
            environment = os.environ.copy()
            environment.update(
                {
                    "FAKE_GH_FILE": str(payload_path),
                    "EVENT_NAME": event_name,
                    "ISSUE_NUMBER": issue_number,
                    "ISSUE_PULL_REQUEST_URL": issue_pull_request_url,
                    "COMMENT_ID": "1001",
                    "COMMENT_CREATED_AT": "2026-08-29T00:00:00Z",
                    "COMMENT_UPDATED_AT": "2026-08-29T00:00:00Z",
                    "EVENT_ACTION": action,
                    "ISSUE_UPDATED_AT": "2026-08-29T00:00:00Z",
                    "FAKE_GH_ISSUE": json.dumps(
                        issue_payload
                        if issue_payload is not None
                        else {
                            "number": int(issue_number or "64"),
                            "updated_at": "2026-08-29T00:00:00Z",
                        }
                    ),
                    "FAKE_GH_ISSUE_EXIT": str(issue_api_exit),
                    "FAKE_GH_COMMENT": json.dumps(
                        {
                            "id": 1001,
                            "created_at": "2026-08-29T00:00:00Z",
                            "updated_at": "2026-08-29T00:00:00Z",
                            "issue_url": "https://api.github.com/repos/owner/repository/issues/64",
                        }
                    ),
                    "WORKFLOW_RUN_ID": "",
                    "GITHUB_RUN_ID": run_id,
                    "GITHUB_REPOSITORY": "owner/repository",
                    "GITHUB_OUTPUT": str(output_path),
                    "PATH": f"{temporary_directory}{os.pathsep}{environment['PATH']}",
                }
            )
            result = subprocess.run(
                [sys.executable, "-c", self.resolver_program()],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
            outputs: dict[str, str] = {}
            if output_path.exists():
                for line in output_path.read_text(encoding="utf-8").splitlines():
                    key, separator, value = line.partition("=")
                    if separator:
                        outputs[key] = value
            return result, outputs

    def run_sensor(self, statuses: object) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_path = Path(temporary_directory)
            fake_gh = temporary_path / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s' \"${FAKE_GH_OUTPUT}\"\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            environment = os.environ.copy()
            environment.update(
                {
                    "FAKE_GH_OUTPUT": json.dumps(statuses),
                    "GITHUB_REPOSITORY": "owner/repository",
                    "GH_TOKEN": "read-only-test-token",
                    "HEAD_SHA": "a" * 40,
                    "SOURCE_RUN_ID": "123",
                    "STATUS_CREATOR_ID": "456",
                    "POLL_INTERVAL_SECONDS": "1",
                    "POLL_TIMEOUT_SECONDS": "1",
                    "PATH": f"{temporary_directory}{os.pathsep}{environment['PATH']}",
                }
            )
            return subprocess.run(
                [sys.executable, "-c", self.sensor_program()],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )

    def test_sensor_creates_a_read_only_latch_for_pr_and_review_changes(self) -> None:
        self.assertIn("name: PR governance review sensor", self.sensor)
        self.assertIn("pull_request:", self.sensor)
        self.assertIn("pull_request_review:", self.sensor)
        self.assertIn("pull_request_review_comment:", self.sensor)
        for event_type in (
            "opened",
            "edited",
            "synchronize",
            "reopened",
            "ready_for_review",
            "converted_to_draft",
            "submitted",
            "edited",
            "dismissed",
            "created",
            "deleted",
        ):
            self.assertIn(f"- {event_type}", self.sensor)
        self.assertIn("actions: read", self.sensor)
        self.assertIn("statuses: read", self.sensor)
        self.assertNotIn("statuses: write", self.sensor)
        self.assertNotIn("actions/checkout", self.sensor)
        self.assertNotIn("\n        uses:", self.sensor)
        self.assertIn("name: KRR / PR governance review latch", self.sensor)
        self.assertIn("timeout-minutes: 15", self.sensor)
        self.assertIn("- name: Reject sensor reruns", self.sensor)
        self.assertIn("RUN_ATTEMPT: ${{ github.run_attempt }}", self.sensor)
        self.assertIn('if [[ "${RUN_ATTEMPT}" != 1 ]]', self.sensor)

    def test_publisher_accepts_only_the_server_generated_sensor_run(self) -> None:
        self.assertIn("workflow_run:", self.publisher)
        self.assertIn("- PR governance review sensor", self.publisher)
        self.assertIn("- requested", self.publisher)
        self.assertIn("actions: read", self.publisher)
        self.assertIn("actions/runs/{run_id}", self.publisher)
        self.assertIn('trusted_run.get("name") != "PR governance review sensor"', self.publisher)
        self.assertIn('trusted_run.get("event")', self.publisher)
        self.assertIn('"pull_request",', self.publisher)
        self.assertIn('trusted_run.get("path")', self.publisher)
        self.assertIn("return value == expected or (", self.publisher)
        self.assertIn('value.startswith(f"{expected}@")', self.publisher)
        self.assertIn('run_repository.get("full_name") != repository', self.publisher)
        self.assertIn("pull_number(trusted_run.get(\"pull_requests\"))", self.publisher)
        self.assertNotIn("  pull_request_review:\n", self.publisher)
        self.assertNotIn("  pull_request_review_comment:\n", self.publisher)
        self.assertNotIn("  pull_request_target:\n", self.publisher)

    def test_publisher_never_serializes_the_full_untrusted_event_payload(self) -> None:
        self.assertNotIn("EVENT_JSON", self.publisher)
        self.assertNotIn("toJSON(github.event)", self.publisher)
        self.assertNotIn("event.get(", self.publisher)
        self.assertIn("ISSUE_PULL_REQUEST_URL: ${{ github.event.issue.pull_request.url }}", self.publisher)
        self.assertIn("ISSUE_NUMBER: ${{ github.event.issue.number }}", self.publisher)
        self.assertIn("WORKFLOW_RUN_ID: ${{ github.event.workflow_run.id }}", self.publisher)
        self.assertIn("def positive_number", self.publisher)
        self.assertIn('re.fullmatch(r"[1-9][0-9]*", value)', self.publisher)
        self.assertIn('output.write(f"source_run_id={source_run_id}\\n")', self.publisher)

    def test_all_workflow_paths_are_not_pr_modifiable_and_threads_use_native_gate(self) -> None:
        verifier = (
            self.repository / "scripts/hooks/verify_push_issue.py"
        ).read_text(encoding="utf-8")
        self.assertIn('_WORKFLOW_DIRECTORY_PREFIX = ".github/workflows/"', verifier)
        self.assertIn("path.startswith(_WORKFLOW_DIRECTORY_PREFIX)", verifier)
        self.assertIn("GitHub Actions workflow", verifier)
        self.assertIn("required_conversation_resolution=true", self.documentation)

    def test_old_sensor_run_rechecks_the_current_pr_state(self) -> None:
        event_resolution = "- name: Resolve PR targets from a trusted event"
        current_pr = "- name: Resolve PR state and trusted default-branch SHA"
        pending_status = "- name: Publish pending governance state"
        self.assertLess(self.publisher.index(event_resolution), self.publisher.index(current_pr))
        self.assertLess(self.publisher.index(current_pr), self.publisher.index(pending_status))
        self.assertIn('"repos/${GITHUB_REPOSITORY}/pulls/${PR_NUMBER}"', self.publisher)
        self.assertIn("HEAD_SHA: ${{ steps.pull-request.outputs.head_sha }}", self.publisher)
        self.assertIn("SOURCE_RUN_ID: ${{ steps.event.outputs.source_run_id }}", self.publisher)
        self.assertIn("現在のbase/head/draft", self.documentation)

    def test_publisher_checks_out_only_the_api_resolved_default_branch_sha(self) -> None:
        resolver_start = self.publisher.index(
            "      - name: Resolve PR state and trusted default-branch SHA"
        )
        resolver_end = self.publisher.index(
            "\n      - name: Create scoped governance App token", resolver_start
        )
        resolver = self.publisher[resolver_start:resolver_end]
        checkout_start = self.publisher.index(
            "      - name: Check out trusted default-branch repository"
        )
        checkout_end = self.publisher.index("\n      - name: Verify PR readiness", checkout_start)
        checkout = self.publisher[checkout_start:checkout_end]

        self.assertIn(
            'default_branch="$(gh api "repos/${GITHUB_REPOSITORY}" --jq \'.default_branch\')"',
            resolver,
        )
        self.assertIn(
            'trusted_base_sha="$(gh api "repos/${GITHUB_REPOSITORY}/git/ref/heads/${default_branch}" --jq \'.object.sha\')"',
            resolver,
        )
        self.assertIn('echo "trusted_base_sha=${trusted_base_sha}"', resolver)
        self.assertIn("ref: ${{ steps.pull-request.outputs.trusted_base_sha }}", checkout)
        self.assertNotIn("ref: ${{ steps.pull-request.outputs.base_sha }}", checkout)

    def test_all_events_for_one_pr_share_a_concurrency_group(self) -> None:
        workflow, jobs = self.publisher.split("\njobs:\n", maxsplit=1)
        self.assertNotIn("\nconcurrency:", workflow)
        group_start = jobs.index("      group: pr-governance-")
        group_end = jobs.index("\n", group_start)
        group = jobs[group_start:group_end]
        self.assertIn("matrix.pr_number", group)
        self.assertNotIn("bound-", group)
        self.assertNotIn("issue-comment-", group)
        self.assertIn("cancel-in-progress: true", jobs)
        sensor_group_start = self.sensor.index("  group: pr-governance-review-latch-")
        sensor_group_end = self.sensor.index("\n", sensor_group_start)
        sensor_group = self.sensor[sensor_group_start:sensor_group_end]
        self.assertIn("github.event.pull_request.number", sensor_group)
        self.assertIn("cancel-in-progress: true", self.sensor)

    def test_issue_events_expand_all_open_prs_and_skip_non_pr_comments(self) -> None:
        self.assertIn("  issues:", self.publisher)
        for event_type in ("opened", "edited", "deleted", "transferred", "closed", "reopened"):
            self.assertIn(f"- {event_type}", self.publisher)
        self.assertIn("state=open", self.publisher)
        self.assertIn("--paginate", self.publisher)
        self.assertIn("ISSUE_PULL_REQUEST_URL", self.publisher)
        self.assertIn('source_run_id: str = ""', self.publisher)
        self.assertIn("matrix:", self.publisher)
        self.assertIn("pr_number", self.publisher)
        self.assertIn("github.event.issue.pull_request.url", self.publisher)

    def test_sensor_path_contract_rejects_a_different_prefix(self) -> None:
        start = self.publisher.index("          def sensor_workflow_path")
        end = self.publisher.index("\n          if event_name == \"workflow_run\":", start)
        namespace: dict[str, object] = {}
        exec(dedent(self.publisher[start:end]), namespace)
        is_sensor_path = namespace["sensor_workflow_path"]
        self.assertTrue(callable(is_sensor_path))
        assert callable(is_sensor_path)
        self.assertTrue(
            is_sensor_path(".github/workflows/pr-governance-review-events.yml")
        )
        self.assertTrue(
            is_sensor_path(".github/workflows/pr-governance-review-events.yml@master")
        )
        self.assertFalse(
            is_sensor_path(".github/workflows/pr-governance-review-events.yml-backup")
        )
        self.assertFalse(is_sensor_path(".github/workflows/other.yml@master"))

    def test_latch_accepts_only_a_bound_terminal_success_from_the_configured_app(self) -> None:
        self.assertIn("KRR_GOVERNANCE_STATUS_CREATOR_ID", self.sensor)
        self.assertIn('context = "KRR / PR governance (trusted)"', self.sensor)
        self.assertIn('source_values == [source_run_id]', self.sensor)
        self.assertIn('["gh", "api", "--paginate", "--slurp", endpoint]', self.sensor)
        self.assertIn('status.get("state") in {"success", "failure", "error"}', self.sensor)
        self.assertIn('str(creator["id"]) != creator_id', self.sensor)
        self.assertIn("Governance status creator does not match the configured App.", self.sensor)
        self.assertIn('if terminal["state"] == "success":', self.sensor)
        self.assertIn("Trusted governance publisher reported failure.", self.sensor)

    def test_latch_fails_closed_for_draft_api_errors_and_missing_bound_status(self) -> None:
        self.assertIn("- name: Reject Draft state", self.sensor)
        self.assertIn("exit 1", self.sensor)
        self.assertIn("Unable to read the trusted governance status.", self.sensor)
        self.assertIn("Timed out waiting for a matching trusted governance status.", self.sensor)
        self.assertIn("Multiple terminal governance statuses match this sensor run.", self.sensor)
        self.assertIn("issue commentまたはIssue revalidation起点のsourceなしstatusはsensor latchを解放しない", self.documentation)

    def test_latch_program_accepts_only_the_matching_app_success(self) -> None:
        bound_success = {
            "context": "KRR / PR governance (trusted)",
            "target_url": "https://github.com/owner/repository/actions/runs/999?source_run_id=123",
            "state": "success",
            "creator": {"id": 456},
        }
        result = self.run_sensor([[bound_success]])
        self.assertEqual(result.returncode, 0, result.stderr)

        cases = {
            "unbound": {
                **bound_success,
                "target_url": "https://github.com/owner/repository/actions/runs/999",
            },
            "creator-mismatch": {**bound_success, "creator": {"id": 999}},
            "publisher-failure": {**bound_success, "state": "failure"},
        }
        for name, status in cases.items():
            with self.subTest(name=name):
                result = self.run_sensor([[status]])
                self.assertNotEqual(result.returncode, 0)

    def test_resolver_scopes_issue_targets_to_non_draft_pr_closing_references(self) -> None:
        issue_prs = [
            [
                {
                    "number": 72,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes #64",
                },
                {
                    "number": 73,
                    "state": "open",
                    "draft": False,
                    "body": "Resolves https://github.com/owner/repository/issues/64",
                },
                {
                    "number": 74,
                    "state": "open",
                    "draft": False,
                    "body": "Refs #64",
                },
            ],
            [
                {
                    "number": 75,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes #65",
                },
                {
                    "number": 76,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/other/repository/issues/64",
                },
                {
                    "number": 77,
                    "state": "open",
                    "draft": True,
                    "body": "Fixes #64",
                },
            ],
        ]
        result, outputs = self.run_resolver(
            "issues",
            issue_prs,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["has_targets"], "true")
        self.assertEqual(
            json.loads(outputs["matrix"]),
            {
                "include": [
                    {"pr_number": "72", "source_run_id": "", "issue_generation_run_id": "999", "issue_number": "64", "issue_updated_at": "2026-08-29T00:00:00Z", "issue_action": "opened", "issue_source": "issues"},
                    {"pr_number": "73", "source_run_id": "", "issue_generation_run_id": "999", "issue_number": "64", "issue_updated_at": "2026-08-29T00:00:00Z", "issue_action": "opened", "issue_source": "issues"},
                ]
            },
        )

        result, outputs = self.run_resolver(
            "issue_comment", issue_prs, issue_number="64", issue_pull_request_url="", action="created"
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["has_targets"], "true")
        self.assertEqual(
            [target["pr_number"] for target in json.loads(outputs["matrix"])["include"]],
            ["72", "73"],
        )

    def test_resolver_rejects_issue_number_prefix_references(self) -> None:
        result, outputs = self.run_resolver(
            "issues",
            [[
                {
                    "number": 78,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes #640",
                },
                {
                    "number": 79,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/640",
                },
                {
                    "number": 80,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/64x",
                },
                {
                    "number": 81,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/64/extra",
                },
            ]],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(outputs["has_targets"], "false")
        self.assertEqual(json.loads(outputs["matrix"]), {"include": []})

    def test_resolver_accepts_exact_issue_url_with_terminal_punctuation_unicode_space_or_quote(self) -> None:
        result, outputs = self.run_resolver(
            "issues",
            [[
                {
                    "number": 82,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/64.",
                },
                {
                    "number": 83,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/64\u00a0",
                },
                {
                    "number": 84,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes https://github.com/owner/repository/issues/64'",
                },
                {
                    "number": 85,
                    "state": "open",
                    "draft": False,
                    "body": 'Fixes https://github.com/owner/repository/issues/64"',
                },
            ]],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            [target["pr_number"] for target in json.loads(outputs["matrix"])["include"]],
            ["82", "83", "84", "85"],
        )

    def test_resolver_fails_closed_when_more_than_256_ready_targets_match(self) -> None:
        result, outputs = self.run_resolver(
            "issues",
            [[
                {
                    "number": number,
                    "state": "open",
                    "draft": False,
                    "body": "Fixes #64",
                }
                for number in range(1, 258)
            ]],
            run_id="1234",
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertLess(len(result.stdout) + len(result.stderr), 16_384)
        self.assertEqual(outputs, {})

    def test_resolver_fails_closed_for_too_many_duplicate_or_invalid_targets(self) -> None:
        duplicate = [[
            {
                "number": 72,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
            },
            {
                "number": 72,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
            },
        ]]
        invalid_page = [{"number": 72, "state": "open", "draft": False, "body": "Fixes #64"}]
        malformed_pr = [[{"number": 72, "state": "open", "draft": "false", "body": "Fixes #64"}]]
        for name, payload in (
            ("duplicate", duplicate),
            ("invalid-page", invalid_page),
            ("malformed-pr", malformed_pr),
        ):
            with self.subTest(name=name):
                result, _ = self.run_resolver("issues", payload)
                self.assertNotEqual(result.returncode, 0)

    def test_issue_generation_fence_and_target_capacity_contract(self) -> None:
        self.assertIn("issue_generation_run_id", self.publisher)
        resolver = self.resolver_program()
        self.assertIn("MAX_MATRIX_TARGETS = 256", resolver)
        self.assertIn("if len(targets) > MAX_MATRIX_TARGETS:", resolver)
        self.assertIn("fail(", resolver)
        self.assertIn("verify_push_issue.py", self.publisher)
        success_start = self.publisher.index("      - name: Publish final governance state")
        success = self.publisher[success_start:]
        issue_contract_start = self.publisher.index("id: final-revalidation")
        self.assertLess(issue_contract_start, success_start)
        self.assertIn("FINAL_REVALIDATION_EXIT", success)
        self.assertIn("current_base", success)
        self.assertIn("current_head", success)
        self.assertIn("同一専用App・同一context", self.documentation)
        self.assertIn("GitHub API", self.documentation)
        self.assertIn("256件超", self.documentation)

    def test_issue_fence_accepts_only_valid_configured_creator_generations(self) -> None:
        def status(generation: object, creator: object = {"id": 456}) -> dict[str, object]:
            return {
                "id": int(generation) if isinstance(generation, int) else 101,
                "context": "KRR / PR governance (trusted)",
                "target_url": f"https://github.com/owner/repository/actions/runs/1?issue_generation_run_id={generation}",
                "creator": creator,
            }

        for fence_id in ("pending-governance-fence", "final-governance-fence"):
            with self.subTest(fence_id=fence_id):
                result = self.run_fence([[status(101)]], fence_id=fence_id)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), "True")
                result = self.run_fence(
                    [[
                        {
                            **status(101),
                            "target_url": "https://github.com/owner/repository/actions/runs/1?source_run_id=1",
                        }
                    ]],
                    fence_id=fence_id,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout.strip(), "True")
                for name, payload in (
                    ("creator-mismatch", [[status(101, {"id": 999})]]),
                    ("creator-missing", [[status(101, {})]]),
                    ("creator-wrong-type", [[status(101, {"id": "456"})]]),
                ):
                    with self.subTest(name=name):
                        result = self.run_fence(payload, fence_id=fence_id)
                        self.assertEqual(result.returncode, 0, result.stderr)
                        self.assertEqual(result.stdout.strip(), "False")

                for name, generation in (
                    ("generation-invalid", "not-a-number"),
                    ("generation-zero", "0"),
                    ("generation-duplicate", "101&issue_generation_run_id=102"),
                ):
                    with self.subTest(name=name):
                        result = self.run_fence([[status(generation)]], fence_id=fence_id)
                        self.assertNotEqual(result.returncode, 0)

                result = self.run_fence([[status(101)]], creator_id="not-a-number", fence_id=fence_id)
                self.assertNotEqual(result.returncode, 0)

    def test_large_unrelated_issue_revalidation_keeps_matrix_output_bounded(self) -> None:
        result, outputs = self.run_resolver(
            "issues",
            [[
                {
                    "number": number,
                    "state": "open",
                    "draft": False,
                    "body": "No closing reference to this Issue",
                }
                for number in range(1, 64001)
            ]],
            run_id="9876",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(outputs["matrix"]), {"include": []})
        self.assertEqual(outputs["has_targets"], "false")
        self.assertLess(len(outputs["matrix"]), 1024)
        self.assertLess(len("\n".join(f"{key}={value}" for key, value in outputs.items())), 1024)

    def test_issue_generation_order_and_final_verification_are_explicit(self) -> None:
        self.assertIn("issue_generation_run_id", self.publisher)
        self.assertIn("creator[\"id\"]", self.publisher)
        self.assertIn("marker_status_id > int(pending_status_id)", self.publisher)
        self.assertIn("同一専用App・同一context", self.documentation)
        self.assertIn("256件超、ページ・型・重複不正はfail-closed", self.documentation)
        final_start = self.publisher.index("      - name: Publish final governance state")
        before_final = self.publisher[:final_start]
        self.assertIn("scripts/hooks/verify_push_issue.py", before_final)
        self.assertIn("scripts/review/verify_pr_ready.py", before_final)
        self.assertIn("--allow-ready", before_final)

    def test_issue_delete_and_transfer_still_revalidate_all_open_pr_targets(self) -> None:
        pages = [[
            {
                "number": 72,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
            }
        ], [
            {
                "number": 81,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
            }
        ]]
        for action in ("deleted", "transferred"):
            with self.subTest(action=action):
                result, outputs = self.run_resolver(
                    "issues",
                    pages,
                    action=action,
                    issue_payload={"message": "Not Found"},
                    issue_api_exit=1,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                targets = json.loads(outputs["matrix"])["include"]
                self.assertEqual([target["pr_number"] for target in targets], ["72", "81"])
                self.assertTrue(all(target["issue_generation_run_id"] == "999" for target in targets))

    def test_issue_edit_close_and_reopen_require_current_issue_updated_at(self) -> None:
        page = [[
            {
                "number": 72,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
            }
        ]]
        for action in ("edited", "closed", "reopened"):
            with self.subTest(action=action):
                result, _ = self.run_resolver(
                    "issues",
                    page,
                    action=action,
                    issue_payload={
                        "number": 64,
                        "updated_at": "2026-08-28T23:59:59Z",
                    },
                )
                self.assertNotEqual(result.returncode, 0)

                result, _ = self.run_resolver(
                    "issues",
                    page,
                    action=action,
                    issue_api_exit=1,
                )
                self.assertNotEqual(result.returncode, 0)

    def test_issue_actions_bind_generation_and_require_current_issue_metadata(self) -> None:
        self.assertIn("ISSUE_ACTION", self.publisher)
        self.assertIn("ISSUE_UPDATED_AT", self.publisher)
        self.assertIn("updated_at", self.publisher)
        self.assertIn("issue_generation_run_id", self.publisher)
        for action in ("opened", "edited", "deleted", "transferred", "closed", "reopened"):
            self.assertIn(action, self.publisher)
        self.assertIn("action is invalid", self.publisher.lower())
        self.assertIn("Issue updated_at", self.publisher)

    def test_publisher_binds_pending_and_final_statuses_to_the_sensor_run(self) -> None:
        pending_start = self.publisher.index("- name: Publish pending governance state")
        checkout_start = self.publisher.index(
            "- name: Check out trusted default-branch repository"
        )
        pending = self.publisher[pending_start:checkout_start]
        final_start = self.publisher.index("- name: Publish final governance state")
        final = self.publisher[final_start:]
        for status_publisher in (pending, final):
            self.assertIn("SOURCE_RUN_ID: ${{ steps.event.outputs.source_run_id }}", status_publisher)
            self.assertIn('if [[ -n "${SOURCE_RUN_ID}" ]]', status_publisher)
            self.assertIn("?source_run_id=${SOURCE_RUN_ID}", status_publisher)
            self.assertIn('-f target_url="${target_url}"', status_publisher)
        self.assertIn("GitHub Actions `app_id=15368`", self.documentation)
        self.assertIn("repository Actionsのdefault `GITHUB_TOKEN` はread-only", self.documentation)

    def test_documented_bootstrap_and_reaction_boundary_are_observable(self) -> None:
        self.assertIn("使い捨てPR", self.documentation)
        self.assertIn("PR merge SHA", self.documentation)
        self.assertIn("final review marker commentを編集", self.documentation)
        self.assertIn("reactionの削除はGitHub Actionsのtrigger対象ではない", self.documentation)
        self.assertIn("Draftへ戻し、新しいfinal review証跡とReady化", self.documentation)

    def test_bootstrap_documentation_contract_keeps_normal_gate_fail_closed(self) -> None:
        external_script = "/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py"
        for token in (
            external_script,
            "activate",
            "finalize",
            "verify",
            "--expected-base",
            "--expected-head",
            "--expected-app-id",
            "--expected-diff-sha256",
            "--allowed-workflow",
            "--apply",
            "--smoke-pr",
            "KRR_GOVERNANCE_APP_JWT",
            "KRR_GOVERNANCE_APP_TOKEN",
            "CLI引数・出力へ出してはならない",
        ):
            self.assertIn(token, self.documentation)
        self.assertIn("PR外の専用GitHub App", self.documentation)
        self.assertIn("PR内のworkflow/branch/Issueを条件にした自己例外", self.documentation)
        self.assertIn("通常gateの代替ではない", self.documentation)

    def test_impl_release_skill_is_synchronized_with_canonical_governance_flow(self) -> None:
        canonical = (self.repository / ".codex/skills/impl-release/SKILL.md").read_text(
            encoding="utf-8"
        )
        repository_skill = (self.repository / ".agents/skills/impl-release/SKILL.md").read_text(
            encoding="utf-8"
        )
        self.assertIn("phase=initial head=${head_sha}", canonical)
        self.assertIn("phase=final head=${head_sha}", canonical)
        self.assertIn("gh pr create --draft", repository_skill)
        self.assertIn("phase=initial head=${head_sha}", repository_skill)
        self.assertIn("phase=final head=${head_sha}", repository_skill)
        self.assertIn("thread へ reply して resolve", repository_skill)
        self.assertIn('just pr-ready-check "<number>"', repository_skill)
        self.assertIn('just pr-ready-check "<number>" &&', repository_skill)
        self.assertIn("gh pr ready", repository_skill)
        self.assertIn("Ready 化後に merge 承認", repository_skill)
        self.assertNotIn('gh pr create --base master --head release/vX.Y.Z', repository_skill)

        for skill in (canonical, repository_skill):
            self.assertIn("初回 bootstrap", skill)
            self.assertIn("PR 外の専用 GitHub App", skill)
            self.assertIn("KRR / PR governance bootstrap", skill)
            self.assertIn("固定 HEAD SHA", skill)
            self.assertIn("参照Issueが OPEN", skill)
            self.assertIn("依存更新証跡", skill)
            self.assertIn("PR range の Issue contract", skill)
            self.assertIn("未 resolve thread 0", skill)
            self.assertIn("PR 内の workflow、branch 名、Issue、status を自己承認", skill)
            self.assertIn("merge 直後に bootstrap context を除去", skill)
            self.assertIn("KRR / PR governance (trusted)", skill)
            self.assertIn("KRR / PR governance review latch", skill)
            self.assertIn('just pr-ready-check "<number>"', skill)

    def test_commit_and_push_skill_is_synchronized_with_canonical_governance_flow(self) -> None:
        canonical = (self.repository / ".codex/skills/commit_and_push/SKILL.md").read_text(
            encoding="utf-8"
        )
        repository_skill = (self.repository / ".agents/skills/commit_and_push/SKILL.md").read_text(
            encoding="utf-8"
        )
        for skill in (canonical, repository_skill):
            self.assertIn("最新 push 後に `head_sha=$(git rev-parse HEAD)`", skill)
            self.assertIn("phase=final head=${head_sha}", skill)
            self.assertIn("未 resolve 数が 0", skill)
            self.assertIn('just pr-ready-check "<number>"', skill)
            self.assertIn("参照Issueが OPEN", skill)
            self.assertIn("依存更新証跡", skill)
            self.assertIn("PR range の Issue contract", skill)


if __name__ == "__main__":
    unittest.main()
