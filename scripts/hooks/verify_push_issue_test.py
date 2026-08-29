from __future__ import annotations

import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

import verify_push_issue as subject


class VerifyPushIssueTest(unittest.TestCase):
    def test_parse_push_updates_rejects_nonempty_malformed_lines(self) -> None:
        with self.assertRaises(subject.ContractViolation):
            subject.parse_push_updates("refs/heads/topic deadbeef\n")

    def test_parse_push_updates_accepts_whitespace_only_input_as_empty(self) -> None:
        self.assertEqual(subject.parse_push_updates(" \n\t\n"), ())

    def test_parse_push_updates_rejects_non_forty_hex_sha(self) -> None:
        with self.assertRaises(subject.ContractViolation):
            subject.parse_push_updates(
                f"refs/heads/topic {'0123456789abcdef0123456789abcdef0123456g'} "
                f"refs/heads/topic {'0' * 40}\n"
            )

    def test_parse_push_updates_rejects_invalid_local_or_remote_refs(self) -> None:
        for local_ref, remote_ref in (
            ("topic", "refs/heads/topic"),
            ("refs/heads/topic", "refs/heads/"),
            ("refs/heads/topic space", "refs/heads/topic"),
        ):
            with self.subTest(local_ref=local_ref, remote_ref=remote_ref):
                with self.assertRaises(subject.ContractViolation):
                    subject.parse_push_updates(
                        f"{local_ref} {'1' * 40} {remote_ref} {'2' * 40}\n"
                    )

    def test_parse_push_updates_accepts_delete_marker_and_skips_deleted_branch(self) -> None:
        updates = subject.parse_push_updates(
            f"(delete) {'0' * 40} refs/heads/obsolete {'1' * 40}\n"
        )
        self.assertEqual(
            subject.pushed_branch_updates(updates, default_branch="master"),
            (),
        )

    def test_parse_push_updates_accepts_revision_expression_as_local_ref(self) -> None:
        local_sha = "1" * 40
        remote_sha = "0" * 40
        self.assertEqual(
            subject.parse_push_updates(
                f"HEAD~ {local_sha} refs/heads/topic {remote_sha}\n"
            ),
            (("HEAD~", local_sha, "refs/heads/topic", remote_sha),),
        )

    def test_parse_push_updates_accepts_object_id_and_revspec_local_refs(self) -> None:
        local_sha = "1" * 40
        remote_sha = "0" * 40
        updates = subject.parse_push_updates(
            "\n".join(
                (
                    f"{'a' * 40} {local_sha} refs/heads/topic {remote_sha}",
                    f"feature~2 {local_sha} refs/heads/other {remote_sha}",
                )
            )
            + "\n"
        )
        self.assertEqual(
            updates,
            (
                ("a" * 40, local_sha, "refs/heads/topic", remote_sha),
                ("feature~2", local_sha, "refs/heads/other", remote_sha),
            ),
        )

    def test_name_status_paths_keeps_normal_rename_and_copy_paths(self) -> None:
        paths = subject.parse_name_status_paths(
            "M\0src/main.rs\0"
            "R100\0Cargo.lock\0renamed.txt\0"
            "C075\0Cargo.toml\0fixtures/Cargo.toml\0"
        )
        self.assertEqual(
            paths,
            [
                "src/main.rs",
                "Cargo.lock",
                "renamed.txt",
                "Cargo.toml",
                "fixtures/Cargo.toml",
            ],
        )

    def test_name_status_paths_rejects_malformed_or_truncated_records(self) -> None:
        for raw in (
            "M\0src/main.rs",
            "R100\0Cargo.lock\0",
            "C\0Cargo.lock\0renamed.txt\0",
            "R101\0Cargo.lock\0renamed.txt\0",
            "M100\0Cargo.lock\0",
            "Z\0unknown\0",
            "A\0\0",
        ):
            with self.subTest(raw=raw):
                with self.assertRaises(subject.ContractViolation):
                    subject.parse_name_status_paths(raw)

    def test_remote_name_with_distinct_fetch_and_push_url_uses_push_url(self) -> None:
        fetch_url = "https://github.com/example/fetch-only.git"
        push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "origin"):
                return fetch_url
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            self.assertEqual(
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name="origin",
                    remote_url=push_url,
                    fallback_branch=None,
                ),
                ("origin", push_url),
            )

    def test_push_url_reverse_resolves_to_configured_remote(self) -> None:
        push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote",):
                return "origin\n"
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            self.assertEqual(
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name=push_url,
                    remote_url=push_url,
                    fallback_branch=None,
                ),
                ("origin", push_url),
            )

    def test_mismatched_remote_name_and_push_url_fails_closed(self) -> None:
        fetch_url = "https://github.com/example/fetch-only.git"
        other_push_url = "https://github.com/example/other.git"
        requested_push_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"

        def run_git(_repository: Path, *arguments: str) -> str:
            if arguments == ("remote", "get-url", "origin"):
                return fetch_url
            if arguments == ("remote", "get-url", "--push", "--all", "origin"):
                return other_push_url
            raise AssertionError(f"unexpected git invocation: {arguments}")

        with patch.object(subject, "_run_git", side_effect=run_git):
            with self.assertRaises(subject.ContractViolation):
                subject._remote_for_push(
                    Path("/tmp/repository"),
                    remote_name="origin",
                    remote_url=requested_push_url,
                    fallback_branch=None,
                )

    def test_python_39_compatibility_contract_is_present_and_help_starts(self) -> None:
        source = Path(subject.__file__).read_text(encoding="utf-8")
        self.assertIn("from __future__ import annotations", source)
        python39 = Path("/usr/bin/python3")
        if python39.exists():
            result = subprocess.run(
                [str(python39), str(Path(subject.__file__)), "--help"],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_pushed_branch_updates_keeps_multiple_branches_and_skips_delete_default_and_tag(
        self,
    ) -> None:
        updates = subject.pushed_branch_updates(
            (
                ("refs/heads/feature-a", "a" * 40, "refs/heads/feature-a", "0" * 40),
                ("refs/heads/feature-b", "b" * 40, "refs/heads/feature-b", "0" * 40),
                ("refs/heads/master", "c" * 40, "refs/heads/master", "0" * 40),
                ("refs/heads/deleted", "0" * 40, "refs/heads/deleted", "d" * 40),
                ("refs/tags/v1", "e" * 40, "refs/tags/v1", "0" * 40),
                ("refs/heads/feature-a", "a" * 40, "refs/heads/feature-a", "0" * 40),
            ),
            default_branch="master",
        )
        self.assertEqual(updates, (("feature-a", "a" * 40), ("feature-b", "b" * 40)))

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

    def test_renamed_lockfile_requires_dependency_evidence_for_its_old_path(self) -> None:
        changed_paths = subject.parse_name_status_paths(
            "R100\0Cargo.lock\0renamed.txt\0"
        )
        with self.assertRaisesRegex(subject.ContractViolation, "依存更新証跡"):
            self.validate(changed_paths=changed_paths)

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

    def test_pr_range_validates_github_metadata_without_git_or_pr_checkout(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "feat: contract\n\nRefs #64"}}
            ],
        }
        files = [[{"filename": "scripts/hooks/pre-push.sh"}]]

        def gh_json(*arguments: str) -> object:
            if arguments == (
                f"repos/HiroyukiFuruno/katana-render-runtime/compare/{base_sha}...{head_sha}",
            ):
                return compare
            if arguments == (
                "--paginate",
                "--slurp",
                "repos/HiroyukiFuruno/katana-render-runtime/pulls/72/files?per_page=100",
            ):
                return files
            raise AssertionError(f"unexpected GitHub API request: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject,
            "_run_git",
            side_effect=AssertionError("PR range mode must not invoke git or check out PR code"),
        ):
            references = subject.validate_pr_range(
                repository="HiroyukiFuruno/katana-render-runtime",
                pr_number=72,
                base_sha=base_sha,
                head_sha=head_sha,
                branch="fix/issue-contract",
                issue_loader=lambda number: self.issue(number),
            )

        self.assertEqual(references, {64})

    def test_pr_range_fails_closed_when_compare_commits_are_truncated(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {"sha": head_sha, "commit": {"message": "feat: only one\n\nRefs #64"}}
            ],
        }

        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "全commit"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_changes_to_any_workflow(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        files = [[{"filename": ".github/workflows/forge-latch.yml"}]]

        def gh_json(*arguments: str) -> object:
            if len(arguments) == 1:
                return compare
            return files

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "GitHub Actions workflow"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_renaming_any_workflow(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        files = [
            [
                {
                    "filename": ".github/workflows/retired.yml",
                    "previous_filename": ".github/workflows/forge-latch.yml",
                }
            ]
        ]

        def gh_json(*arguments: str) -> object:
            if len(arguments) == 1:
                return compare
            return files

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "GitHub Actions workflow"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_rejects_modified_and_deleted_workflows(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }
        cases = {
            "modified": {"filename": ".github/workflows/existing.yml", "status": "modified"},
            "deleted": {"filename": ".github/workflows/retired.yml", "status": "removed"},
        }

        for action, changed_file in cases.items():
            with self.subTest(action=action):
                def gh_json(*arguments: str) -> object:
                    if len(arguments) == 1:
                        return compare
                    return [[changed_file]]

                with patch.object(subject, "_gh_json", side_effect=gh_json):
                    with self.assertRaisesRegex(
                        subject.ContractViolation,
                        "GitHub Actions workflow",
                    ):
                        subject.validate_pr_range(
                            repository="HiroyukiFuruno/katana-render-runtime",
                            pr_number=72,
                            base_sha=base_sha,
                            head_sha=head_sha,
                            branch="fix/issue-contract",
                            issue_loader=lambda number: self.issue(number),
                        )

    def test_pr_range_fails_when_any_base_to_head_commit_lacks_an_issue(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 2,
            "behind_by": 0,
            "total_commits": 2,
            "commits": [
                {
                    "sha": "c" * 40,
                    "commit": {"message": "feat: linked\n\nRefs #64"},
                },
                {"sha": head_sha, "commit": {"message": "fix: unlinked"}},
            ],
        }

        def gh_json(*arguments: str) -> object:
            if len(arguments) == 1:
                return compare
            return [[{"filename": "scripts/hooks/pre-push.sh"}]]

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            with self.assertRaisesRegex(subject.ContractViolation, "Issue参照"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_range_allows_a_base_advanced_after_branch_diverged(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        merge_base_sha = "c" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": merge_base_sha},
            "status": "diverged",
            "ahead_by": 1,
            "behind_by": 2,
            "total_commits": 1,
            "commits": [
                {"sha": head_sha, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }

        def gh_json(*arguments: str) -> object:
            if len(arguments) == 1:
                return compare
            return [[{"filename": "scripts/hooks/pre-push.sh"}]]

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                ),
                {64},
            )

    def test_pr_range_fails_when_compare_final_commit_is_not_head(self) -> None:
        base_sha = "a" * 40
        head_sha = "b" * 40
        compare = {
            "base_commit": {"sha": base_sha},
            "merge_base_commit": {"sha": base_sha},
            "status": "ahead",
            "ahead_by": 1,
            "behind_by": 0,
            "total_commits": 1,
            "commits": [
                {"sha": "c" * 40, "commit": {"message": "fix: contract\n\nRefs #64"}}
            ],
        }

        with patch.object(subject, "_gh_json", return_value=compare):
            with self.assertRaisesRegex(subject.ContractViolation, "最終commit"):
                subject.validate_pr_range(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                    base_sha=base_sha,
                    head_sha=head_sha,
                    branch="fix/issue-contract",
                    issue_loader=lambda number: self.issue(number),
                )

    def test_pr_changed_paths_fails_closed_at_github_files_limit(self) -> None:
        pages = [
            [
                {"filename": f"fixtures/{page}-{entry}.txt"}
                for entry in range(100)
            ]
            for page in range(30)
        ]
        with patch.object(subject, "_gh_json", return_value=pages):
            with self.assertRaisesRegex(subject.ContractViolation, "3,000件上限"):
                subject._pr_changed_paths(
                    repository="HiroyukiFuruno/katana-render-runtime",
                    pr_number=72,
                )

    def test_trusted_workflow_invokes_pr_range_mode_after_base_checkout(self) -> None:
        workflow = (
            Path(subject.__file__).parents[2] / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        checkout = "- name: Check out trusted base repository"
        verifier = "python3 scripts/hooks/verify_push_issue.py"
        self.assertIn("workflow_run:", workflow)
        self.assertNotIn("pull_request_target:", workflow)
        self.assertIn("ref: ${{ steps.pull-request.outputs.base_sha }}", workflow)
        self.assertIn(checkout, workflow)
        self.assertIn(verifier, workflow)
        self.assertIn("--pr-base-sha \"${verified_base}\"", workflow)
        self.assertIn("--pr-head-sha \"${verified_head}\"", workflow)
        self.assertIn("--pr-branch \"${verified_branch}\"", workflow)
        self.assertLess(workflow.index(checkout), workflow.index(verifier))

    def test_final_trusted_status_compares_against_resolved_base_sha(self) -> None:
        workflow = (
            Path(subject.__file__).parents[2] / ".github/workflows/pr-governance.yml"
        ).read_text(encoding="utf-8")
        final_status = workflow[workflow.index("- name: Publish final governance state") :]

        self.assertIn(
            "BASE_SHA: ${{ steps.pull-request.outputs.base_sha }}", final_status
        )
        self.assertIn(
            '[[ "${CURRENT_BASE}" != "${BASE_SHA}" ]]', final_status
        )
        self.assertIn(
            "else\n            state=success\n"
            "            description='Trusted PR governance passed.'",
            final_status,
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

    def test_cli_remote_option_uses_selected_remote_repository_and_default_ref(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            git(
                "remote",
                "add",
                "origin",
                "https://github.com/example/wrong-repository.git",
            )
            git(
                "remote",
                "add",
                "upstream",
                "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            git("update-ref", "refs/remotes/upstream/master", base_sha)
            git("symbolic-ref", "refs/remotes/upstream/HEAD", "refs/remotes/upstream/master")
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: contract", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
            fake_gh = binary_directory / "gh"
            fake_gh.write_text(
                "#!/bin/sh\n"
                "test \"$5\" = \"HiroyukiFuruno/katana-render-runtime\" || exit 21\n"
                "printf '%s\\n' "
                "'{\"number\":64,\"state\":\"OPEN\",\"body\":\"Issue body\","
                "\"url\":\"https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64\"}'\n",
                encoding="utf-8",
            )
            fake_gh.chmod(fake_gh.stat().st_mode | stat.S_IXUSR)
            environment = os.environ.copy()
            environment["PATH"] = f"{binary_directory}:{environment['PATH']}"
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__)), "--remote", "upstream"],
                cwd=repository,
                env=environment,
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

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

    def test_cli_validates_the_pushed_topic_while_master_is_checked_out(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
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
                capture_output=True,
                text=True,
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
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "topic"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "topic.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: missing Issue reference"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            topic_sha = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            subprocess.run(
                ["git", "switch", "master"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            push_update = (
                f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n"
            )
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                input=push_update,
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 1)
            self.assertIn("Issue参照", result.stderr)

    def test_cli_validates_push_update_from_detached_head(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            git(
                "remote",
                "add",
                "origin",
                "https://github.com/HiroyukiFuruno/katana-render-runtime.git",
            )
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: detached push", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
            git("switch", "--detach", base_sha)
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
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_accepts_remote_name_and_url_as_pre_push_arguments(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository = root / "repository"
            binary_directory = root / "bin"
            repository.mkdir()
            binary_directory.mkdir()

            def git(*arguments: str) -> str:
                result = subprocess.run(
                    ["git", *arguments],
                    cwd=repository,
                    check=True,
                    capture_output=True,
                    text=True,
                )
                return result.stdout.strip()

            git("init", "--initial-branch=master")
            git("config", "user.name", "Issue Contract Test")
            git("config", "user.email", "issue@example.com")
            (repository / "base.txt").write_text("base\n", encoding="utf-8")
            git("add", "base.txt")
            git("commit", "-m", "initial")
            base_sha = git("rev-parse", "HEAD")
            remote_url = "https://github.com/HiroyukiFuruno/katana-render-runtime.git"
            git("remote", "add", "origin", remote_url)
            git("update-ref", "refs/remotes/origin/master", base_sha)
            git("symbolic-ref", "refs/remotes/origin/HEAD", "refs/remotes/origin/master")
            git("switch", "-c", "topic")
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            git("add", "topic.txt")
            git("commit", "-m", "feat: url push", "-m", "Refs #64")
            topic_sha = git("rev-parse", "HEAD")
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
                [
                    sys.executable,
                    str(Path(subject.__file__)),
                    "--remote",
                    remote_url,
                    "--remote-url",
                    remote_url,
                ],
                cwd=repository,
                env=environment,
                input=f"refs/heads/topic {topic_sha} refs/heads/topic {'0' * 40}\n",
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("issues=#64", result.stdout)

    def test_cli_skips_tag_only_push_while_topic_is_checked_out(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory)
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
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "update-ref", "refs/remotes/origin/master", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
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
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "switch", "-c", "topic"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            (repository / "topic.txt").write_text("topic\n", encoding="utf-8")
            subprocess.run(
                ["git", "add", "topic.txt"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            subprocess.run(
                ["git", "commit", "-m", "feat: missing Issue reference"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            )
            topic_sha = subprocess.run(
                ["git", "rev-parse", "HEAD"],
                cwd=repository,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()
            result = subprocess.run(
                [sys.executable, str(Path(subject.__file__))],
                cwd=repository,
                input=(
                    f"refs/tags/v0.0.0 {topic_sha} "
                    f"refs/tags/v0.0.0 {'0' * 40}\n"
                ),
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Issue contract skipped", result.stdout)


if __name__ == "__main__":
    unittest.main()
