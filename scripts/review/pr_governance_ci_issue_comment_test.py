from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from textwrap import dedent


class PrGovernanceCiIssueCommentTest(unittest.TestCase):
    def setUp(self) -> None:
        self.workflow = (
            Path(__file__).parents[2] / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")

    def resolver(self) -> str:
        match = re.search(
            r"- name: Resolve PR targets from a trusted event.*?"
            r"python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def ci_source_fence(self, name: str) -> str:
        start = self.workflow.index(f"- name: {name}")
        match = re.search(
            r"python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow[start:],
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def final_revalidation_script(self) -> str:
        start = self.workflow.index("- name: Revalidate final governance contract before success")
        match = re.search(r"run: \|\n(.*?)(?=\n      - name:)", self.workflow[start:], re.DOTALL)
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def run_final_revalidation(self, action: str) -> tuple[subprocess.CompletedProcess[str], str, str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            bin_directory = directory / "bin"
            bin_directory.mkdir()
            output = directory / "output"
            log = directory / "gh.log"
            base_sha = "b" * 40
            head_sha = "a" * 40
            timestamp = "2026-08-29T00:00:00Z"
            (bin_directory / "gh").write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' \"$*\" >> \"${FAKE_GH_LOG}\"\n"
                "case \"$*\" in\n"
                f"  *'/pulls/72'*'.base.sha'*) printf '%s\\n' '{base_sha}' ;;\n"
                f"  *'/pulls/72'*'.head.sha'*) printf '%s\\n' '{head_sha}' ;;\n"
                "  *'/pulls/72'*'.head.ref'*) printf '%s\\n' 'governance-test' ;;\n"
                f"  *'/issues/64'*'.updated_at'*) printf '%s\\n' '{timestamp}' ;;\n"
                "  *'/issues/comments/3'*'.id'*) printf '%s\\n' '3' ;;\n"
                f"  *'/issues/comments/3'*'.created_at'*) printf '%s\\n' '{timestamp}' ;;\n"
                f"  *'/issues/comments/3'*'.updated_at'*) printf '%s\\n' '{timestamp}' ;;\n"
                "  *) exit 97 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            (bin_directory / "python3").write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            for executable in (bin_directory / "gh", bin_directory / "python3"):
                executable.chmod(0o755)
            environment = os.environ | {
                "PATH": f"{bin_directory}:{os.environ['PATH']}",
                "FAKE_GH_LOG": str(log),
                "GITHUB_OUTPUT": str(output),
                "GITHUB_REPOSITORY": "owner/repository",
                "PR_NUMBER": "72", "ISSUE_NUMBER": "64",
                "ISSUE_UPDATED_AT": timestamp, "ISSUE_ACTION": action,
                "ISSUE_SOURCE": "issue_comment", "COMMENT_ID": "3",
                "COMMENT_CREATED_AT": timestamp, "COMMENT_UPDATED_AT": timestamp,
            }
            result = subprocess.run(
                ["/bin/bash", "-c", self.final_revalidation_script()],
                capture_output=True, text=True, env=environment, check=False,
            )
            return result, output.read_text(encoding="utf-8"), log.read_text(encoding="utf-8")

    def test_ci_and_release_workflow_runs_have_strict_server_side_sources(self) -> None:
        resolver = self.resolver()
        self.assertIn("- CI", self.workflow)
        self.assertIn("- release-preflight", self.workflow)
        self.assertIn("- requested", self.workflow)
        self.assertIn("- in_progress", self.workflow)
        self.assertIn("- completed", self.workflow)
        self.assertIn('"CI": ".github/workflows/test-and-build.yml"', resolver)
        self.assertIn(
            '"release-preflight": ".github/workflows/release-preflight.yml"',
            resolver,
        )
        self.assertIn('run_event != "pull_request"', resolver)
        self.assertIn('trusted_run.get("repository")', resolver)
        self.assertIn('trusted_run.get("head_sha")', resolver)
        self.assertIn('for field in ("run_number", "run_attempt")', resolver)
        self.assertIn('trusted_run.get("workflow_id")', resolver)
        self.assertIn("pull request does not bind trusted base and head", resolver)
        self.assertIn("default_branch_workflow_blob(expected_path, run_head_sha)", resolver)
        self.assertNotIn('"PR governance publisher"', resolver)

    def test_requested_ci_invalidates_an_old_success_with_pending_and_completed_failure(self) -> None:
        self.assertIn('conclusion = "pending"', self.workflow)
        self.assertIn('run_action in {"requested", "in_progress"}', self.resolver())
        self.assertIn("Trusted CI or release-preflight source is pending.", self.workflow)
        self.assertIn('"${SOURCE_KIND}" == ci && "${SOURCE_CONCLUSION}" == pending', self.workflow)
        self.assertIn('"${SOURCE_KIND}" == ci && "${SOURCE_CONCLUSION}" != success', self.workflow)
        self.assertIn("Fence CI source against the current PR head", self.workflow)
        self.assertIn('source["head_sha"] != os.environ["EXPECTED_HEAD_SHA"]', self.workflow)
        self.assertIn('source.get("status") != "completed"', self.workflow)
        self.assertIn("source_run_attempt=${SOURCE_RUN_ATTEMPT}", self.workflow)
        self.assertIn("source_workflow_id=${SOURCE_WORKFLOW_ID}", self.workflow)
        self.assertIn("source_head_sha=${SOURCE_HEAD_SHA}", self.workflow)
        self.assertIn("source_base_sha=${SOURCE_BASE_SHA}", self.workflow)
        self.assertIn("Revalidate final CI source generation", self.workflow)
        self.assertIn("SOURCE_GENERATIONS", self.workflow)
        self.assertIn("max(generations)", self.workflow)

    def test_ci_source_fences_reject_a_base_update_after_the_run_started(self) -> None:
        source_run = {
            "head_sha": "a" * 40,
            "run_attempt": 2,
            "run_number": 8,
            "workflow_id": 44,
            "status": "in_progress",
            "pull_requests": [{
                "base": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}},
                "head": {"repo": {"full_name": "owner/repository"}},
            }],
        }
        generations = [{"workflow_runs": [{
            "event": "pull_request", "head_sha": "a" * 40,
            "run_number": 8, "run_attempt": 2,
        }]}]
        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository",
            "SOURCE_RUN_JSON": __import__("json").dumps(source_run),
            "SOURCE_GENERATIONS": __import__("json").dumps(generations),
            "SOURCE_RUN_ATTEMPT": "2", "SOURCE_RUN_NUMBER": "8",
            "SOURCE_WORKFLOW_ID": "44", "SOURCE_HEAD_SHA": "a" * 40,
            "SOURCE_BASE_SHA": "b" * 40, "EXPECTED_HEAD_SHA": "a" * 40,
            "EXPECTED_BASE_SHA": "c" * 40, "SOURCE_CONCLUSION": "pending",
        }
        for name in (
            "Fence CI source against the current PR head",
            "Revalidate final CI source generation",
        ):
            result = subprocess.run(
                [sys.executable, "-c", self.ci_source_fence(name)],
                capture_output=True, text=True, env=environment, check=False,
            )
            self.assertEqual(result.returncode, 1, f"{name}: {result.stderr}")

    def test_ci_source_fences_reject_a_refetched_pull_with_a_different_head(self) -> None:
        source_run = {
            "head_sha": "a" * 40,
            "run_attempt": 2,
            "run_number": 8,
            "workflow_id": 44,
            "status": "in_progress",
            "pull_requests": [{
                "base": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}},
                "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}},
            }],
        }
        generations = [{"workflow_runs": [{
            "event": "pull_request", "head_sha": "a" * 40,
            "run_number": 8, "run_attempt": 2,
        }]}]
        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository",
            "SOURCE_RUN_JSON": __import__("json").dumps(source_run),
            "SOURCE_GENERATIONS": __import__("json").dumps(generations),
            "SOURCE_RUN_ATTEMPT": "2", "SOURCE_RUN_NUMBER": "8",
            "SOURCE_WORKFLOW_ID": "44", "SOURCE_HEAD_SHA": "a" * 40,
            "SOURCE_BASE_SHA": "b" * 40, "EXPECTED_HEAD_SHA": "a" * 40,
            "EXPECTED_BASE_SHA": "b" * 40, "SOURCE_CONCLUSION": "pending",
        }
        for name in (
            "Fence CI source against the current PR head",
            "Revalidate final CI source generation",
        ):
            result = subprocess.run(
                [sys.executable, "-c", self.ci_source_fence(name)],
                capture_output=True, text=True, env=environment, check=False,
            )
            self.assertEqual(result.returncode, 1, f"{name}: {result.stderr}")

    def test_ci_source_fences_accept_only_the_current_generation(self) -> None:
        source_run = {
            "head_sha": "a" * 40,
            "run_attempt": 2,
            "run_number": 8,
            "workflow_id": 44,
            "status": "in_progress",
            "pull_requests": [{
                "base": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}},
                "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}},
            }],
        }
        base_environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository",
            "SOURCE_RUN_JSON": json.dumps(source_run),
            "SOURCE_RUN_ATTEMPT": "2", "SOURCE_RUN_NUMBER": "8",
            "SOURCE_WORKFLOW_ID": "44", "SOURCE_HEAD_SHA": "a" * 40,
            "SOURCE_BASE_SHA": "b" * 40, "EXPECTED_HEAD_SHA": "a" * 40,
            "EXPECTED_BASE_SHA": "b" * 40, "SOURCE_CONCLUSION": "pending",
        }
        names = (
            "Fence CI source against the current PR head",
            "Revalidate final CI source generation",
        )
        for latest, expected_exit in (((8, 2), 0), ((8, 3), 1), ((9, 1), 1)):
            runs = [{
                "event": "pull_request", "head_sha": "a" * 40,
                "run_number": 8, "run_attempt": 2,
            }]
            if latest != (8, 2):
                runs.append({
                    "event": "pull_request", "head_sha": "a" * 40,
                    "run_number": latest[0], "run_attempt": latest[1],
                })
            generations = [{"workflow_runs": runs}]
            environment = base_environment | {"SOURCE_GENERATIONS": json.dumps(generations)}
            for name in names:
                result = subprocess.run(
                    [sys.executable, "-c", self.ci_source_fence(name)],
                    capture_output=True, text=True, env=environment, check=False,
                )
                self.assertEqual(result.returncode, expected_exit, f"{latest}, {name}: {result.stderr}")

    def test_pr_workflow_blob_must_match_the_default_branch_without_checkout(self) -> None:
        resolver = self.resolver()
        self.assertIn("/contents/{path}?ref={ref}", resolver)
        self.assertIn("PR workflow blob differs from the trusted default branch.", resolver)
        self.assertIn('payload.get("type") != "file"', resolver)
        self.assertNotIn("actions/checkout", resolver)
        self.assertNotIn("subprocess.run([\"git\"", resolver)

    def test_non_pr_issue_comment_revalidates_all_matching_ready_prs(self) -> None:
        resolver = self.resolver()
        self.assertIn("def comment_freshness", resolver)
        self.assertIn("def closing_targets", resolver)
        self.assertIn("--paginate", resolver)
        self.assertIn("--slurp", resolver)
        self.assertIn('f"repos/{repository}/issues/comments/{comment_id}"', resolver)
        self.assertIn('action not in {"created", "edited", "deleted"}', resolver)
        self.assertIn("Issue changed before comment target resolution completed.", resolver)
        self.assertIn('"issue_comment"', resolver)
        self.assertIn("if action != \"deleted\"", resolver)
        self.assertIn("Issue comment freshness timestamp is invalid.", resolver)

    def test_final_comment_revalidation_binds_issue_and_comment_timestamps(self) -> None:
        final_start = self.workflow.index("- name: Revalidate final governance contract before success")
        final_end = self.workflow.index("\n      - name: Fence final status", final_start)
        final = self.workflow[final_start:final_end]
        for variable in (
            "ISSUE_SOURCE",
            "COMMENT_ID",
            "COMMENT_CREATED_AT",
            "COMMENT_UPDATED_AT",
        ):
            self.assertIn(variable, final)
        self.assertIn("issue_comment:created|issue_comment:edited|issue_comment:deleted", final)
        self.assertIn('"${current_comment_id}" != "${COMMENT_ID}"', final)
        self.assertIn('"${current_comment_updated_at}" != "${COMMENT_UPDATED_AT}"', final)

    def test_final_comment_revalidation_uses_current_comment_only_for_created_or_edited(self) -> None:
        for action in ("created", "edited", "deleted"):
            with self.subTest(action=action):
                result, output, log = self.run_final_revalidation(action)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("issue_freshness_exit=0", output)
                self.assertIn("repos/owner/repository/issues/64 --jq .updated_at", log)
                if action == "deleted":
                    self.assertNotIn("issues/comments/3", log)
                else:
                    self.assertEqual(log.count("issues/comments/3"), 3)

    def test_schedule_is_only_a_default_branch_reconciliation_path(self) -> None:
        resolver = self.resolver()
        self.assertIn("schedule:", self.workflow)
        self.assertIn("def scheduled_targets", resolver)
        self.assertIn('elif event_name == "schedule"', resolver)
        self.assertIn("Scheduled open pull request response contains a duplicate", resolver)
        self.assertIn("MAX_MATRIX_TARGETS = 256", resolver)

    def test_resolver_accepts_nullable_unrelated_body_without_losing_matching_prs(self) -> None:
        from scripts.review.pr_governance_event_test import PrGovernanceReviewEventTest

        harness = PrGovernanceReviewEventTest(methodName="run_resolver")
        harness.setUp()
        result, outputs = harness.run_resolver(
            "issues",
            [[
                {"number": 1, "state": "open", "draft": False, "body": None},
                {"number": 2, "state": "open", "draft": False, "body": "Fixes #64"},
            ]],
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            [item["pr_number"] for item in __import__("json").loads(outputs["matrix"])["include"]],
            ["2"],
        )

    def test_bind_accepts_requested_ci_generation_and_issue_source(self) -> None:
        match = re.search(
            r"- name: Bind resolved target.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        with tempfile.TemporaryDirectory() as temporary_directory:
            output = Path(temporary_directory) / "output"
            environment = os.environ | {
                "PR_NUMBER": "72", "SOURCE_RUN_ID": "900", "SOURCE_KIND": "ci",
                "SOURCE_CONCLUSION": "pending", "SOURCE_RUN_ATTEMPT": "2",
                "SOURCE_RUN_NUMBER": "8", "SOURCE_WORKFLOW_ID": "44",
                "SOURCE_HEAD_SHA": "a" * 40, "ISSUE_GENERATION_RUN_ID": "901",
                "ISSUE_NUMBER": "64", "ISSUE_UPDATED_AT": "2026-08-29T00:00:00Z",
                "ISSUE_ACTION": "created", "ISSUE_SOURCE": "issue_comment",
                "COMMENT_ID": "3", "COMMENT_CREATED_AT": "2026-08-29T00:00:00Z",
                "COMMENT_UPDATED_AT": "2026-08-29T00:00:00Z", "GITHUB_OUTPUT": str(output),
            }
            result = subprocess.run(
                [sys.executable, "-c", dedent(match.group(1))],
                capture_output=True, text=True, env=environment, check=False,
            )
            self.assertEqual(result.returncode, 1, result.stderr)
            # A source and Issue generation must remain mutually exclusive.
            for name in (
                "SOURCE_RUN_ID", "SOURCE_KIND", "SOURCE_CONCLUSION",
                "SOURCE_RUN_ATTEMPT", "SOURCE_RUN_NUMBER", "SOURCE_WORKFLOW_ID",
                "SOURCE_HEAD_SHA",
            ):
                environment[name] = ""
            result = subprocess.run(
                [sys.executable, "-c", dedent(match.group(1))],
                capture_output=True, text=True, env=environment, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            environment.update(
                {
                    "SOURCE_RUN_ID": "900", "SOURCE_KIND": "ci",
                    "SOURCE_CONCLUSION": "pending", "SOURCE_RUN_ATTEMPT": "2",
                    "SOURCE_RUN_NUMBER": "8", "SOURCE_WORKFLOW_ID": "44",
                    "SOURCE_HEAD_SHA": "a" * 40, "SOURCE_BASE_SHA": "b" * 40,
                    "ISSUE_GENERATION_RUN_ID": "",
                    "ISSUE_NUMBER": "", "ISSUE_UPDATED_AT": "", "ISSUE_ACTION": "",
                    "ISSUE_SOURCE": "", "COMMENT_ID": "", "COMMENT_CREATED_AT": "",
                    "COMMENT_UPDATED_AT": "",
                }
            )
            result = subprocess.run(
                [sys.executable, "-c", dedent(match.group(1))],
                capture_output=True, text=True, env=environment, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
