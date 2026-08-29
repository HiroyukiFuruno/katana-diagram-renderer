from __future__ import annotations

import os
import stat
import subprocess
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("pre-push.sh")


class PrePushDispatcherTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.repository = self.root / "repository"
        self.bin_directory = self.root / "bin"
        self.log = self.root / "order.log"
        self.repository.mkdir()
        self.bin_directory.mkdir()
        subprocess.run(
            ["git", "init", "--initial-branch=master"],
            cwd=self.repository,
            check=True,
            capture_output=True,
            text=True,
        )
        self.write_executable(
            "just",
            '#!/bin/sh\nprintf "check\\n" >> "$ORDER_LOG"\nexit "${JUST_EXIT:-0}"\n',
        )
        self.write_executable(
            "python3",
            '#!/bin/sh\nprintf "issue\\n" >> "$ORDER_LOG"\nexit 0\n',
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_executable(self, name: str, content: str) -> None:
        path = self.bin_directory / name
        path.write_text(content, encoding="utf-8")
        path.chmod(path.stat().st_mode | stat.S_IXUSR)

    def run_dispatcher(self, *, just_exit: int = 0) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["PATH"] = f"{self.bin_directory}:{environment['PATH']}"
        environment["ORDER_LOG"] = str(self.log)
        environment["JUST_EXIT"] = str(just_exit)
        return subprocess.run(
            ["bash", str(SCRIPT)],
            cwd=self.repository,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_repository_check_runs_before_issue_contract(self) -> None:
        result = self.run_dispatcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.log.read_text(encoding="utf-8").splitlines(), ["check", "issue"])

    def test_issue_contract_does_not_run_when_repository_check_fails(self) -> None:
        result = self.run_dispatcher(just_exit=19)
        self.assertEqual(result.returncode, 19)
        self.assertEqual(self.log.read_text(encoding="utf-8").splitlines(), ["check"])


if __name__ == "__main__":
    unittest.main()
