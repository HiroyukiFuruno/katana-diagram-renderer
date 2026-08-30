from __future__ import annotations

import json
import os
import re
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]


class GovernanceDispatcherContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")

    @staticmethod
    def _workflow_program(match: re.Match[str]) -> str:
        """Normalize extracted YAML Python and neutralize its polling delay for tests."""
        return textwrap.dedent(match.group(1)).replace("time.sleep(2)", "None")

    def test_all_issue_mutations_and_schedule_reconcile_every_open_pr(self) -> None:
        actions = (
            "opened", "edited", "deleted", "transferred", "pinned", "unpinned", "closed", "reopened",
            "assigned", "unassigned", "labeled", "unlabeled", "locked", "unlocked", "milestoned",
            "demilestoned", "typed", "untyped", "field_added", "field_removed",
        )
        for action in actions:
            self.assertIn(action, self.workflow)
        self.assertIn("schedule:", self.workflow)
        self.assertIn("single arbiter removes the former 256-target matrix/rotation", self.workflow)
        self.assertIn('"--paginate", "--slurp", f"repos/{repository}/pulls?state=open&per_page=100"', self.workflow)

    def test_one_event_runs_one_arbiter_after_synchronous_invalidation(self) -> None:
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertNotIn("group: pr-governance-status-${{ github.repository_id }}", self.workflow)
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)
        self.assertNotIn("Legacy dispatcher early invalidator", self.workflow)
        self.assertIn("Invalidate every current pull request for the all-open writer", self.workflow)
        self.assertIn("status=in_progress", self.workflow)
        self.assertEqual(self.workflow.count("actions/workflows/pr-governance-status-writer.yml/dispatches"), 2)
        self.assertIn("permission-actions: write", self.workflow)
        self.assertIn("permission-checks: write", self.workflow)
        self.assertIn("KRR_GOVERNANCE_APP_BOT_LOGIN", writer)
        self.assertIn("github.triggering_actor == vars.KRR_GOVERNANCE_APP_BOT_LOGIN", writer)

    def test_priority_event_preempts_a_long_reconciliation_and_writer_rebinds_before_secrets(self) -> None:
        self.assertIn(
            "cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        self.assertIn("PR source を持つ review/ready", self.workflow)
        self.assertIn("Check Run fingerprint fence", self.workflow)
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)
        rebind = writer.index("Rebind trusted default branch before token creation")
        check_write_token = writer.index("Create Check Run writer App token")
        self.assertLess(rebind, check_write_token)
        self.assertIn("Trusted default branch advanced while writer was queued.", writer)
        self.assertIn("str(posted)!=app_id", self.workflow)

    def test_workflow_run_source_is_strict_before_app_tokens_exist(self) -> None:
        validation = self.workflow[:self.workflow.index("- name: Create dispatcher App token")]
        for text in (
            '"PR governance review sensor"', '"CI"', '"release-preflight"',
            '".github/workflows/test-and-build.yml"', '".github/workflows/release-preflight.yml"',
            'run.get("path")', 'run.get("run_attempt")', 'len(pulls) == 1',
            'workflow_run workflow differs from its trusted base.',
        ):
            self.assertIn(text, validation)

    def test_workflow_run_accepts_github_at_ref_path_and_rejects_prefix_or_traversal(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        for path, expected in ((".github/workflows/test-and-build.yml@main", 0), (".github/workflows/test-and-build.yml@refs/pull/72/merge", 0), (".github/workflows/test-and-build.yml.evil@main", 1), (".github/workflows/test-and-build.yml@../main", 1)):
            with self.subTest(path=path), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                run = {"name": "CI", "path": path, "event": "pull_request", "status": "completed", "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}]}
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' '[]' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_requested_and_waiting_workflow_run_statuses_reach_invalidation_path(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        for status in ("requested", "waiting"):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                run = {"name": "CI", "path": ".github/workflows/test-and-build.yml@main", "event": "pull_request", "status": status, "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}]}
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' '[]' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_unrelated_issue_skips_post_and_dispatch_but_referenced_issue_selects_all_closers(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Closes https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        for issue, expected in (
            ("999", {"reconcile": "false", "event_targets": "[]", "priority_targets": "[]"}),
            ("64", {"reconcile": "true", "event_targets": "[72,73]", "priority_targets": "[]"}),
        ):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "issues", "ISSUE_NUMBER": issue, "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("if: steps.current-targets.outputs.has_targets == 'true'", self.workflow)

    def test_dispatcher_accepts_optional_colon_in_closing_references(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes: #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Resolves: https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8")
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(values["event_targets"], "[72,73]")

    def test_malformed_workflow_source_expands_every_derivable_issue_closure(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        run = {"name": "CI", "path": ".github/workflows/test-and-build.yml@main", "event": "pull_request", "status": "requested", "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}]}
        local = {"base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        pulls = [[{"number": 72, "state": "open", "body": "Fixes #64; closes #65", **local}, {"number": 73, "state": "open", "body": "Fixes #64", **local}, {"number": 74, "state": "open", "body": "Fixes #65", **local}]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\ncase \"$*\" in\n  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n  *) exit 91 ;;\nesac\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("reconcile=true", output.read_text(encoding="utf-8"))

    def test_pull_request_target_revalidates_old_and_new_closures_but_skips_forks(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        local = {"base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", **local},
            {"number": 73, "state": "open", "body": "Fixes #65", **local},
        ]]
        current = {"number": 73, "state": "open", "base": {"sha": base, **local["base"]}, "head": {"sha": head, **local["head"]}}
        for source, expected in (
            (current, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[]"}),
            ({**current, "head": {"sha": head, "repo": {"full_name": "fork/repository"}}}, {"reconcile": "false", "event_targets": "[]", "priority_targets": "[]"}),
        ):
            with self.subTest(source=source["head"]["repo"]["full_name"]), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n  *'/pulls/73'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": "edited", "PR_NUMBER": "73", "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": base, "PR_BODY": "Fixes #65", "PR_PREVIOUS_BODY": "Fixes #64", "GITHUB_REPOSITORY": "owner/repository",
                    "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "SOURCE": json.dumps(source),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)

    def test_104_related_prs_keep_the_early_sensor_path_to_the_source_only(self) -> None:
        resolver = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        current = re.search(r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(resolver); self.assertIsNotNone(current)
        assert resolver is not None and current is not None
        base, head = "b" * 40, "a" * 40
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {
                "number": number,
                "state": "open",
                "body": "Fixes #64" if number in {72, 73} else "Fixes #99",
                "draft": False,
                "base": local_base,
                "head": {"sha": f"{number:040x}", "repo": {"full_name": "owner/repository"}},
            }
            for number in range(1, 106)
        ]]
        source = {"number": 72, "state": "open", "base": {"sha": base, **local_base}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "resolve-output"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "EVENT_NAME": "pull_request_target", "PR_ACTION": "ready_for_review", "PR_NUMBER": "72", "PR_HEAD_SHA": head,
                "PR_BASE_SHA": base, "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "Fixes #64", "GITHUB_REPOSITORY": "owner/repository",
                "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "SOURCE": json.dumps(source),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(resolver)], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            resolved = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(resolved["priority_targets"], "[72]")
            self.assertEqual(json.loads(resolved["event_targets"]), [72, 73])

            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            current_output = directory / "current-output"
            current_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(current_output), "PULLS": json.dumps(pulls),
                "EVENT_TARGETS": resolved["event_targets"], "EVENT_PRIORITY_TARGETS": resolved["priority_targets"],
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", self._workflow_program(current)], env=current_environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            selected = dict(line.split("=", 1) for line in current_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(json.loads(selected["priority_targets"]), [72])
            self.assertEqual(selected["event_targets"], "[72,73]")
            self.assertEqual(json.loads(selected["event_targets"]), [72, 73])
            invalidations = json.loads(selected["all_invalidation_targets"])
            self.assertEqual(invalidations[0], 73)
            self.assertLess(invalidations.index(73), invalidations.index(1))
            self.assertEqual(len(json.loads(selected["targets"])), 105)
        early = self.workflow.index("Dispatch and bind the early event writer")
        full = self.workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertNotIn("AFFECTED: ${{ steps.current-targets.outputs.priority_targets }}", self.workflow[early:full])
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_targets }}", self.workflow[full:])

    def test_priority_duplicate_head_skips_early_writer_and_invalidates_every_known_head(self) -> None:
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        invalidator = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); self.assertIsNotNone(invalidator)
        assert current is not None and invalidator is not None
        duplicate_head, unique_head = "a" * 40, "b" * 40
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": duplicate_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": duplicate_head.upper(), "repo": {"full_name": "owner/repository"}}},
            {"number": 74, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": unique_head, "repo": {"full_name": "owner/repository"}}},
        ]]
        response = {
            "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": duplicate_head, "external_id": f"krr-governance/v1/{duplicate_head}/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; selection_output = directory / "selection"; posts = directory / "posts"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            selection_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(selection_output),
                "EVENT_TARGETS": "[72,73]", "EVENT_PRIORITY_TARGETS": "[72]", "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            selected = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current)],
                env=selection_environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(selected.returncode, 0, selected.stderr)
            selection = dict(line.split("=", 1) for line in selection_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(selection["priority_targets"], "[]")
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [72, 73, 74])

            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'|*'/pulls/73'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{duplicate_head}\"}}}}' ;;\n"
                f"  *'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unique_head}\"}}}}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{posts}'; printf '%s' '{json.dumps(response)}' ;;\n"
                "  *'/dispatches'*) exit 92 ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            invalidator_program = self._workflow_program(invalidator).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
            invalidation_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42",
                "AFFECTED": selection["all_invalidation_targets"],
                "KNOWN_TARGET_SNAPSHOTS": selection["all_invalidation_target_snapshots"],
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            invalidated = subprocess.run(
                [sys.executable, "-c", invalidator_program],
                env=invalidation_environment, capture_output=True, text=True, check=False,
            )
            self.assertNotEqual(invalidated.returncode, 0)
            writes = posts.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 2)
            self.assertIn(f"head_sha={duplicate_head}", writes[0])
            self.assertTrue(any(f"head_sha={unique_head}" in write for write in writes))
        early = self.workflow.index("Dispatch and bind the early event writer")
        await_early = self.workflow.index("Await the bound early event writer before all-open invalidation")
        self.assertIn("if: steps.current-targets.outputs.has_priority_targets == 'true'", self.workflow[early:await_early])

    def test_unrelated_duplicate_head_also_suppresses_the_priority_writer(self) -> None:
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); assert current is not None
        source_head, duplicate_head = "c" * 40, "a" * 40
        base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": base, "head": {"sha": source_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": base, "head": {"sha": duplicate_head, "repo": {"full_name": "owner/repository"}}},
            {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": base, "head": {"sha": duplicate_head.upper(), "repo": {"full_name": "owner/repository"}}},
        ]]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(output),
                "EVENT_TARGETS": "[72]", "EVENT_PRIORITY_TARGETS": "[72]", "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current)],
                env=environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            selection = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(selection["priority_targets"], "[]")
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [72, 73, 74])

    def test_pull_request_target_rejects_source_head_or_state_race(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        source = {"number": 72, "state": "closed", "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\ncase \"$*\" in\n  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' '[]' ;;\n  *) exit 91 ;;\nesac\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "pull_request_target", "PR_ACTION": "closed", "PR_NUMBER": "72", "PR_HEAD_SHA": head, "PR_BASE_SHA": base, "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "SOURCE": json.dumps({**source, "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}}}), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_dispatcher_rejects_duplicate_foreign_pr_across_pages(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        fork = {"number": 73, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "fork/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps([[fork], [fork]]), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_only_dispatcher_can_issue_synchronous_pending_invalidation(self) -> None:
        self.assertIn("external_id=f\"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}\"", self.workflow)
        self.assertNotIn("/statuses/", self.workflow)

    def test_relevant_event_is_read_only_until_singleton_reconciles_every_current_local_pr(self) -> None:
        resolver = self.workflow[:self.workflow.index("  reconcile-all-open:")]
        self.assertNotIn("concurrency:", resolver)
        self.assertIn("reconcile: ${{ steps.targets.outputs.reconcile }}", resolver)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("group: pr-governance-dispatcher-${{ github.repository_id }}", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_targets }}", self.workflow)
        reconcile_start = self.workflow.index("  reconcile-all-open:")
        reconcile_job = self.workflow[reconcile_start:self.workflow.index("    concurrency:", reconcile_start)]
        self.assertIn(
            "if: needs.resolve_event.outputs.reconcile == 'true' && github.run_attempt == 1",
            reconcile_job,
        )
        self.assertLess(
            reconcile_job.index("github.run_attempt == 1"),
            reconcile_job.index("environment: pr-governance"),
        )
        match = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        pulls = [[
            {"number": 64, "state": "open", "body": "Fixes #1", "draft": False, "base": local_base, "head": {"sha": "d" * 40, "repo": {"full_name": "owner/repository"}}},
            {"number": 65, "state": "open", "body": "Fixes #2", "draft": False, "base": local_base, "head": {"sha": "e" * 40, "repo": {"full_name": "owner/repository"}}},
            {"number": 66, "state": "open", "body": "Fixes #3", "draft": False, "base": local_base, "head": {"repo": {"full_name": "fork/repository"}}},
        ]]
        for pages, expected in ((pulls, 0), ([[pulls[0][0]], [pulls[0][0]]], 1)):
            with self.subTest(duplicate=expected == 1), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_OUTPUT": str(output),
                    "EVENT_TARGETS": "[64,65]", "EVENT_PRIORITY_TARGETS": "[64]", "PULLS": json.dumps(pages),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(
                        dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines()),
                        {
                            "has_targets": "true", "targets": "[64, 65]",
                            "event_targets": "[64,65]", "has_event_targets": "true",
                            "has_priority_targets": "true", "priority_targets": "[64]",
                            "has_all_invalidation_targets": "true", "all_invalidation_targets": "[65]",
                            "all_invalidation_target_snapshots": "[[65,\"" + "e" * 40 + "\",false]]",
                            "writer_head": "a" * 40, "default_branch": "master",
                        },
                    )

    def test_priority_event_preempts_the_current_reconciler_and_preserves_affected_order(self) -> None:
        # sourceを持つeventだけが全件走査を中断する。通常の全件走査は
        # current snapshotを取り直すが、event由来のcloser集合はwriterへ順序を渡す。
        self.assertIn("needs: resolve_event", self.workflow)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("Re-enumerate every current local governance pull request", self.workflow)
        self.assertNotIn("steps.targets.outputs.affected", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_targets }}", self.workflow)
        self.assertIn("cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}", self.workflow)
        self.assertIn("WRITER_TARGETS: ${{ steps.current-targets.outputs.event_targets }}", self.workflow)

    def test_writer_drain_precedes_pending_invalidation_and_preserves_token_boundaries(self) -> None:
        drain = self.workflow.index("Drain authoritative writer before normal all-open invalidation")
        invalidate = self.workflow.index("Invalidate every current pull request for the all-open writer")
        dispatch = self.workflow.index("Dispatch one repository-wide governance arbiter")
        self.assertLess(drain, invalidate)
        self.assertLess(invalidate, dispatch)
        section = self.workflow[drain:self.workflow.index("Create invalidator read-only App token", drain)]
        self.assertIn("GH_TOKEN: ${{ steps.dispatcher-token.outputs.token }}", section)
        self.assertNotIn("CHECK_WRITE_TOKEN", section)
        self.assertIn('"--paginate", "--slurp", f"repos/{repository}/actions/workflows/{workflow_id}/runs?per_page=100"', section)
        self.assertIn('f"repos/{repository}/actions/runs/{identifier}/cancel"', section)
        self.assertIn('active = {"requested", "queued", "pending", "waiting", "in_progress"}', section)
        self.assertIn("for _ in range(150):", section)
        self.assertIn('run.get("status") != "completed"', section)
        self.assertIn("Governance writer run identity is invalid.", section)

    def test_event_writer_is_terminal_before_full_snapshot_invalidation(self) -> None:
        dispatch = self.workflow.index("Dispatch and bind the early event writer")
        await_early = self.workflow.index("Await the bound early event writer before all-open invalidation")
        all_open = self.workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertLess(dispatch, await_early)
        self.assertLess(await_early, all_open)
        wait_section = self.workflow[await_early:all_open]
        for value in (
            "DEFAULT_BRANCH: ${{ steps.current-targets.outputs.default_branch }}",
            "WRITER_HEAD: ${{ steps.current-targets.outputs.writer_head }}",
            "DISPATCHER_RUN_ID: ${{ github.run_id }}",
            "CHECK_READ_TOKEN: ${{ steps.invalidator-read-token.outputs.token }}",
            'run.get("display_title")!=title', 'run.get("head_sha")!=head',
            'run.get("status")=="completed"', 'run.get("conclusion")!="success"',
        ):
            self.assertIn(value, wait_section)
        self.assertNotIn("CHECK_READ_TOKEN: ${{ steps.invalidator-token.outputs.token }}", wait_section)
        self.assertIn('env={"GH_TOKEN":check_token,"PATH":os.environ["PATH"]}', wait_section)
        read_token = self.workflow[self.workflow.index("Create invalidator read-only App token"):self.workflow.index("Dispatch and bind the early event writer")]
        self.assertIn("permission-checks: read", read_token)

    def test_early_dispatch_binds_exact_new_writer_or_fails_closed(self) -> None:
        match = re.search(
            r"- name: Dispatch and bind the early event writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace("time.sleep(2)", "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "queued", "run_number": 1, "run_attempt": 1,
        }
        for mode, expected in (("exact", 0), ("ambiguous", 1), ("bad-path", 1), ("bad-attempt", 1), ("timeout", 1)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; state = directory / "state"; output = directory / "output"
                fake.write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, sys\n"
                    "arguments = ' '.join(sys.argv[1:]); state = os.environ['STATE']\n"
                    "count = int(open(state).read()) if os.path.exists(state) else 0\n"
                    "if '/runs?per_page=100' in arguments:\n"
                    "    open(state, 'w').write(str(count + 1))\n"
                    "    if count < 2 or os.environ['MODE'] == 'timeout': print(json.dumps([{'workflow_runs': []}]))\n"
                    "    else:\n"
                    "        run = json.loads(os.environ['RUN'])\n"
                    "        if os.environ['MODE'] == 'bad-path': run['path'] = '.github/workflows/other.yml@master'\n"
                    "        if os.environ['MODE'] == 'bad-attempt': run['run_attempt'] = True\n"
                    "        runs = [run] if os.environ['MODE'] != 'ambiguous' else [run, dict(run, id=72)]\n"
                    "        print(json.dumps([{'workflow_runs': runs}]))\n"
                    "elif '/dispatches' in arguments:\n"
                    "    if 'inputs[scope]=early' not in arguments or 'inputs[target_numbers]=[72,73]' not in arguments or 'inputs[preserved_target_numbers]=[]' not in arguments or 'inputs[preserved_writer_run_id]=0' not in arguments: raise SystemExit(92)\n"
                    "else: raise SystemExit(91)\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                program = base_program.replace("range(150)", "range(2)") if mode == "timeout" else base_program
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "DISPATCHER_RUN_ID": "99", "TARGETS": "[72,73]", "MODE": mode, "STATE": str(state),
                    "RUN": json.dumps(valid), "GITHUB_OUTPUT": str(output), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(output.read_text(encoding="utf-8"), "writer_run_id=71\n")

    def test_early_writer_wait_rejects_identity_drift_and_non_success_terminal(self) -> None:
        match = re.search(
            r"- name: Await the bound early event writer before all-open invalidation.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace("time.sleep(2)", "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "completed", "conclusion": "success", "run_number": 1, "run_attempt": 1,
        }
        for mutate, expected in ((lambda run: None, 0), (lambda run: run.update(conclusion="failure"), 1), (lambda run: run.update(head_sha="b" * 40), 1), (lambda run: run.update(run_attempt=True), 1)):
            with self.subTest(expected=expected), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"
                run = dict(valid); mutate(run)
                check = {
                    "id": 701, "name": "KRR / PR governance (trusted check)",
                    "head_sha": "a" * 40,
                    "external_id": "krr-governance/v1/" + "a" * 40 + "/writer-71",
                    "app": {"id": 42}, "status": "completed", "conclusion": "success",
                    "details_url": "https://github.com/owner/repository/actions/runs/71?source_run_id=99",
                }
                token_log = directory / "check-read-token"
                fake.write_text(
                    "#!/bin/sh\ncase \"${GH_TOKEN}:$*\" in\n"
                    f"  checks-read:*) printf '%s' \"${{GH_TOKEN}}\" > '{token_log}'; printf '%s' '{json.dumps([{'check_runs': [check]}])}' ;;\n"
                    "  *) printf '%s' \"${RUN}\" ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "WRITER_RUN_ID": "71", "DEFAULT_BRANCH": "master",
                    "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "99", "RUN": json.dumps(run),
                    "CHECK_APP_ID": "42",
                    "CHECK_READ_TOKEN": "checks-read", "GH_TOKEN": "actions-write", "TARGETS": "[72]",
                    "GITHUB_SERVER_URL": "https://github.com", "GITHUB_OUTPUT": str(directory / "output"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", base_program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(token_log.read_text(encoding="utf-8"), "checks-read")

    def test_invalidator_preempts_priority_dispatchers_and_paces_every_check_write(self) -> None:
        dispatcher_group = "group: pr-governance-dispatcher-${{ github.repository_id }}"
        self.assertEqual(self.workflow.count(dispatcher_group), 1)
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        self.assertIn("write_clock=[time.monotonic()+8.1]", program)
        self.assertIn("time.sleep(delay)", program)
        self.assertIn("delay=write_clock[0]-time.monotonic()", program)
        self.assertIn("write_clock[0]=time.monotonic()+8.1", program)
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("cancel-in-progress: ${{ inputs.scope == 'early' }}", writer)

    def test_invalidator_reopens_terminal_trusted_checks_but_marks_carry_only_for_pending_dispatcher_state(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        self.assertIn('external_id=f"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}"', program)
        self.assertIn('command=["gh","api","--method","POST"', program)
        self.assertNotIn('"--method","PATCH"', program)
        self.assertIn('type(draft) is not bool', program)
        self.assertIn('"carry_pending":str(carry_pending)', program)

    def test_invalidator_pendingizes_a_draft_before_failing_closed_on_terminal_carry(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        head = "a" * 40
        prior = {
            "id": 101, "created_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": head, "external_id": "krr-governance/v1/" + head + "/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=1",
        }
        current = {**prior, "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0"}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; log = directory / "patch.log"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'--method POST'*) echo \"$*\" >> '{log}'; printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs?'*) printf '%s' '{json.dumps([{'check_runs': [prior]}])}' ;;\n"
                "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": "[72]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, True]]),
                "PULL": json.dumps({"draft": True, "head": {"sha": head}}),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(len(log.read_text(encoding="utf-8").splitlines()), 1)

    def test_invalidator_carries_pending_tail_across_104_to_600_open_prs(self) -> None:
        """Every later all-open generation inherits a valid pending tail, not just its first page."""
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        for total in (104, 300, 451, 600):
            with self.subTest(total=total), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"; posted: dict[int, dict[str, object]] = {}; writes: list[list[str]] = []
                def response(value: object) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], 0, json.dumps(value), "")
                def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                    endpoint = arguments[-1]
                    if isinstance(endpoint, str) and "/pulls/" in endpoint:
                        number = int(endpoint.rsplit("/", 1)[1]); return response({"draft": False, "head": {"sha": f"{number:040x}"}})
                    if isinstance(endpoint, str) and "check-runs?" in endpoint:
                        head = endpoint.split("/commits/", 1)[1].split("/", 1)[0]
                        return response([{"check_runs": [{"id": 500_000 + int(head, 16), "created_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-8", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/8?dispatcher_run_id=8&carry_pending=1"}]}])
                    if "--method" in arguments and "POST" in arguments:
                        fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                        identifier = 1_000_000 + int(fields["head_sha"], 16)
                        value = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                        posted[identifier] = value; writes.append(arguments)
                        self.assertEqual(kwargs["env"], {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                        return response(value)
                    if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                        return response(posted[int(endpoint.rsplit("/", 1)[1])])
                    raise AssertionError(arguments)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": json.dumps(list(range(1, total + 1))),
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, f"{number:040x}", False] for number in range(1, total + 1)]),
                    "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"],
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    namespace: dict[str, object] = {"__name__": "__main__"}
                    exec(program, namespace)
                self.assertEqual(len(writes), total)
                self.assertTrue(all("details_url=https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=1" in write for write in writes))
                manifest = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())["check_manifest"]
                self.assertEqual(len(json.loads(manifest)), total)

    def test_compact_check_manifest_stays_within_the_workflow_dispatch_payload_limit(self) -> None:
        # `[pr,check_run_id]` keeps a 600-PR all-open dispatch well below the
        # 65,535-byte GitHub workflow_dispatch input ceiling.
        manifest = json.dumps([[number, 9_000_000_000_000_000_000 + number] for number in range(1, 601)], separators=(",", ":"))
        self.assertLess(len(manifest.encode("utf-8")), 65_535)
        self.assertIn('inputs[check_manifest]={raw_manifest}', self.workflow)

    def test_invalidator_replaces_duplicate_heads_and_pendingizes_unique_known_heads(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        head, unique_head = "a" * 40, "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; post = directory / "post"
            response = {
                "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-9",
                "status": "in_progress", "conclusion": None,
                "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
            }
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'|*'/pulls/73'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                f"  *'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unique_head}\"}}}}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{post}'; printf '%s' '{json.dumps(response)}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": "[72,73,74]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, False], [73, head, False], [74, unique_head, False]]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            writes = post.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 2)
            self.assertIn(f"head_sha={head}", writes[0])
            self.assertIn(f"external_id=krr-governance/v1/{head}/dispatcher-9", writes[0])
            self.assertIn("status=in_progress", writes[0])
            self.assertTrue(any(f"head_sha={unique_head}" in write for write in writes))

    def test_event_source_and_duplicate_heads_are_invalidated_after_an_earlier_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        source_head, duplicate_head, unrelated_head = "c" * 40, "a" * 40, "b" * 40
        response = {
            "id": 102, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": duplicate_head, "external_id": f"krr-governance/v1/{duplicate_head}/dispatcher-9",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; log = directory / "posts"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'/pulls/72'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{source_head}\"}}}}' ;;\n"
                f"  *'/pulls/73'|*'/pulls/74'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{duplicate_head}\"}}}}' ;;\n"
                f"  *'/pulls/75'*) printf '%s' '{{\"draft\":false,\"head\":{{\"sha\":\"{unrelated_head}\"}}}}' ;;\n"
                f"  *'check-runs/102'*) printf '%s' '{json.dumps(response)}' ;;\n"
                f"  *'--method POST'*) echo \"$*\" >> '{log}'; case \"$*\" in *'head_sha={source_head}'*) exit 7 ;; *) printf '%s' '{json.dumps(response)}' ;; esac ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": "[72,73,74,75]", "EVENT_TARGETS": "[72]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, source_head, False], [73, duplicate_head, False], [74, duplicate_head, False], [75, unrelated_head, False]]),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)
            writes = log.read_text(encoding="utf-8").splitlines()
            self.assertEqual(len(writes), 3)
            self.assertEqual(sum(f"head_sha={source_head}" in write for write in writes), 1)
            self.assertEqual(sum(f"head_sha={duplicate_head}" in write for write in writes), 1)
            self.assertEqual(sum(f"head_sha={unrelated_head}" in write for write in writes), 1)

    def test_invalidator_pendingizes_all_known_heads_before_a_refresh_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        first_head, second_head = "a" * 40, "b" * 40
        calls: list[list[str]] = []; posted: dict[int, dict[str, object]] = {}

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            calls.append(arguments)
            endpoint = arguments[-1]
            if isinstance(endpoint, str) and "check-runs?" in endpoint:
                return response([])
            if "--method" in arguments and "POST" in arguments:
                fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                identifier = 100 + len(posted)
                check = {
                    "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                    "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                    "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                }
                posted[identifier] = check
                return response(check)
            if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                return response(posted[int(endpoint.rsplit("/", 1)[1])])
            if isinstance(endpoint, str) and endpoint.endswith("/pulls/72"):
                return response({}, 7)
            if isinstance(endpoint, str) and endpoint.endswith("/pulls/73"):
                return response({"draft": False, "head": {"sha": second_head}})
            raise AssertionError(arguments)

        environment = os.environ | {
            "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
            "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": "[72,73]",
            "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, first_head, False], [73, second_head, False]]),
            "GITHUB_OUTPUT": str(Path(tempfile.gettempdir()) / "krr-invalidator-output"), "PATH": os.environ["PATH"],
        }
        with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
            with self.assertRaises(SystemExit):
                exec(program, {"__name__": "__main__"})
        post_indexes = [index for index, arguments in enumerate(calls) if "--method" in arguments and "POST" in arguments]
        refresh_indexes = [index for index, arguments in enumerate(calls) if arguments[-1] in {"repos/owner/repository/pulls/72", "repos/owner/repository/pulls/73"}]
        self.assertEqual(len(post_indexes), 2)
        self.assertEqual(len(refresh_indexes), 2)
        self.assertLess(max(post_indexes), min(refresh_indexes))
        self.assertEqual({check["head_sha"] for check in posted.values()}, {first_head, second_head})

    def test_writer_drain_handles_historical_and_completed_races_but_fails_closed_otherwise(self) -> None:
        match = re.search(
            r"- name: Drain authoritative writer before normal all-open invalidation.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(2)", "None")
        head = "a" * 40
        valid = {
            "id": 7, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": head,
            "workflow_id": 44, "repository": {"full_name": "owner/repository"},
            "run_number": 1, "run_attempt": 1, "status": "in_progress",
        }
        for mode, expected in (("cancel-failure", 1), ("timeout", 1), ("bad-identity", 1), ("old-head", 0), ("already-completed", 0)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; marker = directory / "cancelled"
                bad = {**valid, "name": "unexpected writer"}
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/7/cancel'*) case \"${MODE}\" in cancel-failure|already-completed) exit 7 ;; esac; touch \"${MARKER}\" ;;\n"
                    "  *'/actions/runs/7'*) printf '%s' \"${POLL}\" ;;\n"
                    "  *'actions/workflows/44/runs'*) printf '%s' \"${RUNS}\" ;;\n"
                    "  *'actions/workflows/pr-governance-status-writer.yml'*) printf '%s' '{\"id\":44}' ;;\n"
                    f"  *'git/ref/heads/master'*) printf '%s' '{{\"object\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                listed_run = {**valid, "head_sha": "b" * 40} if mode == "old-head" else valid
                poll = valid if mode in {"timeout", "cancel-failure"} else bad if mode == "bad-identity" else {**listed_run, "status": "completed"}
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GH_TOKEN": "actions-write", "MODE": mode,
                    "MARKER": str(marker), "RUNS": json.dumps([{"workflow_runs": [listed_run]}]),
                    "POLL": json.dumps(poll), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_dispatch_waits_for_exact_new_writer_registration_and_rejects_gap_or_bad_identity(self) -> None:
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("run-name: source=${{ inputs.dispatcher_run_id }} scope=${{ inputs.scope }}", writer)
        match = re.search(
            r"- name: Dispatch one repository-wide governance arbiter.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace('subprocess.run(["sleep", "2"], check=False)', "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=all",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "queued", "run_number": 1, "run_attempt": 1,
        }
        for mode, expected in (("gap", 0), ("bad", 1), ("bad-attempt", 1), ("timeout", 1)):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; state = directory / "state"
                fake.write_text(
                    "#!/usr/bin/env python3\n"
                    "import json, os, sys\n"
                    "arguments = ' '.join(sys.argv[1:])\n"
                    "state = os.environ['STATE']\n"
                    "count = int(open(state).read()) if os.path.exists(state) else 0\n"
                    "if '/runs?per_page=100' in arguments:\n"
                    "    open(state, 'w').write(str(count + 1))\n"
                    "    if count < 2 or os.environ['MODE'] == 'timeout':\n"
                    "        print(json.dumps([{'workflow_runs': []}]))\n"
                    "    else:\n"
                    "        run = json.loads(os.environ['RUN'])\n"
                    "        if os.environ['MODE'] == 'bad': run['head_sha'] = 'b' * 40\n"
                    "        if os.environ['MODE'] == 'bad-attempt': run['run_attempt'] = True\n"
                    "        unrelated = dict(run, id=70, display_title='source=other scope=all')\n"
                    "        print(json.dumps([{'workflow_runs': [unrelated, run]}]))\n"
                    "elif '/dispatches' in arguments:\n"
                    "    if 'inputs[target_numbers]=[72,73]' not in arguments or 'inputs[preserved_target_numbers]=[]' not in arguments or 'inputs[preserved_writer_run_id]=0' not in arguments: raise SystemExit(92)\n"
                    "else:\n"
                    "    raise SystemExit(91)\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                program = base_program.replace("range(150)", "range(2)") if mode == "timeout" else base_program
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "DISPATCHER_RUN_ID": "99", "WRITER_SCOPE": "all", "WRITER_TARGETS": "[72,73]",
                    "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0", "MODE": mode, "STATE": str(state), "RUN": json.dumps(valid),
                    "WRITER_TAIL_CHECK_MANIFEST": "[[72,701],[73,702]]",
                    "WRITER_PRESERVED_CHECK_MANIFEST": "[]",
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_invalidator_rejects_wrong_or_malformed_check_app_before_dispatch(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None")
        base = {
            "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": "a" * 40, "external_id": "krr-governance/v1/" + "a" * 40,
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0",
        }
        for response in ({**base, "app": {"id": 7}}, {**base, "id": "101"}, {**base, "app": {"id": True}}):
            with self.subTest(response=response), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'check-runs?'*) printf '%s' '[]' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'--method POST'*) printf '%s' \"${POST}\" ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GITHUB_OUTPUT": str(directory / "output"),
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "PULLS": json.dumps([[{"number": 72, "state": "open"}]]),
                    "AFFECTED": "[72]",
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, "a" * 40, False]]),
                    "PULL": json.dumps({"draft": False, "head": {"sha": "a" * 40}}), "POST": json.dumps(response),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertNotEqual(result.returncode, 0)

    def test_invalidator_has_no_all_open_cap_and_continues_after_a_post_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")
        self.assertNotIn("numbers[:", program)
        self.assertNotIn("len(numbers) >", program)
        # The large production-path regression lives above; this fixture
        # retains the single-write failure boundary without inventing a
        # duplicate head SHA that production now rejects before mutation.
        for total, failed, expected in ((1, "", 0), (1, "1", 1)):
            with self.subTest(total=total, failed=failed), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); log = directory / "post.log"; fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'check-runs?'*) printf '%s' '[]' ;;\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'/pulls/'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'--method POST'*)\n"
                    f"    echo \"$*\" >> '{log}'\n"
                    f"    count=$(awk 'END {{ print NR }}' '{log}')\n"
                    f"    if [ '{failed}' = \"$count\" ]; then exit 7; fi\n"
                    "    printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GITHUB_OUTPUT": str(directory / "output"),
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "PULLS": "[]",
                    "AFFECTED": json.dumps(list(range(1, total + 1))),
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, "a" * 40, False] for number in range(1, total + 1)]),
                    "PULL": json.dumps({"draft": False, "head": {"sha": "a" * 40}}),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if log.exists():
                    self.assertEqual(len(log.read_text(encoding="utf-8").splitlines()), total)

    def test_global_serialized_rate_model_bounds_every_sliding_hour_at_445_writes(self) -> None:
        pace_seconds = 8.1
        writer_first_write_delay = 8.1
        # 直列lock内ではinvalidator完了後にwriterへ遷移する。実際に生成
        # される時刻列から、任意window (t-3600, t] の最大件数を求める。
        for all_open, expected_maximum in ((300, 301), (451, 445), (600, 445)):
            with self.subTest(all_open=all_open):
                invalidator_writes = [(index + 1) * pace_seconds for index in range(all_open)]
                writer_first_write = invalidator_writes[-1] + writer_first_write_delay
                writes = [*invalidator_writes, writer_first_write]
                maximum = max(
                    sum(window_end - 3600 < write <= window_end for write in writes)
                    for window_end in writes
                )
                self.assertEqual(len(invalidator_writes), all_open)
                self.assertTrue(all(
                    later - earlier >= pace_seconds - 1e-9
                    for earlier, later in zip(invalidator_writes, invalidator_writes[1:])
                ))
                self.assertGreaterEqual(writer_first_write - invalidator_writes[-1], writer_first_write_delay - 1e-9)
                self.assertEqual(maximum, expected_maximum)
                self.assertLessEqual(maximum, 445)


if __name__ == "__main__":
    unittest.main()
