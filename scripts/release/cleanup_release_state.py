#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path


class CleanupError(RuntimeError):
    pass


ReleaseChecker = Callable[[str], bool]


@dataclass(frozen=True)
class Worktree:
    path: Path
    branch: str | None
    locked: bool


def _run_git(
    repository: Path,
    *arguments: str,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        ["git", *arguments],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if check and result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CleanupError(f"git {' '.join(arguments)} failed: {detail}")
    return result


def _ref_exists(repository: Path, reference: str) -> bool:
    return (
        _run_git(repository, "show-ref", "--verify", "--quiet", reference, check=False).returncode
        == 0
    )


def _remote_branch_exists(repository: Path, remote: str, branch: str) -> bool:
    result = _run_git(
        repository,
        "ls-remote",
        "--exit-code",
        "--heads",
        remote,
        branch,
        check=False,
    )
    if result.returncode not in (0, 2):
        detail = result.stderr.strip() or result.stdout.strip()
        raise CleanupError(f"remote branch audit failed: {detail}")
    return result.returncode == 0


def _fetch_remote_branch(repository: Path, remote: str, branch: str) -> None:
    _run_git(
        repository,
        "fetch",
        remote,
        f"+refs/heads/{branch}:refs/remotes/{remote}/{branch}",
    )


def _is_ancestor(repository: Path, ancestor: str, descendant: str) -> bool:
    result = _run_git(
        repository,
        "merge-base",
        "--is-ancestor",
        ancestor,
        descendant,
        check=False,
    )
    if result.returncode not in (0, 1):
        detail = result.stderr.strip() or result.stdout.strip()
        raise CleanupError(f"merge-base audit failed: {detail}")
    return result.returncode == 0


def _worktrees(repository: Path) -> list[Worktree]:
    output = _run_git(repository, "worktree", "list", "--porcelain").stdout
    worktrees: list[Worktree] = []
    fields: dict[str, str] = {}

    def append_worktree() -> None:
        if "worktree" not in fields:
            return
        branch_ref = fields.get("branch")
        worktrees.append(
            Worktree(
                path=Path(fields["worktree"]).resolve(),
                branch=branch_ref.removeprefix("refs/heads/") if branch_ref else None,
                locked="locked" in fields,
            )
        )

    for line in [*output.splitlines(), ""]:
        if not line:
            append_worktree()
            fields = {}
            continue
        key, _, value = line.partition(" ")
        fields[key] = value
    return worktrees


def _repository_name(repository: Path, remote: str) -> str:
    remote_url = _run_git(repository, "remote", "get-url", remote).stdout.strip()
    for pattern in (
        r"^git@github\.com:(?P<repository>[^/]+/[^/]+?)(?:\.git)?$",
        r"^https://github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
        r"^ssh://git@github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
    ):
        match = re.match(pattern, remote_url)
        if match is not None:
            return match.group("repository")
    raise CleanupError(f"GitHub repositoryをremote URLから判定できません: {remote_url}")


def _github_release_checker(repository: Path, remote: str) -> ReleaseChecker:
    repository_name = _repository_name(repository, remote)

    def is_public(version: str) -> bool:
        result = subprocess.run(
            [
                "gh",
                "release",
                "view",
                version,
                "--repo",
                repository_name,
                "--json",
                "tagName,isDraft",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            return False
        try:
            payload = json.loads(result.stdout)
        except json.JSONDecodeError:
            return False
        return payload.get("tagName") == version and payload.get("isDraft") is False

    return is_public


def _switch_and_update_default(
    repository: Path,
    remote: str,
    default_branch: str,
    actions: list[str],
) -> None:
    current_branch = _run_git(repository, "branch", "--show-current").stdout.strip()
    if current_branch != default_branch:
        if _ref_exists(repository, f"refs/heads/{default_branch}"):
            _run_git(repository, "switch", default_branch)
        else:
            _run_git(
                repository,
                "switch",
                "--track",
                "-c",
                default_branch,
                f"{remote}/{default_branch}",
            )
        actions.append(f"switched to {default_branch}")
    _run_git(repository, "pull", "--ff-only", remote, default_branch)
    actions.append(f"pulled {remote}/{default_branch} with --ff-only")


def cleanup_release_state(
    *,
    repository: Path,
    version: str,
    release_branch: str,
    remote: str,
    default_branch: str,
    release_checker: ReleaseChecker | None = None,
) -> list[str]:
    repository = repository.resolve()
    if release_branch == default_branch:
        raise CleanupError("default branchをcleanup対象にはできません")
    checker = release_checker or _github_release_checker(repository, remote)
    if not checker(version):
        raise CleanupError(f"GitHub Release {version}の公開を確認できません")
    dirty = _run_git(repository, "status", "--porcelain=v1").stdout.strip()
    if dirty:
        raise CleanupError("current worktree is dirty; cleanupを実行しません")

    actions: list[str] = [f"public release {version} verified"]
    _run_git(repository, "fetch", remote, "--prune")
    if not _ref_exists(repository, f"refs/remotes/{remote}/{default_branch}"):
        _fetch_remote_branch(repository, remote, default_branch)
    _switch_and_update_default(repository, remote, default_branch, actions)
    _run_git(repository, "fetch", remote, "--prune")

    remote_exists = _remote_branch_exists(repository, remote, release_branch)
    if remote_exists and not _ref_exists(
        repository,
        f"refs/remotes/{remote}/{release_branch}",
    ):
        _fetch_remote_branch(repository, remote, release_branch)
    local_exists = _ref_exists(repository, f"refs/heads/{release_branch}")
    target_worktrees = [tree for tree in _worktrees(repository) if tree.branch == release_branch]
    target_ref = (
        f"{remote}/{release_branch}"
        if remote_exists
        else release_branch
        if local_exists
        else None
    )
    if target_ref is not None and not _is_ancestor(
        repository,
        target_ref,
        f"{remote}/{default_branch}",
    ):
        raise CleanupError(f"{release_branch}は{default_branch}へ未統合のため保持します")

    for worktree in target_worktrees:
        if worktree.path == repository:
            raise CleanupError(f"cleanup対象branchがcurrent worktreeで使用中です: {worktree.path}")
        if worktree.locked:
            raise CleanupError(f"locked worktreeは保持します: {worktree.path}")
        worktree_dirty = _run_git(
            repository,
            "-C",
            str(worktree.path),
            "status",
            "--porcelain=v1",
        ).stdout.strip()
        if worktree_dirty:
            raise CleanupError(f"dirty worktreeは保持します: {worktree.path}")

    for worktree in target_worktrees:
        _run_git(repository, "worktree", "remove", str(worktree.path))
        actions.append(f"worktree {worktree.path} removed")
    if local_exists:
        _run_git(repository, "branch", "-d", release_branch)
        actions.append(f"local branch {release_branch} deleted")
    if remote_exists:
        _run_git(repository, "push", remote, "--delete", release_branch)
        actions.append(f"remote branch {release_branch} deleted")
    _run_git(repository, "worktree", "prune")
    actions.append("worktree metadata pruned")
    return actions


def _parse_arguments(arguments: Sequence[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="公開済みrelease branchとworktreeを安全条件付きで整理します。"
    )
    parser.add_argument("--version", required=True)
    parser.add_argument("--release-branch")
    parser.add_argument("--remote", default="origin")
    parser.add_argument("--default-branch", default="master")
    parser.add_argument("--repository", type=Path, default=Path.cwd())
    return parser.parse_args(arguments)


def main(arguments: Sequence[str] | None = None) -> int:
    options = _parse_arguments(arguments or sys.argv[1:])
    release_branch = options.release_branch or f"release/{options.version}"
    try:
        actions = cleanup_release_state(
            repository=options.repository,
            version=options.version,
            release_branch=release_branch,
            remote=options.remote,
            default_branch=options.default_branch,
        )
    except CleanupError as error:
        print(f"Release cleanup blocked: {error}", file=sys.stderr)
        return 1
    for action in actions:
        print(action)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
