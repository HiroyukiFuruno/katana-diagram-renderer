from __future__ import annotations

import json
import importlib.util
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
        return (
            textwrap.dedent(match.group(1))
            .replace("time.sleep(2)", "None")
            .replace("time.sleep(5)", "None")
            .replace("time.sleep(30)", "None")
            .replace('subprocess.run(["sleep", "2"], check=False)', "None")
            .replace('subprocess.run(["sleep", "5"], check=False)', "None")
            .replace('subprocess.run(["sleep", "30"], check=False)', "None")
        )

    def _step_if(self, name: str) -> str:
        match = re.search(
            rf"^      - name: {re.escape(name)}\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.workflow, re.MULTILINE | re.DOTALL,
        )
        self.assertIsNotNone(match, name); assert match is not None
        condition = re.search(r"^        if: (?P<value>.+)$", match.group("body"), re.MULTILINE)
        self.assertIsNotNone(condition, name); assert condition is not None
        return condition.group("value")

    @staticmethod
    def _github_if(expression: str, values: dict[str, str]) -> bool:
        """Evaluate the workflow condition using a strict, non-Python subset."""
        token_pattern = re.compile(
            r"(?P<space>\s+)|(?P<operand>steps\.[A-Za-z0-9_-]+\.(?:outputs\.[A-Za-z0-9_-]+|outcome))|"
            r"(?P<string>'[^'\\]*')|(?P<operator>==|!=|&&|\|\||[!()])"
        )
        tokens: list[tuple[str, str]] = []
        position = 0
        while position < len(expression):
            match = token_pattern.match(expression, position)
            if match is None:
                raise AssertionError(f"Invalid workflow if token at offset {position}")
            position = match.end()
            kind = match.lastgroup
            if kind != "space":
                assert kind is not None
                tokens.append((kind, match.group()))

        referenced = {value for kind, value in tokens if kind == "operand"}
        unknown = referenced - set(values)
        if unknown:
            raise AssertionError(f"Unbound workflow if value: {sorted(unknown)}")

        cursor = 0

        def peek() -> tuple[str, str] | None:
            return tokens[cursor] if cursor < len(tokens) else None

        def take(kind: str, value: str | None = None) -> str:
            nonlocal cursor
            token = peek()
            if token is None or token[0] != kind or (value is not None and token[1] != value):
                raise AssertionError("Invalid workflow if grammar")
            cursor += 1
            return token[1]

        def primary() -> str:
            token = peek()
            if token is not None and token[0] == "operand":
                return values[take("operand")]
            if token is not None and token[0] == "string":
                return take("string")[1:-1]
            raise AssertionError("Invalid workflow if operand")

        def comparison() -> bool:
            left = primary()
            token = peek()
            if token is None or token[0] != "operator" or token[1] not in {"==", "!="}:
                raise AssertionError("Workflow if comparison operator is required")
            operator = take("operator")
            right = primary()
            return left == right if operator == "==" else left != right

        def unary() -> bool:
            if peek() == ("operator", "!"):
                take("operator", "!")
                return not unary()
            if peek() == ("operator", "("):
                take("operator", "(")
                result = disjunction_with_unary()
                take("operator", ")")
                return result
            return comparison()

        # Unary negation binds tighter than conjunction.
        def conjunction_with_unary() -> bool:
            result = unary()
            while peek() == ("operator", "&&"):
                take("operator", "&&")
                result = unary() and result
            return result

        def disjunction_with_unary() -> bool:
            result = conjunction_with_unary()
            while peek() == ("operator", "||"):
                take("operator", "||")
                result = conjunction_with_unary() or result
            return result

        if not tokens:
            raise AssertionError("Empty workflow if expression")
        result = disjunction_with_unary()
        if cursor != len(tokens):
            raise AssertionError("Trailing workflow if tokens")
        return result

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

    def test_github_if_rejects_python_and_unbound_or_dangling_tokens(self) -> None:
        values = {"steps.check.outputs.ready": "true"}
        with self.assertRaises(AssertionError):
            self._github_if("().__class__.__mro__[1].__subclasses__()", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.unknown.outputs.ready == 'true'", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.check.outputs.ready ==", values)
        with self.assertRaises(AssertionError):
            self._github_if("steps.check.outputs.ready == \"true\"", values)

    def test_github_if_supports_only_the_workflow_boolean_subset(self) -> None:
        values = {
            "steps.check.outputs.ready": "true",
            "steps.check.outcome": "success",
        }
        self.assertTrue(self._github_if("(steps.check.outputs.ready == 'true') && ! (steps.check.outcome != 'success')", values))
        self.assertFalse(self._github_if("steps.check.outputs.ready != 'true' || steps.check.outcome != 'success'", values))

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
        self.assertEqual(self.workflow.count("actions/workflows/pr-governance-status-writer.yml/dispatches"), 5)
        self.assertIn("permission-actions: write", self.workflow)
        self.assertIn("permission-checks: write", self.workflow)
        self.assertIn("KRR_GOVERNANCE_APP_BOT_LOGIN", writer)
        self.assertIn("github.triggering_actor == vars.KRR_GOVERNANCE_APP_BOT_LOGIN", writer)

    def test_priority_event_preempts_a_long_reconciliation_and_writer_rebinds_before_secrets(self) -> None:
        self.assertIn(
            "cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}",
            self.workflow,
        )
        self.assertIn("PRs may edit a workflow file", self.workflow)
        self.assertIn("Check Run fingerprint fence", self.workflow)
        writer = (ROOT / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")
        writer_program = (ROOT / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        # A queued dispatcher is a generation fence even before its pending
        # Check Run is visible. The writer must inspect the immutable
        # dispatcher-run snapshot before every terminal mutation.
        self.assertIn("def reject_newer_dispatcher_barrier", writer_program)
        self.assertIn("dispatcher_generations(\n            current_generation.workflow_id, current_generation.created_at,", writer_program)
        self.assertIn("Read bounded, exact-workflow generations no older than the source.", writer_program)
        self.assertIn("per_page=100", writer_program)
        self.assertIn("A newer dispatcher generation owns this Check Run head.", writer_program)
        self.assertGreaterEqual(writer_program.count("reject_newer_dispatcher_barrier(head)"), 2)
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
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        invalidator = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(current); self.assertIsNotNone(invalidator)
        assert current is not None and invalidator is not None
        base, head = "b" * 40, "a" * 40
        pull = {
            "number": 72, "state": "open", "body": "Fixes #64", "draft": False,
            "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": head, "repo": {"full_name": "owner/repository"}},
        }
        pulls = [[pull]]
        for status in ("requested", "waiting"):
            with self.subTest(status=status), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                run = {"name": "CI", "path": ".github/workflows/test-and-build.yml@main", "event": "pull_request", "status": status, "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head, "repository": {"full_name": "owner/repository"}, "pull_requests": [{"number": 72, "base": {"sha": base, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}]}
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n", encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                resolved = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(resolved["reconcile"], "true")
                self.assertEqual(resolved["event_targets"], "[72]")
                self.assertEqual(resolved["priority_targets"], "[]")

                selection_output = directory / "selection-output"
                selected = subprocess.run(
                    [sys.executable, "-c", self._workflow_program(current)],
                    env=environment | {
                        "GITHUB_OUTPUT": str(selection_output),
                        "EVENT_TARGETS": resolved["event_targets"],
                        "EVENT_PRIORITY_TARGETS": resolved["priority_targets"],
                    }, capture_output=True, text=True, check=False,
                )
                self.assertEqual(selected.returncode, 0, selected.stderr)
                selection = dict(line.split("=", 1) for line in selection_output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(selection["preinvalidate_targets"], "[]")
                self.assertEqual(selection["all_invalidation_targets"], "[72]")
                self.assertEqual(selection["has_duplicate_governed_heads"], "false")

                posts: list[list[str]] = []
                created: dict[int, dict[str, object]] = {}
                def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                    return subprocess.CompletedProcess([], code, json.dumps(value), "")
                def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                    endpoint = arguments[-1]
                    if "--method" in arguments and "POST" in arguments:
                        self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                        posts.append(arguments)
                        fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if "=" in field}
                        identifier = 500 + len(posts)
                        check = {
                            "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                            "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                            "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                        }
                        created[identifier] = check
                        return response(check)
                    if isinstance(endpoint, str) and "/commits/" in endpoint and "check-runs?" in endpoint:
                        return response([{"check_runs": []}])
                    if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                        return response(created[int(endpoint.rsplit("/", 1)[1])])
                    if isinstance(endpoint, str) and endpoint.endswith("/pulls/72"):
                        return response(pull)
                    raise AssertionError(arguments)
                invalidation_output = directory / "invalidation-output"
                invalidation_environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                    "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42",
                    "AFFECTED": selection["all_invalidation_targets"],
                    "KNOWN_TARGET_SNAPSHOTS": selection["all_invalidation_target_snapshots"],
                    "EVENT_TARGETS": resolved["event_targets"], "DUPLICATE_GOVERNED_HEADS": "[]",
                    "GITHUB_OUTPUT": str(invalidation_output), "PATH": os.environ["PATH"],
                }
                with patch.dict(os.environ, invalidation_environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    exec(self._workflow_program(invalidator), {"__name__": "__main__"})
                self.assertEqual(len(posts), 1)
                manifest = dict(line.split("=", 1) for line in invalidation_output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(json.loads(manifest["check_manifest"]), [[72, 501]])

    def test_workflow_run_prioritizes_review_sensor_but_not_ci_or_release(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        base, head = "b" * 40, "a" * 40
        local = {"base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", **local},
            {"number": 73, "state": "open", "body": "Closes #64", **local},
        ]]
        cases = (
            ("CI", ".github/workflows/test-and-build.yml@master", "pull_request", "[]"),
            ("release-preflight", ".github/workflows/release-preflight.yml@master", "pull_request", "[]"),
            ("PR governance review sensor", ".github/workflows/pr-governance-review-events.yml@master", "pull_request_review", "[72,73]"),
        )
        for name, path, event, expected_priority in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                run = {
                    "name": name, "path": path, "event": event, "status": "completed",
                    "id": 9, "run_number": 1, "run_attempt": 1, "head_sha": head,
                    "repository": {"full_name": "owner/repository"},
                    "pull_requests": [{"number": 72, "base": {"sha": base, **local["base"]}, "head": {"sha": head, **local["head"]}}],
                }
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'/actions/runs/9'*) printf '%s' \"${RUN}\" ;;\n"
                    "  *'/contents/'*) printf '%s' '{\"sha\":\"cccccccccccccccccccccccccccccccccccccccc\"}' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "EVENT_NAME": "workflow_run", "WORKFLOW_RUN_ID": "9",
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                    "GITHUB_OUTPUT": str(output), "RUN": json.dumps(run), "PULLS": json.dumps(pulls),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values["event_targets"], "[72,73]")
                self.assertEqual(values["priority_targets"], expected_priority)

    def test_issue_and_issue_comment_priority_all_closers_of_the_changed_issue(self) -> None:
        match = re.search(r"- name: Resolve current open pull requests from the trusted default branch.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
        self.assertIsNotNone(match); assert match is not None
        pulls = [[
            {"number": 72, "state": "open", "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
            {"number": 73, "state": "open", "body": "Closes https://github.com/owner/repository/issues/64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}},
        ]]
        for event_name, issue, expected in (
            ("issues", "999", {"reconcile": "false", "event_targets": "[]", "priority_targets": "[]"}),
            ("issues", "64", {"reconcile": "true", "event_targets": "[72,73]", "priority_targets": "[72,73]"}),
            ("issue_comment", "64", {"reconcile": "true", "event_targets": "[72,73]", "priority_targets": "[72,73]"}),
        ):
            with self.subTest(event_name=event_name, issue=issue), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text("#!/bin/sh\nprintf '%s' \"${PULLS}\"\n", encoding="utf-8"); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {"EVENT_NAME": event_name, "ISSUE_NUMBER": issue, "ISSUE_PULL_REQUEST_URL": "", "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}"}
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
        for action, source, expected in (
            ("opened", current, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("edited", current, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("closed", {**current, "state": "closed"}, {"reconcile": "true", "event_targets": "[73,72]", "priority_targets": "[73,72]"}),
            ("edited", {**current, "head": {"sha": head, "repo": {"full_name": "fork/repository"}}}, {"reconcile": "false", "event_targets": "[]", "priority_targets": "[]"}),
        ):
            with self.subTest(action=action, source=source["head"]["repo"]["full_name"]), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); fake = directory / "gh"; output = directory / "output"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n  *'/pulls/73'*) printf '%s' \"${SOURCE}\" ;;\n  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "EVENT_NAME": "pull_request_target", "PR_ACTION": action, "PR_NUMBER": "73", "PR_HEAD_SHA": head,
                    "PR_BASE_SHA": base, "PR_BODY": "Fixes #65", "PR_PREVIOUS_BODY": "Fixes #64", "GITHUB_REPOSITORY": "owner/repository",
                    "DEFAULT_BRANCH": "master", "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pulls), "SOURCE": json.dumps(source),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(match)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(values, expected)

    def test_priority_snapshot_accepts_all_related_closers(self) -> None:
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
            self.assertEqual(resolved["priority_targets"], "[72,73]")
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
            self.assertEqual(json.loads(selected["priority_targets"]), [72, 73])
            self.assertEqual(selected["event_targets"], "[72,73]")
            self.assertEqual(json.loads(selected["event_targets"]), [72, 73])
            invalidations = json.loads(selected["all_invalidation_targets"])
            self.assertNotIn(72, invalidations)
            self.assertNotIn(73, invalidations)
            self.assertEqual(invalidations[0], 1)
            self.assertEqual(len(json.loads(selected["targets"])), 105)
        early = self.workflow.index("Dispatch and bind the early event writer")
        full = self.workflow.index("Invalidate every current pull request for the all-open writer")
        self.assertNotIn("AFFECTED: ${{ steps.current-targets.outputs.priority_targets }}", self.workflow[early:full])
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow[full:])

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
                "EVENT_TARGETS": "[72,73]", "EVENT_PRIORITY_TARGETS": "[72,73]", "PULLS": json.dumps(pulls),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            selected = subprocess.run(
                [sys.executable, "-c", self._workflow_program(current)],
                env=selection_environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(selected.returncode, 0, selected.stderr)
            selection = dict(line.split("=", 1) for line in selection_output.read_text(encoding="utf-8").splitlines())
            self.assertEqual(selection["priority_targets"], "[]")
            self.assertEqual(json.loads(selection["preinvalidate_targets"]), [72, 73])
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [74])

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
                "DUPLICATE_GOVERNED_HEADS": json.dumps([duplicate_head]),
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
            self.assertEqual(len(writes), 1)
            self.assertIn(f"head_sha={unique_head}", writes[0])
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
            self.assertEqual(json.loads(selection["preinvalidate_targets"]), [72])
            self.assertEqual(json.loads(selection["all_invalidation_targets"]), [73, 74])

    def test_shared_heads_are_single_generation_and_block_all_writer_before_manifest(self) -> None:
        """Every shared-head shape is fenced once before the all-writer hand-off."""
        current = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(current); assert current is not None

        local_base = {"ref": "master", "repo": {"full_name": "owner/repository"}}
        first, second, third = "a" * 40, "b" * 40, "c" * 40
        cases = (
            (
                "event duplicate",
                [72, 73],
                [72, 73],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": third, "repo": {"full_name": "owner/repository"}}},
                ],
                [72, 73], [74], [first], [first],
            ),
            (
                "event and unrelated share",
                [72],
                [72],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": local_base, "head": {"sha": third, "repo": {"full_name": "owner/repository"}}},
                ],
                [72], [74], [first], [first],
            ),
            (
                "unrelated shared suppresses unique source",
                [72],
                [72],
                [
                    {"number": 72, "state": "open", "body": "Fixes #64", "draft": False, "base": local_base, "head": {"sha": first, "repo": {"full_name": "owner/repository"}}},
                    {"number": 73, "state": "open", "body": "Fixes #99", "draft": False, "base": local_base, "head": {"sha": second, "repo": {"full_name": "owner/repository"}}},
                    {"number": 74, "state": "open", "body": "Fixes #100", "draft": False, "base": local_base, "head": {"sha": second, "repo": {"full_name": "owner/repository"}}},
                ],
                [72], [73, 74], [first], [second],
            ),
        )
        selections: dict[str, dict[str, str]] = {}
        for name, event_targets, priority_targets, pulls, expected_pre, expected_all, expected_pre_heads, expected_duplicate in cases:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"; fake = directory / "gh"
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
                    "EVENT_TARGETS": json.dumps(event_targets), "EVENT_PRIORITY_TARGETS": json.dumps(priority_targets),
                    "PULLS": json.dumps([pulls]), "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", self._workflow_program(current)], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, 0, result.stderr)
                selected = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                self.assertEqual(json.loads(selected["preinvalidate_targets"]), expected_pre)
                self.assertEqual(json.loads(selected["preinvalidated_heads"]), expected_pre_heads)
                self.assertEqual(json.loads(selected["duplicate_governed_heads"]), expected_duplicate)
                self.assertEqual(selected["has_duplicate_governed_heads"], "true")
                self.assertEqual(json.loads(selected["all_invalidation_targets"]), expected_all)
                pre_heads = set(json.loads(selected["preinvalidated_heads"]))
                all_heads = {entry[1] for entry in json.loads(selected["all_invalidation_target_snapshots"])}
                self.assertTrue(pre_heads.isdisjoint(all_heads))
                selections[name] = selected

        pre_match = re.search(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        all_match = re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(pre_match); self.assertIsNotNone(all_match)
        assert pre_match is not None and all_match is not None
        pre_program = self._workflow_program(pre_match)
        all_program = self._workflow_program(all_match).replace("time.sleep(delay)", "None").replace("write_clock=[time.monotonic()+8.1]", "write_clock=[time.monotonic()]")

        def snapshot_pulls(selected: dict[str, str]) -> dict[int, dict[str, object]]:
            records: dict[int, dict[str, object]] = {}
            raw = json.loads(selected["preinvalidate_target_snapshots"] or "[]") + json.loads(selected["all_invalidation_target_snapshots"] or "[]")
            for entry in raw:
                base_ref, base_repo, head_repo = (entry[3], entry[4], entry[5]) if len(entry) == 6 else ("master", "owner/repository", "owner/repository")
                records[entry[0]] = {
                    "number": entry[0], "state": "open", "draft": entry[2],
                    "base": {"ref": base_ref, "repo": {"full_name": base_repo}},
                    "head": {"sha": entry[1], "repo": {"full_name": head_repo}},
                }
            return records

        def run_pre(selected: dict[str, str]) -> tuple[list[str], list[str]]:
            records = snapshot_pulls(selected)
            targets = json.loads(selected["preinvalidate_targets"])
            posts: list[str] = []; rereads: list[str] = []; created: dict[int, dict[str, object]] = {}
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    return response(records[int(endpoint.rsplit("/", 1)[1])])
                if "--method" in arguments and "POST" in arguments:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 700 + len(posts) + 1
                    check = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                    created[identifier] = check; posts.append(fields["head_sha"])
                    return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    identifier = int(endpoint.rsplit("/", 1)[1]); rereads.append(str(identifier)); return response(created[identifier])
                raise AssertionError(arguments)
            environment = os.environ | {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "TARGETS": selected["preinvalidate_targets"], "TARGET_SNAPSHOTS": selected["preinvalidate_target_snapshots"], "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9", "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "output"), "PATH": os.environ["PATH"]}
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                exec(pre_program, {"__name__": "__main__"})
            self.assertEqual(len(posts), len(set(posts)))
            return posts, rereads

        def run_all(selected: dict[str, str], expected_returncode: int) -> tuple[list[str], list[list[int]]]:
            records = snapshot_pulls(selected)
            targets = json.loads(selected["all_invalidation_targets"])
            posts: list[str] = []; writer_dispatches: list[list[str]] = []; created: dict[int, dict[str, object]] = {}; writer_runs: list[dict[str, object]] = []; output = Path(tempfile.mkdtemp()) / "output"
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "/commits/" in endpoint and "check-runs?" in endpoint:
                    return response([{"check_runs": []}])
                if "--method" in arguments and "POST" in arguments and isinstance(endpoint, str) and endpoint.endswith("/check-runs"):
                    self.assertEqual(kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 800 + len(posts) + 1
                    check = {"id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"], "external_id": fields["external_id"], "status": "in_progress", "conclusion": None, "details_url": fields["details_url"]}
                    created[identifier] = check; posts.append(fields["head_sha"]); return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    return response(created[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    return response(records[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                    return response({"id": 9, "name": "PR governance dispatcher", "event": "issues", "head_branch": "master", "head_sha": "a" * 40, "repository": {"full_name": "owner/repository"}, "run_attempt": 1, "status": "in_progress"})
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    return response([list(records.values())])
                if isinstance(endpoint, str) and "pr-governance-status-writer.yml/runs?" in endpoint:
                    return response([{"workflow_runs": writer_runs}])
                if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                    writer_dispatches.append(arguments)
                    writer_runs.append({"id": 901, "name": "PR governance status writer", "display_title": "source=9 scope=all segment=1", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40, "status": "queued", "run_number": 1, "run_attempt": 1})
                    return response({})
                raise AssertionError(arguments)
            environment = os.environ | {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9", "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": selected["duplicate_governed_heads"], "AFFECTED": selected["all_invalidation_targets"], "KNOWN_TARGET_SNAPSHOTS": selected["all_invalidation_target_snapshots"], "EVENT_TARGETS": selected["event_targets"], "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"]}
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                try:
                    exec(all_program, {"__name__": "__main__"})
                    code = 0
                except SystemExit:
                    code = 1
            self.assertEqual(code, expected_returncode)
            manifest = [] if not output.exists() else json.loads(dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines()).get("check_manifest", "[]"))
            dispatch_if = self._step_if("Dispatch one repository-wide governance arbiter segment")
            eligibility = self._github_if(dispatch_if, {
                "steps.current-targets.outputs.has_targets": "true",
                "steps.current-targets.outputs.has_duplicate_governed_heads": "true" if expected_returncode else "false",
            })
            if expected_returncode:
                self.assertFalse(eligibility)
                self.assertEqual(writer_dispatches, [])
            else:
                self.assertTrue(eligibility)
                dispatch_program = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", re.search(r"^      - name: Dispatch one repository-wide governance arbiter segment\n(?P<body>.*?)(?=^      - name: |\Z)", self.workflow, re.MULTILINE | re.DOTALL).group("body"), re.DOTALL)
                self.assertIsNotNone(dispatch_program); assert dispatch_program is not None
                snapshots = [[number, records[number]["head"]["sha"], records[number]["draft"]] for number in targets]
                dispatch_env = environment | {
                    "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "9", "WRITER_SCOPE": "all", "WRITER_TARGETS": selected["event_targets"],
                    "WRITER_ALL_OPEN_TARGETS": json.dumps(targets, separators=(",", ":")), "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(snapshots, separators=(",", ":")),
                    "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0", "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]", "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                    "WRITER_TAIL_CHECK_MANIFEST_1": json.dumps(manifest, separators=(",", ":")), "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]", "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "dispatch-output"),
                }
                with patch.dict(os.environ, dispatch_env, clear=True), patch("subprocess.run", side_effect=fake_run):
                    exec(textwrap.dedent(dispatch_program.group(1)), {"__name__": "__main__"})
                self.assertEqual(len(writer_dispatches), 1)
            return posts, manifest

        for name, _, _, _, expected_pre, expected_all, expected_pre_heads, expected_duplicate in cases:
            selected = selections[name]
            pre_posts, pre_rereads = run_pre(selected)
            self.assertEqual(set(pre_posts), set(expected_pre_heads))
            self.assertEqual(len(pre_rereads), len(pre_posts))
            all_posts, manifest = run_all(selected, 1)
            expected_all_heads = [entry[1] for entry in json.loads(selected["all_invalidation_target_snapshots"])]
            self.assertEqual(set(all_posts), set(expected_all_heads))
            self.assertEqual(len(all_posts), len(set(all_posts)))
            self.assertEqual(manifest, [])

        no_duplicate = {
            "preinvalidate_target_snapshots": "[]",
            "all_invalidation_targets": "[72,74]",
            "all_invalidation_target_snapshots": json.dumps([[72, first, False], [74, third, False]], separators=(",", ":")),
            "duplicate_governed_heads": "[]", "event_targets": "[72,74]",
        }
        no_duplicate_posts, no_duplicate_manifest = run_all(no_duplicate, 0)
        self.assertEqual(set(no_duplicate_posts), {first, third})
        self.assertEqual(len(no_duplicate_posts), 2)
        self.assertEqual([entry[0] for entry in no_duplicate_manifest], [72, 74])

        all_step = self.workflow.index("- name: Invalidate every current pull request for the all-open writer")
        dispatch_step = self.workflow.index("- name: Dispatch one repository-wide governance arbiter")
        all_header = self.workflow[all_step: self.workflow.index("run: |", all_step)]
        dispatch_header = self.workflow[dispatch_step: self.workflow.index("run: |", dispatch_step)]
        invalidator = self._workflow_program(re.search(
            r"- name: Invalidate every current pull request for the all-open writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        ))
        self.assertIn("DUPLICATE_GOVERNED_HEADS:", all_header)
        self.assertIn("DUPLICATE_GOVERNED_HEADS:", all_header)
        self.assertIn("has_all_invalidation_chunk_1 == 'true'", all_header)
        self.assertIn("if duplicate_heads:", invalidator)
        self.assertIn('raise SystemExit("Duplicate governed pull request head SHA.")', invalidator)
        self.assertIn('external_id=f"krr-governance/v1/{head.lower()}/dispatcher-{dispatcher}"', invalidator)
        self.assertRegex(dispatch_header, r"has_duplicate_governed_heads (?:!= 'true'|== 'false')")
        self.assertNotIn("has_duplicate_governed_heads == 'true'", dispatch_header)

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
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow)
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
                    values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                    self.assertEqual(values["has_targets"], "true")
                    self.assertEqual(json.loads(values["targets"]), [64, 65])
                    self.assertEqual(json.loads(values["event_targets"]), [64, 65])
                    self.assertEqual(json.loads(values["priority_targets"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_targets"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_chunk_1"]), [64])
                    self.assertEqual(json.loads(values["preinvalidate_chunk_2"]), [])
                    self.assertEqual(json.loads(values["preinvalidated_heads"]), ["d" * 40])
                    self.assertEqual(json.loads(values["all_invalidation_targets"]), [65])
                    self.assertEqual(json.loads(values["all_invalidation_chunk_1"]), [65])
                    self.assertEqual(json.loads(values["all_invalidation_chunk_2"]), [])
                    self.assertEqual(values["duplicate_governed_heads"], "[]")
                    self.assertEqual(values["writer_head"], "a" * 40)
                    self.assertEqual(values["default_branch"], "master")

    def test_priority_event_preempts_the_current_reconciler_and_preserves_affected_order(self) -> None:
        # sourceを持つeventだけが全件走査を中断する。通常の全件走査は
        # current snapshotを取り直すが、event由来のcloser集合はwriterへ順序を渡す。
        self.assertIn("needs: resolve_event", self.workflow)
        self.assertIn("if: needs.resolve_event.outputs.reconcile == 'true'", self.workflow)
        self.assertIn("Re-enumerate every current local governance pull request", self.workflow)
        self.assertNotIn("steps.targets.outputs.affected", self.workflow)
        self.assertIn("AFFECTED: ${{ steps.current-targets.outputs.all_invalidation_chunk_1 }}", self.workflow)
        self.assertIn("cancel-in-progress: ${{ needs.resolve_event.outputs.priority_targets != '[]' }}", self.workflow)
        self.assertIn("WRITER_TARGETS: ${{ steps.current-targets.outputs.event_targets }}", self.workflow)

    def test_priority_preinvalidation_precedes_drain_and_normal_drain_preserves_token_boundaries(self) -> None:
        preinvalidate = self.workflow.index("Pre-invalidate priority event heads")
        drain = self.workflow.index("Drain authoritative writer before the next governance hand-off")
        invalidate = self.workflow.index("Invalidate every current pull request for the all-open writer")
        dispatch = self.workflow.index("Dispatch one repository-wide governance arbiter")
        # Priority traffic gets a synchronous pending Check Run before drain;
        # the drain then completes before the early writer is dispatched.
        self.assertLess(preinvalidate, drain)
        self.assertLess(drain, invalidate)
        self.assertLess(invalidate, dispatch)
        next_step = self.workflow.index("- name: Dispatch and bind the early event writer", drain)
        section = self.workflow[drain:next_step]
        self.assertIn("if: steps.current-targets.outputs.has_targets == 'true'", self.workflow[drain:next_step])
        self.assertNotIn("has_preinvalidate_targets != 'true'", self.workflow[drain:next_step])
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
            "CHECK_READ_TOKEN: ${{ steps.early-check-read-token.outputs.token }}",
            'run.get("display_title")!=title', 'run.get("head_sha")!=head',
            'run.get("status")=="completed"', 'run.get("conclusion")!="success"',
        ):
            self.assertIn(value, wait_section)
        self.assertNotIn("CHECK_READ_TOKEN: ${{ steps.invalidator-token.outputs.token }}", wait_section)
        self.assertIn('env={"GH_TOKEN":check_token,"PATH":os.environ["PATH"]}', wait_section)
        read_token = self.workflow[self.workflow.index("Create first priority invalidator read token"):self.workflow.index("Dispatch and bind the early event writer")]
        self.assertIn("permission-checks: read", read_token)

    def test_early_dispatch_binds_exact_new_writer_or_fails_closed(self) -> None:
        match = re.search(
            r"- name: Dispatch and bind the early event writer.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        base_program = self._workflow_program(match).replace("time.sleep(2)", "None")
        valid = {
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early segment=0",
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
                    "    if 'inputs[scope]=early' not in arguments or 'inputs[target_numbers]=[72,73]' not in arguments or 'inputs[preserved_target_numbers]=[]' not in arguments or 'inputs[preserved_writer_run_id]=0' not in arguments or 'inputs[terminal_order_numbers]=[]' not in arguments or 'inputs[completed_writer_run_ids]=[]' not in arguments: raise SystemExit(92)\n"
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
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=early segment=0",
            "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch",
            "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40,
            "status": "completed", "conclusion": "success", "run_number": 1, "run_attempt": 1,
            "actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"},
            "triggering_actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"},
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
                    f"  checks-read:*'/pulls/72'*) printf '%s' '{{\"head\":{{\"sha\":\"{'a' * 40}\"}}}}' ;;\n"
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
                    "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                    "GITHUB_SERVER_URL": "https://github.com", "GITHUB_OUTPUT": str(directory / "output"),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", base_program], env=environment, capture_output=True, text=True, check=False)
                self.assertEqual(result.returncode, expected, result.stderr)
                if expected == 0:
                    self.assertEqual(token_log.read_text(encoding="utf-8"), "checks-read")

    def test_priority_preinvalidation_is_synchronous_unique_and_fail_closed(self) -> None:
        """Priority heads receive a newer pending generation before any writer can finish."""
        match = re.search(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(match); assert match is not None
        program = self._workflow_program(match)
        # The stable snapshot is produced at the same boundary as the current
        # priority list.  A fresh PR GET alone cannot distinguish a head that
        # changed after that boundary from the event which is being fenced.
        self.assertIn("preinvalidate_target_snapshots", self.workflow)
        self.assertIn("TARGET_SNAPSHOTS", self.workflow)
        self.assertIn("CHECK_WRITE_TOKEN", self.workflow)
        preinvalidate = self.workflow.index("- name: Pre-invalidate priority event heads")
        early_dispatch = self.workflow.index("- name: Dispatch and bind the early event writer")
        await_early = self.workflow.index("- name: Await the bound early event writer before all-open invalidation")
        drain = self.workflow.index("- name: Drain authoritative writer before the next governance hand-off")
        self.assertLess(preinvalidate, drain)
        self.assertLess(drain, early_dispatch)
        self.assertLess(early_dispatch, await_early)
        preinvalidate_step = self.workflow[preinvalidate:self.workflow.index("- name: Dispatch and bind the early event writer", preinvalidate)]
        self.assertIn("GH_TOKEN: ${{ steps.pre-invalidator-read-1.outputs.token }}", preinvalidate_step)
        self.assertIn("CHECK_WRITE_TOKEN: ${{ steps.pre-invalidator-write-1.outputs.token }}", preinvalidate_step)
        self.assertIn('read_env={"GH_TOKEN":read_token,"PATH":os.environ["PATH"]}', preinvalidate_step)
        self.assertIn("env=read_env", preinvalidate_step)
        self.assertNotIn("READ_TOKEN:", preinvalidate_step)
        for value in (
            "len(entry)!=6", "entry[0]!=number", "entry[3]!=branch",
            "entry[4]!=repository", "entry[5]!=repository", "current_number!=number",
            "current_base_ref!=entry[3]", "current_base_repo!=entry[4]",
            "current_head_repo!=entry[5]", 'pull.get("state")!="open"',
        ):
            self.assertIn(value, preinvalidate_step)

        first, second = "a" * 40, "b" * 40
        def execute(heads: dict[int, str], snapshots: list[list[object]], mode: str = "valid") -> tuple[int, list[list[str]], list[str], list[str]]:
            posts: list[list[str]] = []; rereads: list[str] = []; sleeps: list[float] = []; created: dict[int, dict[str, object]] = {}
            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")
            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = arguments[-1]
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    number = int(endpoint.rsplit("/", 1)[1])
                    value = heads[number]
                    if mode == "head-drift" and number == 72:
                        value = "c" * 40
                    current: dict[str, object] = {
                        "number": number,
                        "state": "open",
                        "draft": False,
                        "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                        "head": {"sha": value, "repo": {"full_name": "owner/repository"}},
                    }
                    if mode == "number-drift" and number == 72:
                        current["number"] = 73
                    if mode == "state-drift" and number == 72:
                        current["state"] = "closed"
                    if mode == "draft-drift" and number == 72:
                        current["draft"] = True
                    if mode == "base-ref-drift" and number == 72:
                        current["base"] = {"ref": "release", "repo": {"full_name": "owner/repository"}}
                    if mode == "base-repo-drift" and number == 72:
                        current["base"] = {"ref": "master", "repo": {"full_name": "other/repository"}}
                    if mode == "head-repo-drift" and number == 72:
                        current["head"] = {"sha": value, "repo": {"full_name": "fork/repository"}}
                    return response(current)
                if "--method" in arguments and "POST" in arguments:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "write", "PATH": os.environ["PATH"]})
                    posts.append(arguments)
                    if mode == "post-failure" and len(posts) == 2:
                        return response({}, 7)
                    fields = {field.split("=", 1)[0]: field.split("=", 1)[1] for field in arguments if "=" in field}
                    identifier = 100 + len(created)
                    check: dict[str, object] = {
                        "id": identifier, "app": {"id": 42}, "name": "KRR / PR governance (trusted check)",
                        "head_sha": fields["head_sha"], "external_id": fields["external_id"],
                        "status": "in_progress", "conclusion": None, "details_url": fields["details_url"],
                    }
                    created[identifier] = check
                    return response("malformed" if mode == "malformed-post" else check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertEqual(_kwargs.get("env"), {"GH_TOKEN": "read", "PATH": os.environ["PATH"]})
                    rereads.append(endpoint)
                    identifier = int(endpoint.rsplit("/", 1)[1]); current = dict(created[identifier])
                    if mode == "stale-reread": current["status"] = "completed"; current["conclusion"] = "success"
                    if mode == "wrong-reread-app": current["app"] = {"id": 7}
                    if mode == "wrong-reread-id": current["id"] = 999
                    if mode == "wrong-reread-name": current["name"] = "other"
                    if mode == "wrong-reread-head": current["head_sha"] = "c" * 40
                    if mode == "wrong-reread-external": current["external_id"] = "krr-governance/v1/" + "c" * 40 + "/dispatcher-9"
                    if mode == "wrong-reread-details": current["details_url"] = "https://example.invalid/other"
                    return response(current)
                raise AssertionError(arguments)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "TARGETS": json.dumps(sorted(heads)),
                "TARGET_SNAPSHOTS": json.dumps(snapshots), "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9",
                "GITHUB_OUTPUT": str(Path(tempfile.mkdtemp()) / "output"),
                "PATH": os.environ["PATH"],
            }
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep", side_effect=lambda seconds: sleeps.append(seconds)):
                try:
                    exec(program, {"__name__": "__main__"})
                    return 0, posts, sleeps, rereads
                except SystemExit:
                    return 1, posts, sleeps, rereads

        snapshot = lambda number, head, draft=False: [number, head, draft, "master", "owner/repository", "owner/repository"]
        result, posts, sleeps, rereads = execute({72: first, 73: second}, [snapshot(72, first), snapshot(73, second)])
        self.assertEqual(result, 0)
        self.assertEqual(len(posts), 2)
        self.assertEqual(len(rereads), 2)
        self.assertGreaterEqual(len(sleeps), 1)
        for expected_head, post in zip((first, second), posts):
            self.assertIn(f"head_sha={expected_head}", post)
            self.assertIn(f"external_id=krr-governance/v1/{expected_head}/dispatcher-9", post)
            self.assertIn("status=in_progress", post)
        # A shared head is one immutable namespace: it gets exactly one new
        # dispatcher generation and cannot let an old writer success win.
        result, duplicate_posts, _, duplicate_rereads = execute({72: first, 73: first}, [snapshot(72, first), snapshot(73, first)])
        self.assertEqual(result, 0)
        self.assertEqual(len(duplicate_posts), 1)
        self.assertEqual(len(duplicate_rereads), 1)
        for mode in ("head-drift", "number-drift", "state-drift", "draft-drift", "base-ref-drift", "base-repo-drift", "head-repo-drift", "malformed-post", "stale-reread", "wrong-reread-app", "wrong-reread-id", "wrong-reread-name", "wrong-reread-head", "wrong-reread-external", "wrong-reread-details"):
            with self.subTest(mode=mode):
                result, _, _, _ = execute({72: first, 73: second}, [snapshot(72, first), snapshot(73, second)], mode)
                self.assertEqual(result, 1)
        # Failure is recorded per head, but a later head is still made pending
        # before the batch exits.  This prevents a partial API outage from
        # leaving an unrelated affected closer with an older success.
        third = "c" * 40
        result, partial_posts, _, _ = execute(
            {72: first, 73: second, 74: third},
            [snapshot(72, first), snapshot(73, second), snapshot(74, third)], "post-failure",
        )
        self.assertEqual(result, 1)
        self.assertEqual(len(partial_posts), 3)
        self.assertIn(f"head_sha={third}", partial_posts[-1])
        for snapshots in (
            [snapshot(71, first), snapshot(73, second)],
            [snapshot(72, "c" * 40), snapshot(73, second)],
            [snapshot(72, first, True), snapshot(73, second)],
        ):
            with self.subTest(snapshots=snapshots):
                result, _, _, _ = execute({72: first, 73: second}, snapshots)
                self.assertEqual(result, 1)

    def test_static_barrier_is_atomic_and_requires_fresh_complete_recovery(self) -> None:
        """Execute the trusted YAML programs against atomic context API state transitions."""
        def program(name: str) -> str:
            match = re.search(rf"- name: {re.escape(name)}.*?python3 - <<'PY'\n(.*?)\n          PY", self.workflow, re.DOTALL)
            self.assertIsNotNone(match, name); assert match is not None
            return self._workflow_program(match)

        source = program("Verify default-branch governance source before barrier credentials")
        activate = program("Activate complete affected-head merge barrier")
        release = program("Release complete affected-head merge barrier only after full pending coverage")
        marker = program("Publish periodic static affected-head barrier App marker")
        barrier = "KRR / PR governance affected-head barrier"; head, other = "a" * 40, "b" * 40
        self.assertLess(self.workflow.index("Activate complete affected-head merge barrier"), self.workflow.index("Pre-invalidate priority event heads"))
        self.assertLess(self.workflow.index("Release complete affected-head merge barrier only after full pending coverage"), self.workflow.index("Dispatch one repository-wide governance arbiter segment"))
        self.assertNotIn("required_status_checks\",\"--input\",\"-\"", self.workflow)
        self.assertIn("required_status_checks/contexts", activate)
        self.assertIn("required_status_checks/contexts", release)
        self.assertNotIn("actions/checkout", self.workflow)
        marker_condition = "(github.event_name == 'schedule' || github.event_name == 'workflow_dispatch') && steps.barrier-source.outcome == 'success'"
        self.assertEqual(self.workflow.count(marker_condition), 3)
        marker_steps = self.workflow[self.workflow.index("Create periodic affected-head barrier marker write token"):self.workflow.index("Create affected-head barrier branch-protection token")]
        self.assertNotIn("has_preinvalidate_targets", marker_steps)
        baseline = {
            "required_status_checks": {"strict": True, "contexts": ["CI / test"], "checks": [{"context": "CI / test", "app_id": None}]},
            "enforce_admins": {"enabled": True}, "required_conversation_resolution": {"enabled": True},
        }
        state: dict[str, object] = {
            "protection": json.loads(json.dumps(baseline)), "mutations": [], "uncertain_delete": False,
            "pulls": [
                {"number": 72, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": head, "repo": {"full_name": "owner/repository"}}},
                {"number": 73, "state": "open", "draft": False, "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": other, "repo": {"full_name": "owner/repository"}}},
            ],
        }
        run = {"id": 99, "name": "PR governance dispatcher", "path": ".github/workflows/pr-governance.yml@master", "event": "issue_comment", "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": head, "run_attempt": 1, "status": "in_progress", "created_at": "2026-08-30T00:00:00Z"}
        state["runs"] = [run]
        state["manifest_checks"] = {
            801: {"id": 801, "name": "KRR / PR governance (trusted check)", "head_sha": head, "external_id": f"krr-governance/v1/{head}/dispatcher-99", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}},
            802: {"id": 802, "name": "KRR / PR governance (trusted check)", "head_sha": other, "external_id": f"krr-governance/v1/{other}/dispatcher-99", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}},
        }
        state["late_event_without_run_list"] = None
        state["late_event_observed_at_release"] = False

        def completed(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        def route(arguments: list[str]) -> str:
            values = [value for value in arguments if isinstance(value, str) and value.startswith("repos/")]
            self.assertEqual(len(values), 1, arguments)
            return values[0]

        def protection_records() -> list[dict[str, object]]:
            protection = state["protection"]; self.assertIsInstance(protection, dict)
            required = protection["required_status_checks"]; self.assertIsInstance(required, dict)
            checks = required["checks"]; self.assertIsInstance(checks, list)
            return checks

        def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertEqual(arguments[:4], ["gh", "api", "--hostname", "github.com"])
            endpoint = route(arguments); environment = kwargs.get("env"); self.assertIsInstance(environment, dict)
            token = environment.get("GH_TOKEN")  # type: ignore[union-attr]
            if endpoint == "repos/owner/repository":
                self.assertIn(token, {"source", "read"}); return completed({"default_branch": "master"})
            if endpoint == "repos/owner/repository/git/ref/heads/master":
                self.assertIn(token, {"source", "read"}); return completed({"object": {"sha": head}})
            if endpoint.endswith("/contents/.github/workflows/pr-governance.yml?ref=" + head):
                self.assertIn(token, {"source", "read"}); return completed({"sha": "c" * 40})
            if endpoint == "repos/owner/repository/branches/master/protection":
                self.assertEqual(token, "admin"); return completed(state["protection"])
            if endpoint == "repos/owner/repository/branches/master/protection/required_status_checks/contexts":
                self.assertEqual(token, "admin"); method = arguments[arguments.index("--method") + 1]
                self.assertIn(method, {"POST", "DELETE"}); self.assertEqual(json.loads(str(kwargs["input"])), {"contexts": [barrier]})
                records = protection_records(); required = state["protection"]["required_status_checks"]  # type: ignore[index]
                self.assertIsInstance(required, dict); mutations = state["mutations"]; self.assertIsInstance(mutations, list); mutations.append(method)
                if method == "POST" and not any(item["context"] == barrier for item in records):
                    records.append({"context": barrier, "app_id": 4_766_933}); required["contexts"].append(barrier)
                if method == "DELETE":
                    records[:] = [item for item in records if item["context"] != barrier]; required["contexts"][:] = [name for name in required["contexts"] if name != barrier]
                    late_event = state["late_event_without_run_list"]
                    if late_event is not None:
                        runs = state["runs"]; checks = state["manifest_checks"]
                        self.assertIsInstance(late_event, dict); self.assertIsInstance(runs, list); self.assertIsInstance(checks, dict)
                        self.assertNotIn(late_event["id"], [item["id"] for item in runs])
                        self.assertNotIn(barrier, required["contexts"])
                        self.assertTrue(all(item["status"] == "in_progress" and item["conclusion"] is None for item in checks.values()))
                        state["late_event_observed_at_release"] = True
                    if state["uncertain_delete"]: return completed([], 1)
                return completed(required["contexts"])
            if endpoint == "repos/owner/repository/pulls?state=open&per_page=100":
                self.assertEqual(token, "read"); return completed([state["pulls"]])
            if endpoint == "repos/owner/repository/actions/runs/99":
                self.assertEqual(token, "read"); return completed(run)
            if endpoint == "repos/owner/repository/actions/workflows/pr-governance.yml/runs?per_page=100":
                self.assertEqual(token, "read"); return completed([{"workflow_runs": state["runs"]}])
            if endpoint == "repos/owner/repository/check-runs":
                self.assertEqual(token, "marker-write"); return completed({"id": 501, "name": barrier, "head_sha": head, "external_id": f"krr-governance-affected-head-barrier/v1/{head}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4_766_933}})
            if endpoint == "repos/owner/repository/check-runs/501":
                self.assertEqual(token, "marker-read"); return completed({"id": 501, "name": barrier, "head_sha": head, "external_id": f"krr-governance-affected-head-barrier/v1/{head}/scheduler-99", "status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/99?barrier_marker=periodic", "app": {"id": 4_766_933}})
            if endpoint in {"repos/owner/repository/check-runs/801", "repos/owner/repository/check-runs/802"}:
                self.assertEqual(token, "read"); identifier = int(endpoint.rsplit("/", 1)[1]); checks = state["manifest_checks"]
                self.assertIsInstance(checks, dict); return completed(checks[identifier])
            raise AssertionError(arguments)

        def execute(code: str, environment: dict[str, str]) -> int:
            with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                try:
                    exec(code, {"__name__": "__main__"}); return 0
                except SystemExit:
                    return 1

        def outputs(path: Path) -> dict[str, str]:
            return dict(line.split("=", 1) for line in path.read_text(encoding="utf-8").splitlines())

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"
            common = {"GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "PATH": os.environ["PATH"], "DEFAULT_BRANCH": "master", "DEFAULT_HEAD": head, "DISPATCHER_RUN_ID": "99", "CHECK_APP_ID": "4766933", "WORKFLOW_REF": "owner/repository/.github/workflows/pr-governance.yml@refs/heads/master", "WORKFLOW_SHA": head}
            self.assertEqual(execute(source, common | {"GH_TOKEN": "source"}), 0)
            self.assertEqual(execute(marker, common | {"CHECK_WRITE_TOKEN": "marker-write", "CHECK_READ_TOKEN": "marker-read"}), 0)
            activate_env = common | {"ADMIN_TOKEN": "admin", "GITHUB_OUTPUT": str(output)}
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "false"}), 0)
            self.assertEqual(outputs(output)["active"], "false"); self.assertEqual(state["mutations"], [])
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            self.assertEqual(state["mutations"], ["POST"])
            old_success = {"CI / test"}
            self.assertNotEqual({item["context"] for item in protection_records()}, old_success, "atomic POST blocks old success before paced writes")
            recovery = directory / "recovery"
            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "false", "GITHUB_OUTPUT": str(recovery)}), 0)
            self.assertEqual(outputs(recovery)["active"], "true")

            def release_env(targets: str, snapshots: str, manifest_1: str, manifest_2: str = "[]") -> dict[str, str]:
                return common | {"READ_TOKEN": "read", "ADMIN_TOKEN": "admin", "TARGETS": targets, "TARGET_SNAPSHOTS": snapshots, "PRE_MANIFEST_1": manifest_1, "PRE_MANIFEST_2": manifest_2, "TAIL_MANIFEST_1": "[]", "TAIL_MANIFEST_2": "[]", "DUPLICATE_GOVERNED_HEADS": "[]"}

            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]")), 1)
            self.assertEqual(state["mutations"], ["POST"])
            pulls = state["pulls"]; self.assertIsInstance(pulls, list); pulls[0] = {**pulls[0], "head": {"sha": "c" * 40, "repo": {"full_name": "owner/repository"}}}
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["POST"]); pulls[0] = {**pulls[0], "head": {"sha": head, "repo": {"full_name": "owner/repository"}}}
            runs = state["runs"]; self.assertIsInstance(runs, list); runs.append({**run, "id": 100, "created_at": "2026-08-30T00:00:01Z"})
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["POST"]); runs.pop()
            state["uncertain_delete"] = True
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 1)
            self.assertEqual(state["mutations"], ["POST", "DELETE", "POST"]); state["uncertain_delete"] = False
            runs.insert(0, {**run, "id": 98, "status": "completed", "created_at": "2026-08-29T23:59:59Z"})
            state["late_event_without_run_list"] = {**run, "id": 100, "created_at": "2026-08-30T00:00:02Z"}
            self.assertEqual(execute(release, release_env("[72,73]", f'[[72,"{head}",false],[73,"{other}",false]]', "[[72,801]]", "[[73,802]]")), 0)
            self.assertEqual(state["mutations"], ["POST", "DELETE", "POST", "DELETE"]); runs.pop(0)
            self.assertTrue(state["late_event_observed_at_release"])
            manifest_checks = state["manifest_checks"]; self.assertIsInstance(manifest_checks, dict)
            self.assertTrue(all(item["status"] == "in_progress" and item["conclusion"] is None for item in manifest_checks.values()))
            state["late_event_without_run_list"] = None
            self.assertEqual({item["context"] for item in protection_records()}, old_success)

            self.assertEqual(execute(activate, activate_env | {"PRIORITY": "true"}), 0)
            pulls[:] = []
            self.assertEqual(execute(release, release_env("[]", "[]", "[]")), 0, "a later schedule recovers a static barrier even after all PRs close")

    def test_old_writer_generation_cannot_terminalize_current_manifest_check(self) -> None:
        """旧dispatcherのfingerprintはcurrent manifest IDのPATCH前に停止する。"""
        head = "a" * 40
        module_name = "krr_status_writer_barrier_old_generation"
        spec = importlib.util.spec_from_file_location(module_name, ROOT / "scripts/review/pr_governance_status_writer.py")
        self.assertIsNotNone(spec); assert spec is not None and spec.loader is not None
        writer = importlib.util.module_from_spec(spec); sys.modules[module_name] = writer
        try:
            spec.loader.exec_module(writer)
            old_external = f"krr-governance/v1/{head}/dispatcher-98"
            current_external = f"krr-governance/v1/{head}/dispatcher-99"
            old = {"id": 700, "name": writer.CHECK_NAME, "head_sha": head, "external_id": old_external, "updated_at": "2026-08-30T00:00:00Z", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/98?dispatcher_run_id=98&carry_pending=0", "app": {"id": 4_766_933}}
            current = {"id": 801, "name": writer.CHECK_NAME, "head_sha": head, "external_id": current_external, "updated_at": "2026-08-30T00:00:01Z", "status": "in_progress", "conclusion": None, "details_url": "https://github.com/owner/repository/actions/runs/99?dispatcher_run_id=99&carry_pending=0", "app": {"id": 4_766_933}}
            with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "4766933", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "98"}, clear=False):
                old_fingerprint = writer.check_fingerprint(old)
            with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "4766933", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_DISPATCHER_RUN_ID": "99"}, clear=False), \
                 patch.object(writer, "check_run", return_value=current), patch.object(writer, "command") as command:
                with self.assertRaises(writer.NoPostGovernanceError):
                    writer.write_check(head, state="failure", description="old writer", details_url=current["details_url"], existing=current, expected_fingerprint=old_fingerprint)
            self.assertNotEqual(old_fingerprint[-1], current_external)
            command.assert_not_called()
        finally:
            sys.modules.pop(module_name, None)

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
                f"  *'/pulls/72'*) printf '%s' '{{\"draft\":true,\"head\":{{\"sha\":\"{head}\"}}}}' ;;\n"
                "  *) exit 91 ;;\nesac\n", encoding="utf-8",
            ); fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": "[72]",
                "KNOWN_TARGET_SNAPSHOTS": json.dumps([[72, head, True]]),
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
                directory = Path(temporary); posted: dict[int, dict[str, object]] = {}; writes: list[list[str]] = []
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
                numbers = list(range(1, total + 1))
                for chunk_index in range(0, len(numbers), 300):
                    chunk = numbers[chunk_index:chunk_index + 300]
                    output = directory / f"output-{chunk_index}"
                    environment = os.environ | {
                        "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com", "GITHUB_RUN_ID": "9",
                        "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": json.dumps(chunk),
                        "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, f"{number:040x}", False] for number in chunk]),
                        "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"],
                    }
                    with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                        namespace: dict[str, object] = {"__name__": "__main__"}
                        exec(program, namespace)
                    manifest = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())["check_manifest"]
                    self.assertEqual(len(json.loads(manifest)), len(chunk))
                self.assertEqual(len(writes), total)
                self.assertTrue(all("details_url=https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=1" in write for write in writes))

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
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": json.dumps([head]), "AFFECTED": "[72,73,74]",
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
                "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": json.dumps([duplicate_head]), "AFFECTED": "[72,73,74,75]", "EVENT_TARGETS": "[72]",
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
            "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "AFFECTED": "[72,73]",
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
            r"- name: Drain authoritative writer before the next governance hand-off.*?python3 - <<'PY'\n(.*?)\n          PY",
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
            "id": 71, "name": "PR governance status writer", "display_title": "source=99 scope=all segment=1",
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
                    "if '/git/ref/heads/master' in arguments: print(json.dumps({'object': {'sha': 'a' * 40}}))\n"
                    "elif '/actions/runs/99' in arguments: print(os.environ['SOURCE'])\n"
                    "elif 'pulls?state=open' in arguments: print(os.environ['PULLS'])\n"
                    "elif arguments.endswith('repos/owner/repository'): print(json.dumps({'default_branch': 'master'}))\n"
                    "elif '/runs?per_page=100' in arguments:\n"
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
                    "SOURCE": json.dumps({"id": 99, "name": "PR governance dispatcher", "event": "issues", "status": "in_progress", "run_attempt": 1, "head_branch": "master", "head_sha": "a" * 40, "repository": {"full_name": "owner/repository"}}),
                    "PULLS": json.dumps([[{"number": 72, "state": "open", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}}, "draft": False}, {"number": 73, "state": "open", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}}, "draft": False}]]),
                    "WRITER_ALL_OPEN_TARGETS": "[72,73]", "WRITER_ALL_OPEN_SNAPSHOTS": "[[72,\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",false],[73,\"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\",false]]",
                    "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]", "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                    "WRITER_TAIL_CHECK_MANIFEST_1": "[[72,701],[73,702]]", "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]",
                    "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "GITHUB_OUTPUT": str(directory / "output"),
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
                    "  *'check-runs?'*) printf '%s' '[{\"check_runs\":[]}]' ;;\n"
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
                    f"#!/bin/sh\necho \"${{GH_TOKEN}}:$*\" >> '{directory / 'calls.log'}'\ncase \"$*\" in\n"
                    "  *'check-runs/101'*) printf '%s' '{\"id\":101,\"app\":{\"id\":42},\"name\":\"KRR / PR governance (trusted check)\",\"head_sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"external_id\":\"krr-governance/v1/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/dispatcher-9\",\"status\":\"in_progress\",\"conclusion\":null,\"details_url\":\"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending=0\"}' ;;\n"
                    "  *'check-runs?'*) printf '%s' '[{\"check_runs\":[]}]' ;;\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    f"  *'/pulls/'*) printf '%s' '{json.dumps({'number': 1, 'state': 'open', 'draft': False, 'base': {'ref': 'master', 'repo': {'full_name': 'owner/repository'}}, 'head': {'sha': 'a' * 40, 'repo': {'full_name': 'owner/repository'}}}, separators=(',', ':'))}' ;;\n"
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
                    # The invalidator now binds every list/reread to this
                    # explicit read token; the fixture must not accidentally
                    # rely on the ambient process credential.
                    "GH_TOKEN": "read", "CHECK_WRITE_TOKEN": "write", "CHECK_APP_ID": "42", "DUPLICATE_GOVERNED_HEADS": "[]", "PULLS": "[]",
                    "AFFECTED": json.dumps(list(range(1, total + 1))),
                    "KNOWN_TARGET_SNAPSHOTS": json.dumps([[number, "a" * 40, False] for number in range(1, total + 1)]),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run([sys.executable, "-c", program], env=environment, capture_output=True, text=True, check=False)
                if result.returncode != expected:
                    calls = directory / "calls.log"
                    self.fail(result.stdout + result.stderr + (calls.read_text(encoding="utf-8") if calls.exists() else ""))
                self.assertEqual(result.returncode, expected, result.stderr)
                calls = (directory / "calls.log").read_text(encoding="utf-8").splitlines()
                self.assertTrue(calls[0].startswith("read:"))
                self.assertTrue(calls[1].startswith("write:"))
                if expected == 0:
                    self.assertTrue(all(line.startswith("read:") for line in (calls[2], calls[3])))
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

    def test_large_priority_targets_are_resolved_into_complete_ttl_safe_chunks(self) -> None:
        """The resolver and every pre-invalidator cover 41 and 600 current PRs."""
        resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(resolver); assert resolver is not None
        pre_blocks = re.findall(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        # One token pair cannot safely span 600 writes.  The workflow must
        # expose two independently tokenized 300-head pre-invalidation steps.
        self.assertGreaterEqual(len(pre_blocks), 2)
        self.assertIn("terminal_batch_numbers", self.workflow)
        self.assertIn("continuation_index", self.workflow)

        def pull(number: int) -> dict[str, object]:
            head = f"{number:040x}"
            return {
                "number": number,
                "state": "open",
                "body": "Fixes #64",
                "draft": False,
                "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                "head": {"sha": head, "repo": {"full_name": "owner/repository"}},
            }

        for total in (41, 600):
            with self.subTest(total=total), tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary); output = directory / "output"
                pages = [
                    [pull(number) for number in range(start, min(start + 100, total + 1))]
                    for start in range(1, total + 1, 100)
                ]
                fake = directory / "gh"
                fake.write_text(
                    "#!/bin/sh\ncase \"$*\" in\n"
                    "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                    "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                    "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                    "  *) exit 91 ;;\nesac\n",
                    encoding="utf-8",
                )
                fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                    "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                    "EVENT_TARGETS": json.dumps(list(range(1, total + 1)), separators=(",", ":")),
                    "EVENT_PRIORITY_TARGETS": json.dumps(list(range(1, total + 1)), separators=(",", ":")),
                    "GITHUB_OUTPUT": str(output), "PULLS": json.dumps(pages, separators=(",", ":")),
                    "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
                }
                result = subprocess.run(
                    [sys.executable, "-c", self._workflow_program(resolver)],
                    env=environment, capture_output=True, text=True, check=False,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                values = dict(line.split("=", 1) for line in output.read_text(encoding="utf-8").splitlines())
                targets = json.loads(values["targets"])
                priority = json.loads(values["priority_targets"])
                pre_targets = json.loads(values["preinvalidate_targets"])
                self.assertEqual(targets, list(range(1, total + 1)))
                self.assertEqual(pre_targets, targets)
                self.assertEqual(priority, list(range(1, min(total, 40) + 1)))
                chunks = [
                    json.loads(values[f"preinvalidate_chunk_{index}"])
                    for index in (1, 2)
                ]
                self.assertEqual([len(chunk) for chunk in chunks], [min(total, 300), max(0, total - 300)])
                self.assertEqual(chunks[0] + chunks[1], targets)
                snapshots = [
                    json.loads(values[f"preinvalidate_chunk_{index}_snapshots"])
                    for index in (1, 2)
                ]
                self.assertEqual([entry[0] for chunk in snapshots for entry in chunk], targets)
                self.assertTrue(all(len(chunk) <= 300 for chunk in chunks))

    def test_large_preinvalidation_posts_each_distinct_head_once_with_fresh_token_pair(self) -> None:
        """Both 300-head chunks must be executable and never duplicate an external ID."""
        resolver = re.search(
            r"- name: Re-enumerate every current local governance pull request.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertIsNotNone(resolver); assert resolver is not None
        pre_blocks = re.findall(
            r"- name: Pre-invalidate priority event heads.*?python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow, re.DOTALL,
        )
        self.assertGreaterEqual(len(pre_blocks), 2)
        total = 600
        heads = {number: f"{number:040x}" for number in range(1, total + 1)}
        pages = [[
            {
                "number": number, "state": "open", "body": "Fixes #64", "draft": False,
                "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}},
            }
            for number in range(start, min(start + 100, total + 1))
        ] for start in range(1, total + 1, 100)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); resolver_output = directory / "resolver-output"
            fake = directory / "gh"
            fake.write_text(
                "#!/bin/sh\ncase \"$*\" in\n"
                "  *'pulls?state=open'*) printf '%s' \"${PULLS}\" ;;\n"
                "  *'git/ref/heads/master'*) printf '%s' '{\"object\":{\"sha\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"}}' ;;\n"
                "  *'repos/owner/repository'*) printf '%s' '{\"default_branch\":\"master\"}' ;;\n"
                "  *) exit 91 ;;\nesac\n",
                encoding="utf-8",
            )
            fake.chmod(fake.stat().st_mode | stat.S_IXUSR)
            resolver_environment = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master",
                "EVENT_NAME": "issues", "ISSUE_NUMBER": "64", "ISSUE_PULL_REQUEST_URL": "",
                "EVENT_TARGETS": json.dumps(list(heads), separators=(",", ":")),
                "EVENT_PRIORITY_TARGETS": json.dumps(list(heads), separators=(",", ":")),
                "GITHUB_OUTPUT": str(resolver_output), "PULLS": json.dumps(pages, separators=(",", ":")),
                "PATH": f"{directory}{os.pathsep}{os.environ['PATH']}",
            }
            result = subprocess.run(
                [sys.executable, "-c", self._workflow_program(resolver)],
                env=resolver_environment, capture_output=True, text=True, check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            values = dict(line.split("=", 1) for line in resolver_output.read_text(encoding="utf-8").splitlines())
            posts: list[list[str]] = []
            created: dict[int, dict[str, object]] = {}
            read_tokens: set[str] = set()
            write_tokens: set[str] = set()

            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")

            def fake_run(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = arguments[-1]
                supplied = kwargs.get("env")
                if "--method" in arguments and "POST" in arguments:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    write_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(write_token, str)
                    assert isinstance(write_token, str)
                    self.assertRegex(write_token, r"^write-[12]$")
                    write_tokens.add(write_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    posts.append(arguments)
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    identifier = 10_000 + len(posts)
                    check: dict[str, object] = {
                        "id": identifier, "app": {"id": 42},
                        "name": "KRR / PR governance (trusted check)", "head_sha": fields["head_sha"],
                        "external_id": fields["external_id"], "status": "in_progress", "conclusion": None,
                        "details_url": fields["details_url"],
                    }
                    created[identifier] = check
                    return response(check)
                if isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    read_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(read_token, str)
                    assert isinstance(read_token, str)
                    self.assertRegex(read_token, r"^read-[12]$")
                    read_tokens.add(read_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    return response(created[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    self.assertIsInstance(supplied, dict)
                    assert isinstance(supplied, dict)
                    read_token = supplied.get("GH_TOKEN")
                    self.assertIsInstance(read_token, str)
                    assert isinstance(read_token, str)
                    self.assertRegex(read_token, r"^read-[12]$")
                    read_tokens.add(read_token)
                    self.assertEqual(supplied.get("PATH"), os.environ["PATH"])
                    number = int(endpoint.rsplit("/", 1)[1])
                    return response({
                        "number": number, "state": "open", "draft": False,
                        "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                        "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}},
                    })
                raise AssertionError(arguments)

            for index, block in enumerate(pre_blocks[:2], 1):
                chunk = json.loads(values[f"preinvalidate_chunk_{index}"])
                if not chunk:
                    continue
                output = directory / f"pre-{index}-output"
                environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                    "GH_TOKEN": f"read-{index}", "CHECK_WRITE_TOKEN": f"write-{index}", "CHECK_APP_ID": "42",
                    "TARGETS": values[f"preinvalidate_chunk_{index}"],
                    "TARGET_SNAPSHOTS": values[f"preinvalidate_chunk_{index}_snapshots"],
                    "DEFAULT_BRANCH": "master", "DISPATCHER_RUN_ID": "9", "GITHUB_OUTPUT": str(output),
                    "PATH": os.environ["PATH"],
                }
                # The production script must use a read-only token for every
                # reread and a different fresh write token for each chunk.
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    exec(textwrap.dedent(block), {"__name__": "__main__"})
            self.assertEqual(len(posts), total)
            self.assertEqual(len({next(item.split("=", 1)[1] for item in post if item.startswith("external_id=")) for post in posts}), total)
            self.assertEqual(read_tokens, {"read-1", "read-2"})
            self.assertEqual(write_tokens, {"write-1", "write-2"})
            self.assertEqual(
                [next(item.split("=", 1)[1] for item in post if item.startswith("head_sha=")) for post in posts],
                [heads[number] for number in range(1, total + 1)],
            )

    def test_terminal_writer_segments_are_contiguous_bounded_and_fail_closed(self) -> None:
        """A 600-target manifest is partitioned into at most four ordered 150 slices."""
        writer = (ROOT / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        workflow = self.workflow
        for required in (
            "GOVERNANCE_TERMINAL_BATCH_NUMBERS", "GOVERNANCE_CONTINUATION_INDEX",
            "len(terminal_batch) > 150", "continuation_index - 1) * 150",
            "Writer terminal segment boundary is invalid.",
        ):
            self.assertIn(required, writer)
        self.assertGreaterEqual(workflow.lower().count("terminal_batch_numbers"), 2)
        self.assertGreaterEqual(workflow.lower().count("continuation_index"), 2)
        dispatch_match = re.search(
            r"- name: [^\n]*repository-wide governance arbiter segment[^\n]*\n.*?python3 - <<'PY'\n(.*?)\n          PY",
            workflow, re.DOTALL | re.IGNORECASE,
        )
        self.assertIsNotNone(dispatch_match); assert dispatch_match is not None
        self.assertNotIn("KRR_GOVERNANCE_APP_PRIVATE_KEY", dispatch_match.group(1))
        self.assertNotIn("private-key", dispatch_match.group(1).lower())
        # Exercise the production ordering helper so the segment check does
        # not merely bless a hand-written numeric range in this test.
        spec = importlib.util.spec_from_file_location("krr_status_writer", ROOT / "scripts/review/pr_governance_status_writer.py")
        self.assertIsNotNone(spec); assert spec is not None and spec.loader is not None
        module = importlib.util.module_from_spec(spec)
        sys.modules[spec.name] = module
        spec.loader.exec_module(module)
        for total in (41, 600):
            pulls = tuple({"number": number, "isDraft": False} for number in range(1, total + 1))
            snapshot = module.OpenSnapshot(tuple(range(1, total + 1)), {}, pulls)
            order = module.governance_order(snapshot, frozenset(), tuple(range(1, min(total, 40) + 1)))
            early = order[:40]
            tail = order[40:]
            self.assertEqual(early, tuple(range(1, min(total, 40) + 1)))
            self.assertEqual(len(tail), max(0, total - 40))
            self.assertEqual(order, tuple(range(1, total + 1)))
            segments = [order[start:start + 150] for start in range(0, len(order), 150)]
            self.assertEqual([len(segment) for segment in segments], [150] * (total // 150) + ([total % 150] if total % 150 else []))
            self.assertEqual(tuple(number for segment in segments for number in segment), order)
            self.assertTrue(all(len(segment) <= 150 for segment in segments))
            self.assertLessEqual(len(segments), 4)
            if total == 41:
                self.assertEqual(tail, (41,))

    def test_all_writer_dispatches_sequential_terminal_segments_and_stops_after_failure(self) -> None:
        """Each terminal segment has its own token/dispatch/await fail-closed chain."""
        steps = list(re.finditer(
            r"^      - name: (?P<name>[^\n]+)\n(?P<body>.*?)(?=^      - name: |\Z)",
            self.workflow, re.MULTILINE | re.DOTALL,
        ))
        dispatch_steps = [step for step in steps if "terminal_batch_numbers" in step.group("body") and "Dispatch" in step.group("name") and "governance arbiter segment" in step.group("name")]
        await_steps = [step for step in steps if "Await" in step.group("name") and "governance arbiter segment" in step.group("name")]
        self.assertEqual(len(dispatch_steps), 4)
        self.assertEqual(len(await_steps), 4)
        dispatch_indices: list[int] = []
        for position, step in enumerate(dispatch_steps):
            body = step.group("body")
            step_number = steps.index(step)
            self.assertGreater(step_number, 0)
            self.assertIn("actions/create-github-app-token", steps[step_number - 1].group("body"))
            index_match = re.search(r"inputs\[continuation_index\][^\n]*?=([1-4])(?:\"|')?", body)
            self.assertIsNotNone(index_match, step.group("name")); assert index_match is not None
            index = int(index_match.group(1)); dispatch_indices.append(index)
            self.assertEqual(index, position + 1)
            batch_match = re.search(r"inputs\[terminal_batch_numbers\].{0,180}", body)
            self.assertIsNotNone(batch_match)
            self.assertIn("inputs[terminal_order_numbers]", body)
            self.assertIn("inputs[completed_writer_run_ids]", body)
            self.assertNotIn("KRR_GOVERNANCE_APP_PRIVATE_KEY", body)
            # Every segment must be gated by the preceding await, so a failed
            # or NoPost segment cannot enqueue the next writer.
            if position:
                prior_id = re.search(r"\bid:\s*([A-Za-z0-9_-]+)", await_steps[position - 1].group("body"))
                self.assertIsNotNone(prior_id); assert prior_id is not None
                step_if = re.search(r"^\s*if:\s*(.+)$", body, re.MULTILINE)
                self.assertIsNotNone(step_if); assert step_if is not None
                self.assertIn(f"steps.{prior_id.group(1)}.outcome", step_if.group(1))
                self.assertNotIn("always()", step_if.group(1))
        self.assertEqual(dispatch_indices, [1, 2, 3, 4])
        for index, step in enumerate(await_steps, 1):
            body = step.group("body")
            self.assertIn(f"segment={index}", body)
            for required in ("run.get(\"head_sha\")", "run.get(\"status\")", "run.get(\"conclusion\")", "run.get(\"event\")"):
                self.assertIn(required, body)
            self.assertRegex(body, r"run\.get\(\"(?:actor|triggering_actor)\"\)")
            self.assertIn('run.get("status") == "completed"', body)
            self.assertIn('run.get("conclusion") != "success"', body)
            gate = re.search(r"^\s*if:\s*(.+)$", body, re.MULTILINE)
            self.assertIsNotNone(gate); assert gate is not None
            dispatch_id = re.search(r"\bid:\s*([A-Za-z0-9_-]+)", dispatch_steps[index - 1].group("body"))
            self.assertIsNotNone(dispatch_id); assert dispatch_id is not None
            self.assertIn(f"steps.{dispatch_id.group(1)}.outputs", gate.group(1))

        # Execute the dispatch and await blocks against a complete 600-target
        # fixture. This catches missing source/snapshot/carry API reads.
        all_targets = list(range(1, 601)); preserved = all_targets[:40]
        heads = {number: f"{number:040x}" for number in all_targets}
        snapshots = [[number, heads[number], False] for number in all_targets]
        pre_manifest = [[number, 50_000 + number] for number in all_targets]
        batches = [all_targets[40 + start:40 + start + 150] for start in range(0, 560, 150)]
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); output = directory / "output"
            candidates: list[dict[str, object]] = []; dispatches: list[list[str]] = []
            pages = [[
                {"number": number, "state": "open", "body": "Fixes #64", "draft": False,
                 "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
                 "head": {"sha": heads[number], "repo": {"full_name": "owner/repository"}}}
                for number in range(start, min(start + 100, 601))
            ] for start in range(1, 601, 100)]

            def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
                return subprocess.CompletedProcess([], code, json.dumps(value), "")

            def fake_run(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    return response(pages)
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if isinstance(endpoint, str) and "/actions/runs/" in endpoint:
                    identifier = int(endpoint.rsplit("/", 1)[1])
                    if identifier == 9:
                        return response({"id": 9, "name": "PR governance dispatcher", "event": "issues", "status": "in_progress", "run_attempt": 1, "head_branch": "master", "head_sha": "a" * 40, "repository": {"full_name": "owner/repository"}})
                    return response(next(candidate for candidate in candidates if candidate["id"] == identifier))
                if isinstance(endpoint, str) and "actions/workflows/pr-governance-status-writer.yml/runs?" in endpoint:
                    return response([{"workflow_runs": candidates}])
                if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                    dispatches.append(arguments)
                    fields = {item.split("=", 1)[0]: item.split("=", 1)[1] for item in arguments if "=" in item}
                    index = fields.get("inputs[continuation_index]", "")
                    order = json.loads(fields["inputs[terminal_order_numbers]"])
                    completed = json.loads(fields["inputs[completed_writer_run_ids]"])
                    self.assertEqual(order, all_targets[40:])
                    self.assertEqual(completed, [70_000 + prior for prior in range(1, len(dispatches))])
                    candidate = {"id": 70_000 + len(dispatches), "name": "PR governance status writer", "display_title": f"source=9 scope=all segment={index}", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "repository": {"full_name": "owner/repository"}, "head_branch": "master", "head_sha": "a" * 40, "status": "completed", "conclusion": "success", "run_number": len(dispatches), "run_attempt": 1, "actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"}, "triggering_actor": {"login": "katana-rust-pr-governance-hf[bot]", "type": "Bot"}}
                    candidates.append(candidate)
                    return response({})
                raise AssertionError(arguments)

            common = {
                "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                "DISPATCHER_RUN_ID": "9", "WRITER_SCOPE": "all", "WRITER_TARGETS": json.dumps(all_targets, separators=(",", ":")),
                "WRITER_ALL_OPEN_TARGETS": json.dumps(all_targets, separators=(",", ":")), "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(snapshots, separators=(",", ":")),
                "WRITER_PRESERVED_TARGETS": json.dumps(preserved, separators=(",", ":")), "PRESERVED_WRITER_RUN_ID": "71",
                "WRITER_CARRY_TARGET_NUMBERS": "[]", "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[]", "WRITER_PREINVALIDATE_TARGETS": json.dumps(all_targets, separators=(",", ":")),
                "WRITER_PRE_CHECK_MANIFEST_1": json.dumps(pre_manifest[:300], separators=(",", ":")), "WRITER_PRE_CHECK_MANIFEST_2": json.dumps(pre_manifest[300:], separators=(",", ":")),
                "WRITER_TAIL_CHECK_MANIFEST_1": "[]", "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": json.dumps(pre_manifest[:40], separators=(",", ":")),
                "WRITER_TERMINAL_ORDER": json.dumps(all_targets[40:], separators=(",", ":")), "COMPLETED_WRITER_RUN_IDS": "[]",
                "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]", "GITHUB_OUTPUT": str(output), "PATH": os.environ["PATH"],
            }
            for index, step in enumerate(dispatch_steps, 1):
                environment = os.environ | common | {
                    "WRITER_TERMINAL_BATCH_NUMBERS": json.dumps(batches[index - 1], separators=(",", ":")),
                    "WRITER_CONTINUATION_INDEX": str(index), "TERMINAL_BATCH": json.dumps(batches[index - 1], separators=(",", ":")),
                    "CONTINUATION_INDEX": str(index), "WRITER_CHECK_MANIFEST": json.dumps(pre_manifest, separators=(",", ":")),
                    "COMPLETED_WRITER_RUN_IDS": json.dumps([70_000 + prior for prior in range(1, index)], separators=(",", ":")),
                }
                with patch.dict(os.environ, environment, clear=True), patch("subprocess.run", side_effect=fake_run):
                    try:
                        exec(textwrap.dedent(re.search(r"python3 - <<'PY'\n(.*?)\n          PY", step.group("body"), re.DOTALL).group(1)), {"__name__": "__main__"})  # type: ignore[union-attr]
                    except SystemExit as error:
                        self.assertEqual(error.code, 0)
                await_body = await_steps[index - 1].group("body")
                await_program_match = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", await_body, re.DOTALL)
                self.assertIsNotNone(await_program_match); assert await_program_match is not None
                writer_id = str(70_000 + index)
                await_environment = os.environ | {
                    "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                    "WRITER_RUN_ID": writer_id, "DISPATCHER_RUN_ID": "9", "CONTINUATION_INDEX": str(index),
                    "GH_TOKEN": "read", "GITHUB_SERVER_URL": "https://github.com", "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]", "GITHUB_OUTPUT": str(output),
                    "PATH": os.environ["PATH"],
                }
                with patch.dict(os.environ, await_environment, clear=True), patch("subprocess.run", side_effect=fake_run), patch("time.sleep"):
                    try:
                        exec(textwrap.dedent(await_program_match.group(1)), {"__name__": "__main__"})
                    except SystemExit as error:
                        self.assertEqual(error.code, 0)
            self.assertEqual(len(dispatches), 4)
            sent_batches = [json.loads(next(item.split("=", 1)[1] for item in dispatch if item.startswith("inputs[terminal_batch_numbers]="))) for dispatch in dispatches]
            self.assertEqual([number for batch in sent_batches for number in batch], all_targets[40:])
            self.assertTrue(all(len(batch) <= 150 for batch in sent_batches))
            next_segment_if = self._step_if("Dispatch second repository-wide governance arbiter segment")
            success_values = {
                "steps.dispatch-all-1.outputs.has_terminal_batch_2": "true",
                "steps.await-all-1.outcome": "success",
                "steps.await-all-1.outputs.success": "true",
            }
            self.assertTrue(self._github_if(next_segment_if, success_values))
            self.assertEqual(sum(
                next(item.split("=", 1)[1] for item in dispatch if item.startswith("inputs[continuation_index]=")) == "2"
                for dispatch in dispatches
            ), 1)

        # Exercise the hand-off rather than only its individual YAML snippets:
        # the second TTL chunk may be the sole carry source.  The dispatcher
        # has to retain it in the first terminal dispatch, and the actual
        # writer main has to accept precisely that carry-first canonical order.
        first_program_match = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", dispatch_steps[0].group("body"), re.DOTALL)
        self.assertIsNotNone(first_program_match); assert first_program_match is not None
        carry_heads = {1: f"{1:040x}", 301: f"{301:040x}"}
        carry_pulls = [{
            "number": number, "state": "open", "body": "Fixes #64", "draft": False,
            "base": {"sha": "a" * 40, "ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": head, "ref": f"issue/{number}", "repo": {"full_name": "owner/repository"}},
        } for number, head in carry_heads.items()]
        carry_snapshots = [[number, head, False] for number, head in carry_heads.items()]
        carry_manifest = [[1, 91], [301, 92]]
        carried_dispatches: list[list[str]] = []
        registered: list[dict[str, object]] = []

        def response(value: object, code: int = 0) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], code, json.dumps(value), "")

        dispatcher_source_active = {
            "id": 9, "workflow_id": 8, "name": "PR governance dispatcher",
            "path": ".github/workflows/pr-governance.yml@master", "event": "issues",
            "head_branch": "master", "head_sha": "a" * 40,
            "repository": {"full_name": "owner/repository"}, "run_number": 1,
            "run_attempt": 1, "status": "in_progress", "conclusion": None,
            "created_at": "2026-08-30T00:00:00Z",
        }

        def dispatch_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
            endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
            if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                return response({"default_branch": "master"})
            if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                return response({"object": {"sha": "a" * 40}})
            if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                return response(dispatcher_source_active)
            if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                return response([carry_pulls])
            if isinstance(endpoint, str) and "pr-governance-status-writer.yml/runs?" in endpoint:
                return response([{"workflow_runs": registered}])
            if "--method" in arguments and "POST" in arguments and any(isinstance(item, str) and "/dispatches" in item for item in arguments):
                carried_dispatches.append(arguments)
                registered.append({
                    "id": 77, "name": "PR governance status writer",
                    "display_title": "source=9 scope=all segment=1",
                    "path": ".github/workflows/pr-governance-status-writer.yml@master",
                    "event": "workflow_dispatch", "repository": {"full_name": "owner/repository"},
                    "head_branch": "master", "head_sha": "a" * 40, "status": "queued",
                    "run_number": 1, "run_attempt": 1,
                })
                return response({})
            raise AssertionError(arguments)

        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary); handoff_output = directory / "handoff-output"
            handoff_env = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40, "DISPATCHER_RUN_ID": "9",
                "WRITER_SCOPE": "all", "WRITER_TARGETS": "[]",
                "WRITER_ALL_OPEN_TARGETS": "[1,301]",
                "WRITER_ALL_OPEN_SNAPSHOTS": json.dumps(carry_snapshots, separators=(",", ":")),
                "WRITER_PRESERVED_TARGETS": "[]", "PRESERVED_WRITER_RUN_ID": "0",
                "WRITER_PREINVALIDATE_TARGETS": "[]", "WRITER_PRE_CHECK_MANIFEST_1": "[]",
                "WRITER_PRE_CHECK_MANIFEST_2": "[]",
                "WRITER_TAIL_CHECK_MANIFEST_1": json.dumps(carry_manifest, separators=(",", ":")),
                "WRITER_TAIL_CHECK_MANIFEST_2": "[]", "WRITER_PRESERVED_CHECK_MANIFEST": "[]",
                "WRITER_CARRY_TARGET_NUMBERS_1": "[]", "WRITER_CARRY_TARGET_NUMBERS_2": "[301]",
                "GITHUB_OUTPUT": str(handoff_output), "PATH": os.environ["PATH"],
            }
            with patch.dict(os.environ, handoff_env, clear=True), patch("subprocess.run", side_effect=dispatch_transport):
                exec(textwrap.dedent(first_program_match.group(1)), {"__name__": "__main__"})
            self.assertEqual(len(carried_dispatches), 1)
            dispatched_fields = {
                item.split("=", 1)[0]: item.split("=", 1)[1]
                for item in carried_dispatches[0] if isinstance(item, str) and "=" in item
            }
            self.assertEqual(json.loads(dispatched_fields["inputs[terminal_order_numbers]"]), [301, 1])
            self.assertEqual(json.loads(dispatched_fields["inputs[terminal_batch_numbers]"]), [301, 1])
            self.assertEqual(json.loads(dispatched_fields["inputs[completed_writer_run_ids]"]), [])

            # This is intentionally the production writer main, with only
            # GitHub transport and delay mocked.  A contract failure gives a
            # bounded terminal failure path without replacing internal helpers.
            check_runs: dict[int, dict[str, object]] = {}
            for number, identifier in carry_manifest:
                head = carry_heads[number]
                carry = number == 301
                check_runs[identifier] = {
                    "id": identifier, "app": {"id": 42},
                    "name": "KRR / PR governance (trusted check)", "head_sha": head,
                    "external_id": f"krr-governance/v1/{head}/dispatcher-9",
                    "status": "in_progress", "conclusion": None,
                    "details_url": f"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending={int(carry)}",
                    "updated_at": "2026-08-30T00:00:00Z",
                }
            terminal_writes: list[int] = []

            def writer_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                if arguments and arguments[0] == sys.executable:
                    # verify_push_issue is an external contract command. Its
                    # failure exercises the writer's ordinary fail-closed path.
                    return subprocess.CompletedProcess(arguments, 1, "", "")
                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                if isinstance(endpoint, str) and "pulls?state=open" in endpoint:
                    return response([carry_pulls])
                if isinstance(endpoint, str) and "/pulls/" in endpoint:
                    number = int(endpoint.rsplit("/", 1)[1])
                    return response(next(pull for pull in carry_pulls if pull["number"] == number))
                if isinstance(endpoint, str) and endpoint == "repos/owner/repository":
                    return response({"default_branch": "master"})
                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                    return response({"object": {"sha": "a" * 40}})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/9"):
                    return response({**dispatcher_source_active, "status": "completed", "conclusion": "success"})
                if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/77"):
                    return response({
                        "id": 77, "name": "PR governance status writer",
                        "path": ".github/workflows/pr-governance-status-writer.yml@master",
                        "event": "workflow_dispatch", "head_sha": "a" * 40,
                        "repository": {"full_name": "owner/repository"}, "status": "in_progress", "run_attempt": 1,
                    })
                if isinstance(endpoint, str) and "/actions/workflows/8/runs?" in endpoint:
                    return response({"workflow_runs": [{**dispatcher_source_active, "status": "completed", "conclusion": "success"}], "total_count": 1})
                if "--method" in arguments and "PATCH" in arguments and isinstance(endpoint, str) and "/check-runs/" in endpoint:
                    identifier = int(endpoint.rsplit("/", 1)[1]); value = dict(check_runs[identifier])
                    fields = {
                        item.split("=", 1)[0]: item.split("=", 1)[1]
                        for item in arguments if isinstance(item, str) and "=" in item
                    }
                    value.update({"status": "completed", "conclusion": "failure", "details_url": fields["details_url"], "updated_at": f"2026-08-30T00:00:0{len(terminal_writes) + 1}Z"})
                    check_runs[identifier] = value; terminal_writes.append(identifier)
                    return response(value)
                if isinstance(endpoint, str) and endpoint.startswith("repos/owner/repository/check-runs/"):
                    return response(check_runs[int(endpoint.rsplit("/", 1)[1])])
                if isinstance(endpoint, str) and "pr-governance-review-events.yml/runs?" in endpoint:
                    return response([{"workflow_runs": []}])
                if isinstance(endpoint, str) and endpoint.endswith("test-and-build.yml"):
                    return response({"id": 31})
                if isinstance(endpoint, str) and endpoint.endswith("release-preflight.yml"):
                    return response({"id": 32})
                if isinstance(endpoint, str) and "/actions/workflows/" in endpoint and "/runs?" in endpoint:
                    return response([{"workflow_runs": []}])
                raise AssertionError(arguments)

            writer_env = os.environ | {
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "GITHUB_RUN_ID": "77", "GITHUB_REF_NAME": "master", "GITHUB_SHA": "a" * 40,
                "GITHUB_ACTIONS": "true", "GH_TOKEN": "read", "DEFAULT_READ_TOKEN": "default-read",
                "CHECK_WRITE_TOKEN": "write", "KRR_GOVERNANCE_CHECK_APP_ID": "42",
                "KRR_GOVERNANCE_APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                "GOVERNANCE_DISPATCHER_RUN_ID": dispatched_fields["inputs[dispatcher_run_id]"],
                "GOVERNANCE_SCOPE": dispatched_fields["inputs[scope]"],
                "GOVERNANCE_TARGET_NUMBERS": dispatched_fields["inputs[target_numbers]"],
                "GOVERNANCE_PRESERVED_TARGET_NUMBERS": dispatched_fields["inputs[preserved_target_numbers]"],
                "GOVERNANCE_PRESERVED_WRITER_RUN_ID": dispatched_fields["inputs[preserved_writer_run_id]"],
                "GOVERNANCE_CHECK_MANIFEST": dispatched_fields["inputs[check_manifest]"],
                "GOVERNANCE_TERMINAL_BATCH_NUMBERS": dispatched_fields["inputs[terminal_batch_numbers]"],
                "GOVERNANCE_CONTINUATION_INDEX": dispatched_fields["inputs[continuation_index]"],
                "GOVERNANCE_TERMINAL_ORDER_NUMBERS": dispatched_fields["inputs[terminal_order_numbers]"],
                "GOVERNANCE_COMPLETED_WRITER_RUN_IDS": dispatched_fields["inputs[completed_writer_run_ids]"],
                "PATH": os.environ["PATH"],
            }
            module_name = "krr_status_writer_event_handoff"
            with patch.dict(os.environ, writer_env, clear=True), patch("subprocess.run", side_effect=writer_transport), patch("time.sleep"):
                writer_spec = importlib.util.spec_from_file_location(module_name, ROOT / "scripts/review/pr_governance_status_writer.py")
                self.assertIsNotNone(writer_spec); assert writer_spec is not None and writer_spec.loader is not None
                writer_module = importlib.util.module_from_spec(writer_spec); sys.modules[module_name] = writer_module
                try:
                    writer_spec.loader.exec_module(writer_module)
                    self.assertEqual(writer_module.main(), 0)
                finally:
                    sys.modules.pop(module_name, None)
            self.assertEqual(terminal_writes, [92, 91])

            # A non-success first writer, an old writer seeing a newer
            # dispatcher generation, and a default-branch drift are all
            # terminal stop conditions.  The workflow's next segment is only
            # entered after a zero exit, so its dispatch endpoint remains
            # untouched for each failed run.
            for mode in ("failure", "newer-generation", "default-branch-drift"):
                with self.subTest(mode=mode):
                    next_segment_dispatches: list[list[str]] = []
                    if mode == "failure":
                        # The actual first await program rejects a failed
                        # registered writer before the segment-2 step is eligible.
                        failed = dict(registered[0], status="completed", conclusion="failure")
                        await_program = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", await_steps[0].group("body"), re.DOTALL)
                        self.assertIsNotNone(await_program); assert await_program is not None
                        def failed_await_transport(arguments: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
                            endpoint = arguments[-1]
                            if isinstance(endpoint, str) and endpoint.endswith("/actions/runs/77"):
                                return response(failed)
                            raise AssertionError(arguments)
                        await_env = os.environ | {
                            "GITHUB_REPOSITORY": "owner/repository", "DEFAULT_BRANCH": "master", "WRITER_HEAD": "a" * 40,
                            "WRITER_RUN_ID": "77", "DISPATCHER_RUN_ID": "9", "CONTINUATION_INDEX": "1", "GH_TOKEN": "read",
                            "GITHUB_SERVER_URL": "https://github.com", "APP_BOT_LOGIN": "katana-rust-pr-governance-hf[bot]",
                            "GITHUB_OUTPUT": str(directory / "failure-await"), "PATH": os.environ["PATH"],
                        }
                        with patch.dict(os.environ, await_env, clear=True), patch("subprocess.run", side_effect=failed_await_transport), patch("time.sleep"):
                            with self.assertRaises(SystemExit):
                                exec(textwrap.dedent(await_program.group(1)), {"__name__": "__main__"})
                    else:
                        # The production writer itself owns the terminal
                        # barrier; a failed main never makes a continuation
                        # dispatch eligible.
                        def stopping_transport(arguments: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                            if mode == "default-branch-drift":
                                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                                if isinstance(endpoint, str) and endpoint.endswith("/git/ref/heads/master"):
                                    return response({"object": {"sha": "b" * 40}})
                            if mode == "newer-generation":
                                endpoint = next((item for item in arguments if isinstance(item, str) and item.startswith("repos/")), arguments[-1])
                                if isinstance(endpoint, str) and "/actions/workflows/8/runs?" in endpoint:
                                    newer = {**dispatcher_source_active, "id": 10, "created_at": "2026-08-30T00:00:01Z", "status": "queued", "conclusion": None}
                                    return response({"workflow_runs": [{**dispatcher_source_active, "status": "completed", "conclusion": "success"}, newer], "total_count": 2})
                            return writer_transport(arguments, **kwargs)
                        # Reset the mutable Check Run fixture so each mode has
                        # a valid dispatcher-pending baseline.
                        for number, identifier in carry_manifest:
                            head = carry_heads[number]
                            check_runs[identifier].update({"status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/9?dispatcher_run_id=9&carry_pending={int(number == 301)}", "updated_at": "2026-08-30T00:00:00Z"})
                        with patch.dict(os.environ, writer_env, clear=True), patch("subprocess.run", side_effect=stopping_transport), patch("time.sleep"):
                            writer_spec = importlib.util.spec_from_file_location(f"{module_name}_{mode}", ROOT / "scripts/review/pr_governance_status_writer.py")
                            self.assertIsNotNone(writer_spec); assert writer_spec is not None and writer_spec.loader is not None
                            writer_module = importlib.util.module_from_spec(writer_spec); sys.modules[writer_spec.name] = writer_module
                            try:
                                writer_spec.loader.exec_module(writer_module)
                                self.assertEqual(writer_module.main(), 1)
                            finally:
                                sys.modules.pop(writer_spec.name, None)
                    failed_values = {
                        "steps.dispatch-all-1.outputs.has_terminal_batch_2": "true",
                        "steps.await-all-1.outcome": "failure",
                        "steps.await-all-1.outputs.success": "false",
                    }
                    # The values are the actual failed preceding result; use
                    # the workflow expression itself, never a local proxy
                    # condition, to decide whether the next dispatch runs.
                    self.assertFalse(self._github_if(next_segment_if, failed_values))
                    if self._github_if(next_segment_if, failed_values):
                        exec(textwrap.dedent(re.search(r"python3 - <<'PY'\n(.*?)\n          PY", dispatch_steps[1].group("body"), re.DOTALL).group(1)), {"__name__": "__main__"})  # pragma: no cover
                    self.assertEqual(next_segment_dispatches, [])


if __name__ == "__main__":
    unittest.main()
