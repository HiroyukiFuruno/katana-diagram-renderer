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
        self.assertIn("PULL_REQUEST_NUMBER: ${{ github.event.pull_request.number }}", self.publisher)
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
        event_resolution = "- name: Resolve a PR number from a trusted event"
        current_pr = "- name: Resolve trusted base SHA"
        pending_status = "- name: Publish pending governance state"
        self.assertLess(self.publisher.index(event_resolution), self.publisher.index(current_pr))
        self.assertLess(self.publisher.index(current_pr), self.publisher.index(pending_status))
        self.assertIn('"repos/${GITHUB_REPOSITORY}/pulls/${PR_NUMBER}"', self.publisher)
        self.assertIn("HEAD_SHA: ${{ steps.pull-request.outputs.head_sha }}", self.publisher)
        self.assertIn("SOURCE_RUN_ID: ${{ steps.event.outputs.source_run_id }}", self.publisher)
        self.assertIn("現在のbase/head/draft", self.documentation)

    def test_all_events_for_one_pr_share_a_concurrency_group(self) -> None:
        group_start = self.publisher.index("  group: pr-governance-")
        group_end = self.publisher.index("\n", group_start)
        group = self.publisher[group_start:group_end]
        self.assertIn("github.event.issue.number", group)
        self.assertIn("github.event.workflow_run.pull_requests[0].number", group)
        self.assertIn("bound-{0}", group)
        self.assertIn("issue-comment-{0}", group)
        self.assertLess(
            group.index("github.event.workflow_run.pull_requests[0].number"),
            group.index("github.event.workflow_run.id"),
        )
        sensor_group_start = self.sensor.index("  group: pr-governance-review-latch-")
        sensor_group_end = self.sensor.index("\n", sensor_group_start)
        sensor_group = self.sensor[sensor_group_start:sensor_group_end]
        self.assertIn("github.event.pull_request.number", sensor_group)
        self.assertIn("cancel-in-progress: true", self.sensor)

    def test_sensor_path_contract_rejects_a_different_prefix(self) -> None:
        start = self.publisher.index("def sensor_workflow_path")
        end = self.publisher.index("\n\n          source_run_id = \"\"", start)
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
        self.assertIn("issue comment起点のunbound statusはsensor latchを解放しない", self.documentation)

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

    def test_publisher_binds_pending_and_final_statuses_to_the_sensor_run(self) -> None:
        pending_start = self.publisher.index("- name: Publish pending governance state")
        checkout_start = self.publisher.index("- name: Check out trusted base repository")
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
        self.assertIn("just PR=<number> pr-ready-check", repository_skill)
        self.assertIn('just PR=<number> pr-ready-check && gh pr ready "${pr_url}"', repository_skill)
        self.assertIn("gh pr ready", repository_skill)
        self.assertIn("Ready 化後に merge 承認", repository_skill)
        self.assertNotIn('gh pr create --base master --head release/vX.Y.Z', repository_skill)


if __name__ == "__main__":
    unittest.main()
