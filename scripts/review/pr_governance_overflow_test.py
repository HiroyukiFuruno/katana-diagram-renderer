from __future__ import annotations

import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path
from textwrap import dedent


class PrGovernanceConflictTest(unittest.TestCase):
    """Executable contracts for canonical-Issue conflict invalidation."""

    def setUp(self) -> None:
        self.workflow = (
            Path(__file__).parents[2] / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        match = re.search(
            r"- name: Resolve PR targets from a trusted event.*?"
            r"python3 - <<'PY'\n(.*?)\n          PY",
            self.workflow,
            re.DOTALL,
        )
        self.assertIsNotNone(match)
        assert match is not None
        self.resolver = dedent(match.group(1))

    def conflict_publisher(self) -> str:
        start = self.workflow.index(
            "- name: Publish canonical-Issue conflict failures to current trusted pull request heads"
        )
        match = re.search(r"python3 - <<'PY'\n(.*?)\n          PY", self.workflow[start:], re.DOTALL)
        self.assertIsNotNone(match)
        assert match is not None
        return dedent(match.group(1))

    def run_resolver(self, pages: list[list[dict]]) -> tuple[subprocess.CompletedProcess[str], str, str]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            bin_directory = directory / "bin"
            bin_directory.mkdir()
            output = directory / "output"
            fake_gh = bin_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "case \"$*\" in\n"
                "  */issues/64*) printf '%s\\n' '{\"number\":64,\"updated_at\":\"2026-08-29T00:00:00Z\"}' ;;\n"
                "  */issues/comments/3*) printf '%s\\n' '{\"id\":3,\"created_at\":\"2026-08-29T00:00:00Z\",\"updated_at\":\"2026-08-29T00:00:00Z\",\"issue_url\":\"https://api.github.com/repos/owner/repository/issues/64\"}' ;;\n"
                "  *'/pulls?state=open&per_page=100'*) printf '%s' \"$FAKE_PAGES\" ;;\n"
                "  *) exit 97 ;;\n"
                "esac\n",
                encoding="utf-8",
            )
            fake_gh.chmod(0o755)
            environment = os.environ | {
                "PATH": f"{bin_directory}:{os.environ['PATH']}",
                "FAKE_PAGES": json.dumps(pages, separators=(",", ":")),
                "GITHUB_OUTPUT": str(output),
                "GITHUB_REPOSITORY": "owner/repository",
                "GITHUB_RUN_ID": "900",
                "EVENT_NAME": "issue_comment", "EVENT_ACTION": "created",
                "ISSUE_NUMBER": "64", "ISSUE_UPDATED_AT": "2026-08-29T00:00:00Z",
                "ISSUE_PULL_REQUEST_URL": "", "COMMENT_ID": "3",
                "COMMENT_CREATED_AT": "2026-08-29T00:00:00Z",
                "COMMENT_UPDATED_AT": "2026-08-29T00:00:00Z",
            }
            result = subprocess.run(
                ["python3", "-c", self.resolver],
                capture_output=True, text=True, env=environment, check=False,
            )
            return result, output.read_text(encoding="utf-8") if output.exists() else "", result.stderr

    @staticmethod
    def pull_requests(count: int, start: int = 1) -> list[dict]:
        return [
            {
                "number": number,
                "state": "open",
                "draft": False,
                "body": "Fixes #64",
                "head": {"sha": f"{number:040x}"},
            }
            for number in range(start, start + count)
        ]

    def test_open_pull_request_pagination_reaches_a_late_target(self) -> None:
        result, output, stderr = self.run_resolver([
            self.pull_requests(100),
            self.pull_requests(1, 101),
        ])
        self.assertEqual(result.returncode, 0, stderr)
        conflicts = json.loads(
            next(line[len("conflict_matrix="):] for line in output.splitlines() if line.startswith("conflict_matrix="))
        )
        self.assertIn(
            "101",
            [target["pr_number"] for target in json.loads(conflicts["include"][0]["targets"])],
        )

    def test_open_pull_request_duplicate_across_pages_is_rejected(self) -> None:
        result, _, stderr = self.run_resolver([
            self.pull_requests(1),
            self.pull_requests(1),
        ])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("duplicate pull request", stderr)

    def test_open_pull_request_malformed_page_is_rejected(self) -> None:
        result, _, stderr = self.run_resolver([[{"number": 1, "state": "open", "draft": False, "body": 42}]])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid body", stderr)

    def test_256_targets_sharing_one_issue_use_one_conflict_matrix_entry(self) -> None:
        result, output, stderr = self.run_resolver([self.pull_requests(256)])
        self.assertEqual(result.returncode, 0, stderr)
        matrix = json.loads(next(line[7:] for line in output.splitlines() if line.startswith("matrix=")))
        conflicts = json.loads(
            next(line[len("conflict_matrix="):] for line in output.splitlines() if line.startswith("conflict_matrix="))
        )
        self.assertEqual(matrix["include"], [])
        self.assertEqual(len(conflicts["include"]), 1)
        self.assertEqual(len(json.loads(conflicts["include"][0]["targets"])), 256)
        self.assertIn("has_conflicts=true", output)

    def test_conflicts_have_a_trusted_failure_fanout_and_continue_after_post_failure(self) -> None:
        self.assertIn("invalidate-conflicts:", self.workflow)
        self.assertRegex(self.workflow, r"257|MAX_MATRIX_TARGETS")
        self.assertIn("failures += 1", self.workflow)
        self.assertRegex(
            self.workflow,
            r"state=failure|failure.*(?:status|statuses)|(?:status|statuses).*failure",
            msg="conflict invalidation must publish trusted failure statuses",
        )

    def run_conflict_publisher(self, numbers: list[int], fail_number: int | None = None, missing_number: int | None = None) -> tuple[subprocess.CompletedProcess[str], list[str]]:
        with tempfile.TemporaryDirectory() as temporary_directory:
            directory = Path(temporary_directory)
            bin_directory = directory / "bin"
            bin_directory.mkdir()
            log = directory / "posts.log"
            fake_gh = bin_directory / "gh"
            fake_gh.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "args = sys.argv[1:]\n"
                "url = next((value for value in args if value.startswith('repos/')), '')\n"
                "if '/pulls?state=open' in url:\n"
                "    print(os.environ['FAKE_OPEN_PRS'])\n"
                "elif '/pulls/' in url:\n"
                "    number = int(url.rsplit('/pulls/', 1)[1])\n"
                "    sha = f'{number:040x}'\n"
                "    print(json.dumps({'number': number, 'state': 'open', 'draft': False, 'body': 'Fixes #64', 'base': {'repo': {'full_name': 'owner/repository'}}, 'head': {'repo': {'full_name': 'owner/repository'}, 'sha': sha}}))\n"
                "elif '--method' in args and 'POST' in args:\n"
                "    with open(os.environ['FAKE_LOG'], 'a', encoding='utf-8') as stream: stream.write(url + '\\n')\n"
                "    raise SystemExit(1 if os.environ.get('FAIL_NUMBER') and url.endswith(os.environ['FAIL_NUMBER']) else 0)\n"
                "else:\n"
                "    raise SystemExit(97)\n",
                encoding="utf-8",
            )
            fake_gh.chmod(0o755)
            listed_numbers = [number for number in numbers if number != missing_number]
            pages = [
                [{"number": number, "state": "open", "draft": False, "body": "Fixes #64", "head": {"sha": f"{number:040x}"}} for number in listed_numbers[:100]],
                [{"number": number, "state": "open", "draft": False, "body": "Fixes #64", "head": {"sha": f"{number:040x}"}} for number in listed_numbers[100:200]],
                [{"number": number, "state": "open", "draft": False, "body": "Fixes #64", "head": {"sha": f"{number:040x}"}} for number in listed_numbers[200:]],
            ]
            environment = os.environ | {
                "PATH": f"{bin_directory}:{os.environ['PATH']}",
                "FAKE_OPEN_PRS": json.dumps(pages), "FAKE_LOG": str(log),
                "CONFLICT_TARGETS": json.dumps([
                    {"pr_number": str(number), "canonical_issue_number": "64", "preflight_head_sha": f"{number:040x}"}
                    for number in numbers
                ]),
                "CANONICAL_ISSUE_NUMBER": "64",
                "GITHUB_REPOSITORY": "owner/repository", "GITHUB_SERVER_URL": "https://github.com",
                "GITHUB_RUN_ID": "900",
            }
            if fail_number is not None:
                environment["FAIL_NUMBER"] = f"{fail_number:040x}"
            result = subprocess.run(
                ["python3", "-c", self.conflict_publisher()],
                capture_output=True, text=True, env=environment, check=False,
            )
            posted = log.read_text(encoding="utf-8").splitlines() if log.exists() else []
            return result, posted

    def test_conflict_posts_a_failure_to_every_current_head(self) -> None:
        result, posted = self.run_conflict_publisher([1, 2, 3])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(len(posted), 3)

    def test_conflict_continues_after_one_post_failure_and_fails_the_job(self) -> None:
        result, posted = self.run_conflict_publisher([1, 2, 3], fail_number=2)
        self.assertNotEqual(result.returncode, 0)
        self.assertGreaterEqual(len(posted), 5)
        self.assertEqual(len(set(posted)), 3)

    def test_conflict_skips_a_closed_or_missing_target_and_invalidates_every_remaining_head(self) -> None:
        result, posted = self.run_conflict_publisher([1, 2, 3], missing_number=2)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(posted), 3)
        self.assertIn(f"repos/owner/repository/statuses/{2:040x}", posted)


if __name__ == "__main__":
    unittest.main()
