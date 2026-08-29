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


class PrGovernanceThreadEventTest(unittest.TestCase):
    def setUp(self) -> None:
        repository = Path(__file__).parents[2]
        self.sensor = (
            repository / ".github/workflows/pr-governance-review-events.yml"
        ).read_text(encoding="utf-8")
        self.publisher = (
            repository / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        self.readiness = (
            repository / "scripts/review/verify_pr_ready.py"
        ).read_text(encoding="utf-8")

    def run_sensor_resolver(
        self, run: object, default_blob: object, sensor_blob: object
    ) -> tuple[subprocess.CompletedProcess[str], dict[str, str]]:
        match = re.search(
            r"- name: Resolve PR targets from a trusted event.*?"
            r"          python3 - <<'PY'\n(.*?)\n          PY",
            self.publisher,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_path = Path(temporary_directory)
            fake_gh = temporary_path / "gh"
            response_paths = {
                "RUN": temporary_path / "run.json",
                "DEFAULT": temporary_path / "default.json",
                "SENSOR": temporary_path / "sensor.json",
                "REPOSITORY": temporary_path / "repository.json",
            }
            response_paths["RUN"].write_text(json.dumps(run), encoding="utf-8")
            response_paths["DEFAULT"].write_text(
                json.dumps(default_blob), encoding="utf-8"
            )
            response_paths["SENSOR"].write_text(
                json.dumps(sensor_blob), encoding="utf-8"
            )
            response_paths["REPOSITORY"].write_text(
                json.dumps({"default_branch": "master"}), encoding="utf-8"
            )
            fake_gh.write_text(
                "#!/bin/sh\n"
                "case \"$*\" in\n"
                "  */actions/runs/*) cat \"${FAKE_RUN}\" ;;\n"
                "  *'contents/.github/workflows/pr-governance-review-events.yml?ref=master'*) "
                "cat \"${FAKE_DEFAULT}\" ;;\n"
                "  *'contents/.github/workflows/pr-governance-review-events.yml?ref='*) "
                "cat \"${FAKE_SENSOR}\" ;;\n"
                "  'api repos/owner/repository') cat \"${FAKE_REPOSITORY}\" ;;\n"
                "  *) exit 91 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            output = temporary_path / "github-output"
            environment = os.environ.copy()
            environment.update(
                {
                    "EVENT_NAME": "workflow_run",
                    "EVENT_ACTION": "requested",
                    "WORKFLOW_RUN_ID": "731",
                    "GITHUB_REPOSITORY": "owner/repository",
                    "GITHUB_OUTPUT": str(output),
                    "FAKE_RUN": str(response_paths["RUN"]),
                    "FAKE_DEFAULT": str(response_paths["DEFAULT"]),
                    "FAKE_SENSOR": str(response_paths["SENSOR"]),
                    "FAKE_REPOSITORY": str(response_paths["REPOSITORY"]),
                    "PATH": f"{temporary_directory}{os.pathsep}{environment['PATH']}",
                }
            )
            result = subprocess.run(
                [sys.executable, "-c", dedent(match.group(1))],
                capture_output=True,
                text=True,
                env=environment,
                check=False,
            )
            outputs: dict[str, str] = {}
            if output.exists():
                for line in output.read_text(encoding="utf-8").splitlines():
                    key, separator, value = line.partition("=")
                    if separator:
                        outputs[key] = value
            return result, outputs

    def test_unsupported_thread_webhook_is_not_claimed_as_an_actions_trigger(
        self,
    ) -> None:
        self.assertNotIn("  pull_request_review_thread:\n", self.sensor)
        self.assertNotIn("  pull_request_review_thread:\n", self.publisher)
        self.assertIn("actions: read", self.sensor)
        self.assertIn("statuses: read", self.sensor)
        self.assertNotIn("statuses: write", self.sensor)
        self.assertNotIn("pull-requests: write", self.sensor)
        self.assertNotIn("actions/checkout", self.sensor)
        self.assertNotIn("${{ secrets.", self.sensor)

    def test_default_branch_schedule_revalidates_every_ready_open_pr(self) -> None:
        self.assertIn("  schedule:\n", self.publisher)
        self.assertRegex(self.publisher, r"    - cron: ['\"](?:[^'\"]+)['\"]")
        self.assertIn('elif event_name == "schedule":', self.publisher)
        self.assertIn(
            'f"repos/{repository}/pulls?state=open&per_page=100"', self.publisher
        )
        self.assertIn('"--paginate",', self.publisher)
        self.assertIn('"--slurp",', self.publisher)
        self.assertIn("Scheduled open pull request response is invalid.", self.publisher)
        self.assertIn(
            "Scheduled open pull request response contains a non-open pull request.",
            self.publisher,
        )
        self.assertIn("if not draft:", self.publisher)

    def test_sensor_workflow_run_keeps_the_existing_strict_server_binding(
        self,
    ) -> None:
        workflow_start = self.publisher.index("  workflow_run:\n")
        workflow_end = self.publisher.index("\npermissions:\n", workflow_start)
        workflow_run = self.publisher[workflow_start:workflow_end]
        self.assertIn("      - PR governance review sensor\n", workflow_run)
        self.assertIn("      - requested\n", workflow_run)
        self.assertIn('run_name == "PR governance review sensor"', self.publisher)
        self.assertIn('run_event not in {', self.publisher)
        self.assertIn('"pull_request_review_comment",', self.publisher)
        self.assertIn("not sensor_workflow_path(run_path)", self.publisher)
        self.assertIn('run_repository.get("full_name") != repository', self.publisher)
        self.assertIn('pull_number(trusted_run.get("pull_requests"))', self.publisher)

    def test_sensor_workflow_run_requires_the_current_default_branch_blob(
        self,
    ) -> None:
        sensor_sha = "a" * 40
        run = {
            "name": "PR governance review sensor",
            "event": "pull_request_review",
            "path": ".github/workflows/pr-governance-review-events.yml",
            "head_sha": "b" * 40,
            "repository": {"full_name": "owner/repository"},
            "pull_requests": [{"number": 72}],
        }
        result, outputs = self.run_sensor_resolver(
            run,
            {"sha": sensor_sha, "type": "file"},
            {"sha": sensor_sha, "type": "file"},
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(outputs["matrix"])["include"][0]["pr_number"], "72")

        result, _ = self.run_sensor_resolver(
            run,
            {"sha": sensor_sha, "type": "file"},
            {"sha": "c" * 40, "type": "file"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from the trusted default branch", result.stderr)

        result, _ = self.run_sensor_resolver(
            run,
            {"sha": sensor_sha, "type": "file"},
            {"sha": True, "type": "file"},
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("Trusted workflow blob response is invalid", result.stderr)

    def test_thread_state_revalidation_remains_paginated_and_fail_closed(self) -> None:
        self.assertIn("reviewThreads(first: 100", self.readiness)
        self.assertIn("hasNextPage", self.readiness)
        self.assertIn("reviewThreads endCursor must be a string", self.readiness)
        self.assertIn("thread comments are truncated", self.readiness)
        self.assertIn("未resolve review thread", self.readiness)


if __name__ == "__main__":  # pragma: no cover
    unittest.main()
