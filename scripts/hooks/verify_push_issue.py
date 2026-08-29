#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath


class ContractViolation(RuntimeError):
    pass


@dataclass(frozen=True)
class Issue:
    number: int
    state: str
    body: str
    url: str


IssueLoader = Callable[[int], Issue | None]

_FULL_ISSUE_PATTERN = re.compile(
    r"https://github\.com/(?P<owner>[^/\s]+)/(?P<repo>[^/\s]+)/issues/(?P<number>[1-9]\d*)",
    re.IGNORECASE,
)
_SHORT_ISSUE_PATTERN = re.compile(r"(?<![\w/])#(?P<number>[1-9]\d*)\b")
_MANIFEST_NAMES = {
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "Gemfile",
}
_LOCKFILE_NAMES = {
    "Cargo.lock",
    "bun.lock",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "poetry.lock",
    "go.sum",
    "Gemfile.lock",
}
_EVIDENCE_FIELDS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("上流公開版", ("上流公開版", "Upstream release")),
    ("API移行", ("API移行", "API migration")),
    (
        "依存manifest",
        ("依存manifest", "Dependency manifest", "Dependency manifests"),
    ),
    ("lockfile", ("lockfile", "Lockfiles")),
    ("検証証跡", ("検証証跡", "Verification")),
)


def issue_numbers(message: str, repository: str) -> set[int]:
    expected_repository = repository.casefold()
    numbers = {
        int(match.group("number"))
        for match in _FULL_ISSUE_PATTERN.finditer(message)
        if f"{match.group('owner')}/{match.group('repo')}".casefold() == expected_repository
    }
    numbers.update(
        int(match.group("number")) for match in _SHORT_ISSUE_PATTERN.finditer(message)
    )
    return numbers


def dependency_contract_paths(paths: Sequence[str]) -> tuple[list[str], list[str]]:
    manifests: set[str] = set()
    lockfiles: set[str] = set()
    for raw_path in paths:
        path = PurePosixPath(raw_path)
        if path.name in _MANIFEST_NAMES:
            manifests.add(raw_path)
        if path.name in _LOCKFILE_NAMES:
            lockfiles.add(raw_path)
    return sorted(manifests), sorted(lockfiles)


def dependency_evidence_errors(
    body: str,
    manifests: Sequence[str],
    lockfiles: Sequence[str],
) -> list[str]:
    errors: list[str] = []
    if not re.search(
        r"(?im)^##+\s*(?:依存更新証跡|Dependency Update Evidence)\s*$",
        body,
    ):
        errors.append("依存更新証跡の見出し")
    for display_name, labels in _EVIDENCE_FIELDS:
        label_pattern = "|".join(re.escape(label) for label in labels)
        match = re.search(
            rf"(?im)^\s*[-*]\s*(?:{label_pattern})\s*[:：]\s*(?P<value>.+?)\s*$",
            body,
        )
        if match is None or match.group("value").strip().casefold() in {
            "",
            "-",
            "todo",
            "tbd",
        }:
            errors.append(display_name)
    for path in [*manifests, *lockfiles]:
        if path not in body:
            errors.append(path)
    return errors


def validate_contract(
    *,
    branch: str,
    default_branch: str,
    repository: str,
    commit_messages: Sequence[str],
    changed_paths: Sequence[str],
    issue_loader: IssueLoader,
) -> None:
    if branch == default_branch:
        return

    referenced_numbers: set[int] = set()
    for index, message in enumerate(commit_messages, start=1):
        references = issue_numbers(message, repository)
        if not references:
            raise ContractViolation(
                f"非default branchのcommit {index}に対象repositoryのIssue参照がありません"
            )
        referenced_numbers.update(references)

    loaded_issues: list[Issue] = []
    for number in sorted(referenced_numbers):
        issue = issue_loader(number)
        if issue is None:
            raise ContractViolation(f"Issue #{number}を対象repositoryで確認できません")
        if issue.state != "OPEN":
            raise ContractViolation(f"Issue #{number}はOPENではありません: {issue.state}")
        loaded_issues.append(issue)

    manifests, lockfiles = dependency_contract_paths(changed_paths)
    if not manifests and not lockfiles:
        return

    issue_errors = [
        dependency_evidence_errors(issue.body, manifests, lockfiles)
        for issue in loaded_issues
    ]
    if any(not errors for errors in issue_errors):
        return
    missing = sorted({error for errors in issue_errors for error in errors})
    raise ContractViolation(
        "参照Issueの依存更新証跡が不足しています: " + ", ".join(missing)
    )


def _run_git(repository: Path, *arguments: str) -> str:
    result = subprocess.run(
        ["git", *arguments],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ContractViolation(f"git {' '.join(arguments)} failed: {detail}")
    return result.stdout.strip()


def _repository_name(remote_url: str) -> str:
    patterns = (
        r"^git@github\.com:(?P<repository>[^/]+/[^/]+?)(?:\.git)?$",
        r"^https://github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
        r"^ssh://git@github\.com/(?P<repository>[^/]+/[^/]+?)(?:\.git)?/?$",
    )
    for pattern in patterns:
        match = re.match(pattern, remote_url)
        if match is not None:
            return match.group("repository")
    raise ContractViolation(f"GitHub repositoryをremote URLから判定できません: {remote_url}")


def branch_remote(repository: Path, branch: str) -> str:
    configured = subprocess.run(
        ["git", "config", f"branch.{branch}.remote"],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if configured.returncode == 0 and configured.stdout.strip():
        return configured.stdout.strip()
    remotes = _run_git(repository, "remote").splitlines()
    if "origin" in remotes:
        return "origin"
    if len(remotes) == 1:
        return remotes[0]
    raise ContractViolation(f"branch {branch}のpush remoteを判定できません")


def _default_branch(repository: Path, remote: str) -> str:
    symbolic = subprocess.run(
        ["git", "symbolic-ref", "--quiet", "--short", f"refs/remotes/{remote}/HEAD"],
        cwd=repository,
        capture_output=True,
        text=True,
        check=False,
    )
    if symbolic.returncode == 0:
        return symbolic.stdout.strip().removeprefix(f"{remote}/")
    for candidate in ("master", "main"):
        exists = subprocess.run(
            ["git", "show-ref", "--verify", "--quiet", f"refs/remotes/{remote}/{candidate}"],
            cwd=repository,
            check=False,
        )
        if exists.returncode == 0:
            return candidate
    raise ContractViolation(f"{remote}のdefault branchを判定できません")


def _load_issue(repository_name: str, number: int) -> Issue | None:
    result = subprocess.run(
        [
            "gh",
            "issue",
            "view",
            str(number),
            "--repo",
            repository_name,
            "--json",
            "number,state,body,url",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    payload = json.loads(result.stdout)
    return Issue(
        number=int(payload["number"]),
        state=str(payload["state"]),
        body=str(payload.get("body") or ""),
        url=str(payload["url"]),
    )


def main() -> int:
    try:
        repository = Path(_run_git(Path.cwd(), "rev-parse", "--show-toplevel"))
        branch = _run_git(repository, "branch", "--show-current")
        if not branch:
            raise ContractViolation("detached HEADではIssue契約を検証できません")
        remote = branch_remote(repository, branch)
        remote_url = _run_git(repository, "remote", "get-url", remote)
        repository_name = _repository_name(remote_url)
        default_branch = _default_branch(repository, remote)
        if branch == default_branch:
            print(f"Issue contract skipped on default branch: {branch}")
            return 0
        default_ref = f"{remote}/{default_branch}"
        commit_output = _run_git(
            repository,
            "log",
            "--reverse",
            "--format=%B%x00",
            f"{default_ref}..HEAD",
        )
        commit_messages = [message.strip() for message in commit_output.split("\0") if message.strip()]
        changed_output = _run_git(
            repository,
            "diff",
            "--name-only",
            f"{default_ref}...HEAD",
        )
        changed_paths = [path for path in changed_output.splitlines() if path]
        cache: dict[int, Issue | None] = {}

        def load_issue(number: int) -> Issue | None:
            if number not in cache:
                cache[number] = _load_issue(repository_name, number)
            return cache[number]

        validate_contract(
            branch=branch,
            default_branch=default_branch,
            repository=repository_name,
            commit_messages=commit_messages,
            changed_paths=changed_paths,
            issue_loader=load_issue,
        )
        references = sorted(
            {
                number
                for message in commit_messages
                for number in issue_numbers(message, repository_name)
            }
        )
        print(
            f"Issue contract passed: branch={branch}, commits={len(commit_messages)}, "
            f"issues={','.join(f'#{number}' for number in references)}"
        )
        return 0
    except (ContractViolation, json.JSONDecodeError) as error:
        print(f"Issue contract failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
