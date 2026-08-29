from __future__ import annotations

import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))

import verify_push_issue as subject


class VerifyPushIssueTest(unittest.TestCase):
    def issue(
        self,
        number: int = 64,
        *,
        state: str = "OPEN",
        body: str = "Issue body",
    ) -> subject.Issue:
        return subject.Issue(
            number=number,
            state=state,
            body=body,
            url=f"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/{number}",
        )

    def validate(
        self,
        *,
        branch: str = "feature/contract",
        messages: list[str] | None = None,
        changed_paths: list[str] | None = None,
        issue: subject.Issue | None = None,
    ) -> None:
        selected_issue = issue or self.issue()
        subject.validate_contract(
            branch=branch,
            default_branch="master",
            repository="HiroyukiFuruno/katana-render-runtime",
            commit_messages=messages or ["feat: add contract\n\nRefs #64"],
            changed_paths=changed_paths or ["scripts/hooks/pre-push.sh"],
            issue_loader=lambda number: selected_issue
            if number == selected_issue.number
            else None,
        )

    def test_default_branch_does_not_require_an_issue_reference(self) -> None:
        self.validate(branch="master", messages=["chore: direct maintenance"])

    def test_non_default_commit_requires_an_issue_reference(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(messages=["feat: missing issue"])

    def test_foreign_repository_issue_does_not_satisfy_the_contract(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
            self.validate(
                messages=[
                    "feat: wrong issue\n\n"
                    "Refs https://github.com/example/other/issues/64"
                ]
            )

    def test_referenced_issue_must_be_open(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "OPEN"):
            self.validate(issue=self.issue(state="CLOSED"))

    def test_lockfile_only_transitive_update_still_requires_dependency_evidence(self) -> None:
        with self.assertRaisesRegex(subject.ContractViolation, "依存更新証跡"):
            self.validate(changed_paths=["Cargo.lock"])

    def test_dependency_issue_requires_all_evidence_fields(self) -> None:
        body = """## 依存更新証跡
- 上流公開版: serde 2.0.0
- API移行: 互換変更なし
- 依存manifest: Cargo.toml
- lockfile: Cargo.lock
"""
        with self.assertRaisesRegex(subject.ContractViolation, "検証証跡"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_dependency_issue_must_name_changed_contract_files(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0
- API migration: no migration required
- Dependency manifests: package.json
- Lockfiles: bun.lock
- Verification: just check passed
"""
        with self.assertRaisesRegex(subject.ContractViolation, "Cargo.toml"):
            self.validate(
                changed_paths=["Cargo.toml", "Cargo.lock"],
                issue=self.issue(body=body),
            )

    def test_complete_dependency_evidence_satisfies_the_contract(self) -> None:
        body = """## 依存更新証跡
- 上流公開版: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API移行: 互換変更のため移行不要
- 依存manifest: Cargo.toml
- lockfile: Cargo.lock
- 検証証跡: just check 成功
"""
        self.validate(
            changed_paths=["Cargo.toml", "Cargo.lock"],
            issue=self.issue(body=body),
        )

    def test_lockfile_only_transitive_update_accepts_complete_evidence(self) -> None:
        body = """## Dependency Update Evidence
- Upstream release: serde 2.0.0 https://crates.io/crates/serde/2.0.0
- API migration: no migration required
- Dependency manifest: Cargo.toml
- Lockfiles: Cargo.lock
- Verification: just check passed
"""
        self.validate(
            changed_paths=["Cargo.lock"],
            issue=self.issue(body=body),
        )

    def test_new_branch_without_upstream_uses_origin(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
            subprocess.run(
                ["git", "init", "--initial-branch=master"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
                ],
                cwd=repository,
                check=True,
            )
            self.assertEqual(subject.branch_remote(repository, "feature/new"), "origin")

    def test_cli_validates_the_first_push_of_a_new_branch(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()
            commands = [
                ["git", "init", "--initial-branch=master"],
                ["git", "config", "user.name", "Issue Contract Test"],
                ["git", "config", "user.email", "issue@example.com"],
            ]
            for command in commands:
                subprocess.run(
                    command,
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "base.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "initial"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                [
                    "git",
                    "remote",
                    "add",
                    "origin",
                    "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
                ],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "update-ref", "refs/remotes/origin/master", "HEAD"],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "symbolic-ref",
                    "refs/remotes/origin/HEAD",
                    "refs/remotes/origin/master",
                ],
                cwd=repository,
                check=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "feature/issue-contract"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "feature.txt").write_text("feature\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "feature.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: contract", "-m", "Refs #64"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                env=environment,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)


if __name__ == "__main__":
    unittest.main()
