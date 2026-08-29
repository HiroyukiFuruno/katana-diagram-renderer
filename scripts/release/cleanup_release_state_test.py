from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import cleanup_release_state as subject


class CleanupReleaseStateTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.remote = self.root / "remote.git"
        self.repository = self.root / "repository"
        self.git("init", "--bare", "--initial-branch=master", str(self.remote), cwd=self.root)
        self.git("init", "--initial-branch=master", str(self.repository), cwd=self.root)
        self.git("config", "user.name", "Cleanup Test", cwd=self.repository)
        self.git("config", "user.email", "cleanup@example.com", cwd=self.repository)
        (self.repository / "README.md").write_text("base\n", encoding="utf-8")
        self.git("add", "README.md", cwd=self.repository)
        self.git("commit", "-m", "initial", cwd=self.repository)
        self.git("remote", "add", "origin", str(self.remote), cwd=self.repository)
        self.git("push", "-u", "origin", "master", cwd=self.repository)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def git(self, *arguments: str, cwd: Path) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["git", *arguments],
            cwd=cwd,
            check=True,
            capture_output=True,
            text=True,
        )

    def create_release_branch(self, *, merge: bool) -> None:
        self.git("switch", "-c", "release/v9.9.9", cwd=self.repository)
        (self.repository / "release.txt").write_text("release\n", encoding="utf-8")
        self.git("add", "release.txt", cwd=self.repository)
        self.git("commit", "-m", "release", cwd=self.repository)
        self.git("push", "-u", "origin", "release/v9.9.9", cwd=self.repository)
        self.git("switch", "master", cwd=self.repository)
        if merge:
            self.git(
                "merge",
                "--no-ff",
                "release/v9.9.9",
                "-m",
                "merge release",
                cwd=self.repository,
            )
            self.git("push", "origin", "master", cwd=self.repository)

    def cleanup(self, *, published: bool = True) -> list[str]:
        return subject.cleanup_release_state(
            repository=self.repository,
            version="v9.9.9",
            release_branch="release/v9.9.9",
            remote="origin",
            default_branch="master",
            release_checker=lambda _version: published,
        )

    def remote_branch_exists(self, branch: str) -> bool:
        result = subprocess.run(
            ["git", "ls-remote", "--exit-code", "--heads", "origin", branch],
            cwd=self.repository,
            capture_output=True,
            text=True,
            check=False,
        )
        return result.returncode == 0

    def test_refuses_cleanup_before_public_release(self) -> None:
        self.create_release_branch(merge=True)
        with self.assertRaisesRegex(subject.CleanupError, "公開"):
            self.cleanup(published=False)
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_switches_to_default_and_deletes_merged_local_and_remote_branch(self) -> None:
        self.create_release_branch(merge=True)
        self.git("switch", "release/v9.9.9", cwd=self.repository)
        actions = self.cleanup()
        current = self.git("branch", "--show-current", cwd=self.repository).stdout.strip()
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertEqual(current, "master")
        self.assertEqual(local.strip(), "")
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))
        self.assertIn("pulled origin/master with --ff-only", actions)
        self.assertIn("remote branch release/v9.9.9 deleted", actions)

    def test_retains_unmerged_branch(self) -> None:
        self.create_release_branch(merge=False)
        with self.assertRaisesRegex(subject.CleanupError, "未統合"):
            self.cleanup()
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))
        local = self.git("branch", "--list", "release/v9.9.9", cwd=self.repository).stdout
        self.assertNotEqual(local.strip(), "")

    def test_retains_dirty_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        (worktree / "dirty.txt").write_text("dirty\n", encoding="utf-8")
        with self.assertRaisesRegex(subject.CleanupError, "dirty"):
            self.cleanup()
        self.assertTrue(worktree.exists())
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_removes_clean_merged_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        actions = self.cleanup()
        self.assertFalse(worktree.exists())
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))
        self.assertTrue(any(action.startswith("worktree ") for action in actions))

    def test_retains_locked_linked_worktree(self) -> None:
        self.create_release_branch(merge=True)
        worktree = self.root / "release-worktree"
        self.git("worktree", "add", str(worktree), "release/v9.9.9", cwd=self.repository)
        self.git("worktree", "lock", str(worktree), cwd=self.repository)
        with self.assertRaisesRegex(subject.CleanupError, "locked"):
            self.cleanup()
        self.assertTrue(worktree.exists())
        self.assertTrue(self.remote_branch_exists("release/v9.9.9"))

    def test_fetches_release_branch_when_remote_refspec_is_narrow(self) -> None:
        self.create_release_branch(merge=True)
        self.git("config", "--unset-all", "remote.origin.fetch", cwd=self.repository)
        self.git(
            "config",
            "--add",
            "remote.origin.fetch",
            "+refs/heads/master:refs/remotes/origin/master",
            cwd=self.repository,
        )
        self.git(
            "update-ref",
            "-d",
            "refs/remotes/origin/release/v9.9.9",
            cwd=self.repository,
        )
        self.cleanup()
        self.assertFalse(self.remote_branch_exists("release/v9.9.9"))

    def test_rejects_default_branch_as_cleanup_target(self) -> None:
        with self.assertRaisesRegex(subject.CleanupError, "default branch"):
            subject.cleanup_release_state(
                repository=self.repository,
                version="v9.9.9",
                release_branch="master",
                remote="origin",
                default_branch="master",
                release_checker=lambda _version: True,
            )


if __name__ == "__main__":
    unittest.main()
