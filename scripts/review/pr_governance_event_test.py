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


ROOT = Path(__file__).parents[2]


class GovernanceDispatcherContractTest(unittest.TestCase):
    def setUp(self) -> None:
        self.workflow = (ROOT / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")

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
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: false",
            self.workflow,
        )
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertNotIn("group: pr-governance-status-${{ github.repository_id }}", self.workflow)
        self.assertIn("cancel-in-progress: false", writer)
        self.assertIn("Invalidate affected current pull requests before dispatch", self.workflow)
        self.assertIn("status=in_progress", self.workflow)
        self.assertEqual(self.workflow.count("actions/workflows/pr-governance-status-writer.yml/dispatches"), 1)
        self.assertIn("permission-actions: write", self.workflow)
        self.assertIn("permission-checks: write", self.workflow)

    def test_dispatcher_is_not_queued_behind_writer_and_writer_rebinds_before_secrets(self) -> None:
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: false",
            self.workflow,
        )
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        self.assertIn("group: pr-governance-status-${{ github.repository_id }}", writer)
        self.assertIn("cancel-in-progress: false", writer)
        rebind = writer.index("Rebind trusted default branch before token creation")
        check_write_token = writer.index("Create Check Run writer App token")
        self.assertLess(rebind, check_write_token)
        self.assertIn("Trusted default branch advanced while writer was queued.", writer)
        self.assertIn("str(posted_by) != app_id", self.workflow)

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
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' '[]' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
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
                result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_unrelated_issue_skips_post_and_dispatch_but_referenced_issue_selects_all_closers(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Closes https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        for issue, expected in (("999", {"reconcile": "false"}), ("64", {"reconcile": "true"})):
            with self.subTest(issue=issue), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "issues", "ISSUE_NUMBER": issue, "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("if: steps.current-targets.outputs.has_targets == 'true'", self.workflow)

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
            result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
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
        for source, expected in ((current, {"reconcile": "true"}), ({**current, "head": {"sha": head, "repo": {"full_name": "fork/repository"}}}, {"reconcile": "false"})):
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
                result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)

    def test_pull_request_target_rejects_source_head_or_state_race(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        source = {"number": 72, "state": "closed", "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\ncase \"$*\" in\n  *'/pulls/72'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' '[]' ;;\n  *) exit 91 ;;\nesac\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "pull_request_target", "PR_ACTION": "closed", "PR_NUMBER": "72", "PR_HEAD_SHA": head, "PR_BASE_SHA": base, "PR_BODY": "Fixes #64", "PR_PREVIOUS_BODY": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "SOURCE": json.dumps({**source, "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}}}), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_dispatcher_rejects_duplicate_foreign_pr_across_pages(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        fork = {"number": 73, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "fork/repository"}}}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
            fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {"EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps([[fork], [fork]]), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
            result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
            self.assertNotEqual(result.returncode, 0)

    def test_only_dispatcher_can_issue_synchronous_pending_invalidation(self) -> None:
        self.assertIn("external_id=f\"krr-governance/v1/{head.lower()}\"", self.workflow)
        self.assertNotIn("/statuses/", self.workflow)

    def test_relevant_event_is_read_only_until_singleton_reconciles_every_current_local_pr(self) -> None:
        resolver = self.workflow[:self.workflow.index("  reconcile-all-open:")]
        self.assertNotIn("concurrency:", resolver)
        self.assertIn("reconcile: ${{ steps.targets.outputs.reconcile }}", resolver)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("group: pr-governance-dispatcher-${{ github.repository_id }}", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.targets }}", self.workflow)
        match = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        local = {"base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        pulls = [[
            {"number": 64, "state": "open", "body": "Fixes #1", **local},
            {"number": 65, "state": "open", "body": "Fixes #2", **local},
            {"number": 66, "state": "open", "body": "Fixes #3", "base": local["base"], "head": {"repo": {"full_name": "fork/repository"}}},
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
                    "PULLS": json.dumps(pages), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", match.group(1)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(
                        dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines()),
                        {"has_targets": "true", "targets": "[64, 65]", "writer_head": "a" * 40, "default_branch": "master"},
                    )

    def test_replacement_reconciler_includes_prior_pending_event_targets(self) -> None:
        # D1が実行中、D2が#65をpending化、D3が待機中に置換されても、D3は
        # event由来のaffected集合を使わずcurrent all-openを再取得する。
        self.assertIn("needs: resolve_event", self.workflow)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("Re-enumerate every current local governance pull request", self.workflow)
        self.assertNotIn("steps.targets.outputs.affected", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.targets }}", self.workflow)
        self.assertIn("cancel-in-progress: false", self.workflow)

    def test_writer_drain_precedes_pending_invalidation_and_preserves_token_boundaries(self) -> None:
        drain = self.workflow.index("Drain authoritative writer before invalidation")
        invalidate = self.workflow.index("Invalidate affected current pull requests before dispatch")
        dispatch = self.workflow.index("Dispatch one repository-wide governance arbiter")
        self.assertLess(drain, invalidate)
        self.assertLess(invalidate, dispatch)
        section = self.workflow[drain:invalidate]
        self.assertIn("GH_TOKEN: ${{ steps.dispatcher-token.outputs.token }}", section)
        self.assertNotIn("CHECK_WRITE_TOKEN", section)
        self.assertIn('"--paginate", "--slurp", f"repos/{repository}/actions/workflows/{workflow_id}/runs?per_page=100"', section)
        self.assertIn('f"repos/{repository}/actions/runs/{identifier}/cancel"', section)
        self.assertIn('active = {"requested", "queued", "pending", "waiting", "in_progress"}', section)
        self.assertIn("for _ in range(150):", section)
        self.assertIn('run.get("status") != "completed"', section)
        self.assertIn("Governance writer run identity is invalid.", section)

    def test_invalidator_serializes_dispatchers_and_paces_every_check_write(self) -> None:
        dispatcher_group = "group: pr-governance-dispatcher-${{ github.repository_id }}"
        self.assertEqual(self.workflow.count(dispatcher_group), 1)
        self.assertIn(
            "concurrency:\n      group: pr-governance-dispatcher-${{ github.repository_id }}\n      cancel-in-progress: false",
            self.workflow,
        )
        match = re.search(
            r"- name: Invalidate affected current pull requests before dispatch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = match.group(1)
        self.assertIn("next_write_at=time.monotonic()+8.1", program)
        self.assertIn("time.sleep(delay)", program)
        self.assertLess(program.index("next_write_at=time.monotonic()+8.1"), program.index("for number in numbers:"))

    def test_invalidator_reopens_terminal_trusted_checks_but_marks_carry_only_for_pending_dispatcher_state(self) -> None:
        match = re.search(
            r"- name: Invalidate affected current pull requests before dispatch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = match.group(1)
        self.assertIn('run.get("status")=="completed" and run.get("conclusion") in {"success","failure"}', program)
        self.assertIn('run.get("status")=="in_progress" and run.get("conclusion") is None and is_prior_dispatcher_invalidation', program)
        self.assertIn('draft is False and run is not None', program)
        self.assertIn('type(draft) is not bool', program)
        self.assertIn('"carry_pending":str(carry_pending)', program)

    def test_invalidator_resets_a_prior_dispatcher_marker_on_a_draft_to_carry_zero(self) -> None:
        match = re.search(
            r"- name: Invalidate affected current pull requests before dispatch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = match.group(1).replace("time.sleep(delay)", "None")
        head = "a" * 40
        prior = {
            "id": 101, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
            "head_sha": head, "external_id": "krr-governance/v1/" + head,
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/8?dispatcher_run_id=8&carry_pending=1",
        }
        current = {**prior, "details_url": "https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0"}
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); fake = directory / "gh"; log = directory / "patch.log"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                f"  *'--method PATCH'*) echo \"$*\" >> '{log}'; printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs/101'*) printf '%s' '{json.dumps(current)}' ;;\n"
                f"  *'check-runs?'*) printf '%s' '{json.dumps([{'check_runs': [prior]}])}' ;;\n"
                "  *'/pulls/72'*) printf '%s' \"${PULL}\" ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "AFFECTED": "[72]",
                "PULL": json.dumps({"draft": True, "head": {"sha": head}}),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("carry_pending=0", log.read_text(encoding="utf-8"))

    def test_writer_drain_handles_historical_and_completed_races_but_fails_closed_otherwise(self) -> None:
        match = re.search(
            r"- name: Drain authoritative writer before invalidation.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = match.group(1).replace("time.sleep(2)", "None")
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
        self.assertIn("run-name: source=${{ inputs.dispatcher_run_id }}", writer)
        match = re.search(
            r"- name: Dispatch one repository-wide governance arbiter.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = match.group(1).replace('subprocess.run(["sleep", "2"], check=False)', "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "queued", "run_number": 1, "run_attempt": 1,
        }
        for mode, expected in (("gap", 0), ("bad", 1), ("timeout", 1)):
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
                    "        unrelated = dict(run, id=70, display_title='source=other')\n"
                    "        print(json.dumps([{'workflow_runs': [unrelated, run]}]))\n"
                    "elif '/dispatches' in arguments:\n"
                    "    pass\n"
                    "else:\n"
                    "    raise SystemExit(91)\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                program = base_program.replace("range(150)", "range(2)") if mode == "timeout" else base_program
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "DISPATCHER_RUN_ID": "99", "MODE": mode, "STATE": str(state), "RUN": json.dumps(valid),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)

    def test_invalidator_rejects_wrong_or_malformed_check_app_before_dispatch(self) -> None:
        match = re.search(
            r"- name: Invalidate affected current pull requests before dispatch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        program = match.group(1).replace("time.sleep(delay)", "None")
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
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "PULLS": json.dumps([[{"number": 72, "state": "open"}]]),
                    "AFFECTED": "[72]",
                    "PULL": json.dumps({"draft": False, "head": {"sha": "a" * 40}}), "POST": json.dumps(response),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                self.assertNotEqual(result.returncode, 0)

    def test_invalidator_has_no_all_open_cap_and_continues_after_a_post_failure(self) -> None:
        match = re.search(
            r"- name: Invalidate affected current pull requests before dispatch.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = match.group(1).replace("time.sleep(delay)", "None")
        self.assertNotIn("numbers[:", program)
        self.assertNotIn("len(numbers) >", program)
        for total, failed, expected in ((300, "", 0), (300, "150", 1), (451, "", 0), (600, "", 0)):
            with self.subTest(total=total, failed=failed), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); log = directory / "post.log"; fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'check-runs?'*) printf '%s' '[]' ;;\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'/pulls/'*) printf '%s' \"${PULL}\" ;;\n"
                    "  *'--method POST'*)\n"
                    f"    echo \"$*\" >> '{log}'\n"
                    f"    count=$(awk 'END {{ print NR }}' '{log}')\n"
                    f"    if [ '{failed}' = \"$count\" ]; then exit 7; fi\n"
                    "    printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "PULLS": "[]",
                    "AFFECTED": json.dumps(list(range(1, total + 1))),
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
