#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections.abc import Callable, Sequence
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Optional


class ContractViolation(RuntimeError):
    pass


@dataclass(frozen=True)
class Issue:
    number: int
    state: str
    body: str
    url: str


IssueLoader = Callable[[int], Optional[Issue]]

_FULL_ISSUE_PATTERN = re.compile(
    r"https://github\.com/(?P<owner>[^/\s]+)/(?P<repo>[^/\s]+)/issues/(?P<number>[1-9]\d*)",
    re.IGNORECASE,
)
_SHORT_ISSUE_PATTERN = re.compile(r"(?<![\w/])#(?P<number>[1-9]\d*)\b")
_ZERO_SHA = "0" * 40
_SHA_PATTERN = re.compile(r"^[0-9a-fA-F]{40}$")
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


def parse_push_updates(raw: str) -> tuple[tuple[str, str, str, str], ...]:
    updates: list[tuple[str, str, str, str]] = []
    for line in raw.splitlines():
        if not line.strip():
            continue
        fields = line.split()
        if len(fields) != 4:
            raise ContractViolation(f"pre-push updateの形式が不正です: {line!r}")
        local_ref, local_sha, remote_ref, remote_sha = fields
        if not _is_local_ref(local_ref):
            raise ContractViolation(f"pre-push local refの形式が不正です: {local_ref!r}")
        if not _is_remote_ref(remote_ref):
            raise ContractViolation(
                f"pre-push refの形式が不正です: {local_ref!r} -> {remote_ref!r}"
            )
        if not _SHA_PATTERN.fullmatch(local_sha) or not _SHA_PATTERN.fullmatch(
            remote_sha
        ):
            raise ContractViolation(
                f"pre-push SHAの形式が不正です: {local_sha!r} -> {remote_sha!r}"
            )
        updates.append((local_ref, local_sha, remote_ref, remote_sha))
    return tuple(updates)


def _is_remote_ref(reference: str) -> bool:
    """Accept a branch or tag ref using Git's refname safety constraints."""
    if not (reference.startswith("refs/heads/") or reference.startswith("refs/tags/")):
        return False
    suffix = reference.removeprefix("refs/heads/")
    if suffix == reference:
        suffix = reference.removeprefix("refs/tags/")
    if not suffix or suffix.startswith("/") or suffix.endswith("/"):
        return False
    if "//" in suffix or ".." in suffix or "@{" in suffix or suffix.endswith("."):
        return False
    forbidden = set(" ~^:?*[")
    if any(character.isspace() or ord(character) < 32 or character in forbidden for character in suffix):
        return False
    if "\\" in suffix:
        return False
    return all(
        component not in {"", ".", ".."}
        and not component.startswith(".")
        and not component.endswith(".lock")
        for component in suffix.split("/")
    )


def _is_local_ref(reference: str) -> bool:
    if reference == "(delete)":
        return True
    if any(character.isspace() for character in reference):
        return False
    return (
        _SHA_PATTERN.fullmatch(reference) is not None
        or _is_remote_ref(reference)
        or reference == "HEAD"
        or "~" in reference
        or "^" in reference
    )


def pushed_branch_updates(
    updates: Sequence[tuple[str, str, str, str]],
    *,
    default_branch: str,
) -> tuple[tuple[str, str], ...]:
    targets: list[tuple[str, str]] = []
    seen: set[tuple[str, str]] = set()
    for _local_ref, local_sha, remote_ref, _remote_sha in updates:
        if local_sha == _ZERO_SHA:
            continue
        if not remote_ref.startswith("refs/heads/"):
            continue
        branch = remote_ref.removeprefix("refs/heads/")
        if not branch or branch == default_branch:
            continue
        key = (branch, local_sha)
        if key not in seen:
            targets.append(key)
            seen.add(key)
    return tuple(targets)


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


def _configured_remote_for_url(repository: Path, remote_url: str) -> str:
    requested = _normalize_remote_url(remote_url)
    matches = [
        remote
        for remote in _run_git(repository, "remote").splitlines()
        if _normalize_remote_url(_effective_remote_url(repository, remote)) == requested
    ]
    if len(matches) != 1:
        raise ContractViolation(
            "push URLに対応する設定済remoteを一意に判定できません"
        )
    return matches[0]


def _normalize_remote_url(remote_url: str) -> str:
    return remote_url.strip().removesuffix("/").removesuffix(".git")


def _effective_remote_url(repository: Path, remote: str) -> str:
    try:
        return _run_git(repository, "remote", "get-url", "--push", remote)
    except ContractViolation:
        return _run_git(repository, "remote", "get-url", remote)


def _remote_for_push(
    repository: Path,
    *,
    remote_name: str | None,
    remote_url: str | None,
    fallback_branch: str | None,
) -> tuple[str, str]:
    """Return the configured remote used for default-ref comparison and its URL."""
    if remote_name:
        if _is_remote_url(remote_name):
            # Keep compatibility with direct invocations that historically passed a URL
            # to --remote, while the hook itself passes the remote name separately.
            if remote_url is not None and (
                _normalize_remote_url(remote_name) != _normalize_remote_url(remote_url)
            ):
                raise ContractViolation("--remoteと--remote-urlが同じpush先を示していません")
            remote_url = remote_url or remote_name
        else:
            configured_url = _effective_remote_url(repository, remote_name)
            if remote_url is not None and (
                _normalize_remote_url(configured_url) != _normalize_remote_url(remote_url)
            ):
                raise ContractViolation("--remoteと--remote-urlが同じpush先を示していません")
            return remote_name, remote_url or configured_url

    if remote_url:
        return _configured_remote_for_url(repository, remote_url), remote_url

    if fallback_branch:
        remote = branch_remote(repository, fallback_branch)
        return remote, _effective_remote_url(repository, remote)

    remotes = _run_git(repository, "remote").splitlines()
    if "origin" in remotes:
        return "origin", _effective_remote_url(repository, "origin")
    if len(remotes) == 1:
        return remotes[0], _effective_remote_url(repository, remotes[0])
    raise ContractViolation("push remoteを設定済remoteから一意に判定できません")


def _is_remote_url(value: str) -> bool:
    return value.startswith(("git@", "ssh://", "https://", "http://"))


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
        parser = argparse.ArgumentParser(description="Validate Issue references on push")
        parser.add_argument(
            "--remote",
            help="the remote used by the current git push (pre-push hook $1)",
        )
        parser.add_argument(
            "--remote-url",
            help="the remote URL used by the current git push (pre-push hook $2)",
        )
        arguments = parser.parse_args()
        repository = Path(_run_git(Path.cwd(), "rev-parse", "--show-toplevel"))
        push_input = sys.stdin.read()
        updates = parse_push_updates(push_input)
        branch = _run_git(repository, "branch", "--show-current")
        if not updates and not branch:
            raise ContractViolation("空のpre-push入力ではcheckout branchが必要です")

        remote, pushed_remote_url = _remote_for_push(
            repository,
            remote_name=arguments.remote,
            remote_url=arguments.remote_url,
            fallback_branch=branch if not updates else None,
        )
        repository_name = _repository_name(pushed_remote_url)
        default_branch = _default_branch(repository, remote)
        push_updates = pushed_branch_updates(updates, default_branch=default_branch)
        validation_targets = list(push_updates) if push_updates else []
        if not validation_targets and not push_input.strip() and branch != default_branch:
            if not branch:
                raise ContractViolation("空のpre-push入力ではcheckout branchが必要です")
            validation_targets.append((branch, "HEAD"))

        if not validation_targets:
            if branch == default_branch:
                print(f"Issue contract skipped on default branch: {branch}")
            else:
                print("Issue contract skipped: no branch push updates")
            return 0

        default_ref = f"{remote}/{default_branch}"
        cache: dict[int, Issue | None] = {}

        def load_issue(number: int) -> Issue | None:
            if number not in cache:
                cache[number] = _load_issue(repository_name, number)
            return cache[number]

        all_references: set[int] = set()
        for target_branch, target_revision in validation_targets:
            commit_output = _run_git(
                repository,
                "log",
                "--reverse",
                "--format=%B%x00",
                f"{default_ref}..{target_revision}",
            )
            commit_messages = [
                message.strip()
                for message in commit_output.split("\0")
                if message.strip()
            ]
            changed_output = _run_git(
                repository,
                "diff",
                "--name-only",
                f"{default_ref}...{target_revision}",
            )
            changed_paths = [path for path in changed_output.splitlines() if path]
            for message in commit_messages:
                all_references.update(issue_numbers(message, repository_name))

            validate_contract(
                branch=target_branch,
                default_branch=default_branch,
                repository=repository_name,
                commit_messages=commit_messages,
                changed_paths=changed_paths,
                issue_loader=load_issue,
            )

        references = sorted(all_references)
        print(
            "Issue contract passed: "
            f"targets={','.join(target[0] for target in validation_targets)}, "
            f"issues={','.join(f'#{number}' for number in references)}"
        )
        return 0
    except (ContractViolation, json.JSONDecodeError) as error:
        print(f"Issue contract failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
