#!/usr/bin/env python3
"""Verify that a pull request has completed the required review workflow."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from urllib.parse import parse_qs, urlparse
from collections.abc import Mapping, Sequence
from datetime import datetime
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).parents[1] / "hooks"))
import verify_push_issue as issue_contract


_MARKER_PATTERN = re.compile(
    r"<!--\s*krr-review\s+phase=(?P<phase>initial|final)\s+"
    r"head=(?P<head>[0-9a-fA-F]{40})\s*-->"
)
_SUCCESSFUL_CONCLUSIONS = {"SUCCESS", "NEUTRAL", "SKIPPED"}
_VALID_REVIEW_STATES = frozenset({"APPROVED", "CHANGES_REQUESTED", "COMMENTED"})
_SELF_CHECK_NAMES = frozenset(
    {
        "PR governance",
        "KRR / PR governance (trusted check)",
        "KRR / PR governance review latch",
    }
)
_CODEX_REVIEW_TRIGGER = re.compile(r"(?m)^\s*@codex\s+review\s*$")
_TRUSTED_REPLY_ASSOCIATIONS = frozenset({"COLLABORATOR", "MEMBER", "OWNER"})
_TRUSTED_CHECK = "KRR / PR governance (trusted check)"
_LATCH_CHECK = "KRR / PR governance review latch"


def _bot_login(value: object) -> str | None:
    if not isinstance(value, Mapping):
        return None
    login = value.get("login")
    return login if isinstance(login, str) else None


def _is_review_bot(login: str | None, review_bot: str) -> bool:
    if login is None:
        return False
    return login.removesuffix("[bot]").casefold() == review_bot.removesuffix(
        "[bot]"
    ).casefold()


def _timestamp(value: object, field: str) -> datetime | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    return datetime.fromisoformat(normalized)


def _review_completion_times(
    reviews: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> list[datetime]:
    not_before_time = _timestamp(not_before, "marker updated_at")
    if not_before_time is None:
        raise TypeError("marker updated_at must be an ISO-8601 timestamp")
    before_time = _timestamp(before, "marker created_at")
    completion_times: list[datetime] = []
    for review in reviews:
        if not (
            _is_review_bot(_bot_login(review.get("author")), review_bot)
            and review.get("state") in _VALID_REVIEW_STATES
            and isinstance(review.get("commit"), Mapping)
            and review["commit"].get("oid") == head
        ):
            continue
        submitted_at = _timestamp(review.get("submittedAt"), "review submittedAt")
        if (
            submitted_at is not None
            and submitted_at > not_before_time
            and (before_time is None or submitted_at < before_time)
        ):
            completion_times.append(submitted_at)
    return completion_times


def _review_is_for(
    reviews: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    return bool(
        _review_completion_times(reviews, review_bot, head, not_before, before)
    )


def _marker_updated_at(comment: Mapping[str, object]) -> object:
    """Return the marker's latest edit time for evidence freshness checks."""

    if "updated_at" in comment:
        return comment["updated_at"]
    if "updatedAt" in comment:
        return comment["updatedAt"]
    # Keep accepting legacy fixtures/API projections that never exposed the
    # edit field; an explicitly present null or invalid value still fails closed.
    return comment.get("created_at")


def _marker_updated_time(comment: Mapping[str, object]) -> datetime | None:
    try:
        return _timestamp(_marker_updated_at(comment), "marker updated_at")
    except (TypeError, ValueError):
        return None


def _bot_plus_one_time(
    comment: Mapping[str, object],
    reactions: Mapping[int, Sequence[Mapping[str, object]]],
    review_bot: str,
    not_before: object,
    before: object | None = None,
) -> datetime | None:
    comment_id = comment["id"]
    if not isinstance(comment_id, int):
        raise TypeError("comment id must be an integer")
    not_before_time = _timestamp(not_before, "marker created_at")
    if not_before_time is None:
        raise TypeError("marker created_at must be an ISO-8601 timestamp")
    before_time = _timestamp(before, "marker created_at")
    edited_at = comment.get("updated_at") or comment.get("updatedAt")
    if edited_at is None:
        # An old reaction may predate an edited marker while still being newer
        # than its original creation time, so missing edit evidence is unsafe.
        return None
    try:
        edited_time = _timestamp(edited_at, "marker updated_at")
    except (TypeError, ValueError):
        return None
    if edited_time is None:
        return None
    if edited_time < not_before_time:
        return None

    def is_timely(reaction: Mapping[str, object]) -> bool:
        created_at = reaction.get("created_at")
        if created_at is None:
            return False
        try:
            reaction_time = _timestamp(created_at, "reaction created_at")
        except (TypeError, ValueError):
            return False
        return (
            reaction_time is not None
            and reaction_time > edited_time
            and (before_time is None or reaction_time < before_time)
        )

    valid_times: list[datetime] = []
    for reaction in reactions.get(comment_id, ()):
        if (
            reaction.get("content") == "+1"
            and _is_review_bot(_bot_login(reaction.get("user")), review_bot)
            and is_timely(reaction)
        ):
            created_at = _timestamp(reaction.get("created_at"), "reaction created_at")
            if created_at is not None:
                valid_times.append(created_at)
    return max(valid_times, default=None)


def _bot_plus_one(
    comment: Mapping[str, object],
    reactions: Mapping[int, Sequence[Mapping[str, object]]],
    review_bot: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    return (
        _bot_plus_one_time(comment, reactions, review_bot, not_before, before)
        is not None
    )


def _latest_issue_updated_time(
    referenced_issues: Sequence[issue_contract.Issue],
) -> datetime | None:
    """Return the newest referenced Issue edit time, failing closed per item."""

    if not referenced_issues:
        return None
    timestamps: list[datetime] = []
    for issue in referenced_issues:
        try:
            updated_at = _timestamp(issue.updated_at, f"Issue #{issue.number} updated_at")
        except (TypeError, ValueError):
            return None
        if updated_at is None:
            return None
        timestamps.append(updated_at)
    return max(timestamps)


def closing_reference_errors(
    *,
    repository: str,
    body: object,
    referenced_issues: Sequence[issue_contract.Issue],
) -> list[str]:
    """Require one canonical open Issue in both commits and the PR body."""

    if not isinstance(body, str):
        raise TypeError("pull request body must be a string")
    errors: list[str] = []
    if len(referenced_issues) != 1:
        errors.append(
            "commit範囲の参照Issueはちょうど1件のOPEN Issueである必要があります: "
            f"{len(referenced_issues)}件"
        )
    else:
        issue = referenced_issues[0]
        if type(issue.number) is not int or issue.number < 1:
            errors.append("commit範囲のcanonical Issue番号が不正です")
        elif issue.state != "OPEN":
            errors.append(
                "commit範囲の参照IssueはOPENである必要があります: "
                f"#{issue.number}={issue.state}"
            )
        elif (
            not isinstance(issue.url, str)
            or issue.url.casefold()
            != f"https://github.com/{repository}/issues/{issue.number}".casefold()
        ):
            errors.append(
                f"Issue #{issue.number}は対象repositoryのcanonical Issue URLではありません"
            )
    referenced_numbers = {issue.number for issue in referenced_issues}
    closing_numbers = issue_contract.closing_issue_numbers(body, repository)
    missing = sorted(referenced_numbers - closing_numbers)
    extra = sorted(closing_numbers - referenced_numbers)
    if missing or extra:
        details: list[str] = []
        if missing:
            details.append(
                "不足=" + ", ".join(f"#{number}" for number in missing)
            )
        if extra:
            details.append(
                "余分=" + ", ".join(f"#{number}" for number in extra)
            )
        errors.append(
            "PR本文のGitHub closing Issue集合がcommit範囲参照Issue集合と一致しません: "
            + "; ".join(details)
        )
    return errors


def _open_pull_requests(repository: str) -> list[dict[str, object]]:
    """Read every open PR body through GraphQL cursor pagination."""

    owner, name = repository.split("/", maxsplit=1)
    pull_requests: list[dict[str, object]] = []
    seen_numbers: set[int] = set()
    seen_cursors: set[str] = set()
    cursor: str | None = None
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            "query="
            "query($owner: String!, $name: String!, $cursor: String) {\n"
            "  repository(owner: $owner, name: $name) {\n"
            "    pullRequests(first: 100, states: OPEN, after: $cursor) {\n"
            "      nodes { number isDraft body }\n"
            "      pageInfo { hasNextPage endCursor }\n"
            "    }\n"
            "  }\n"
            "}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        payload = _gh_json(*arguments)
        if not isinstance(payload, Mapping):
            raise TypeError("open pull requests response must be an object")
        data = payload.get("data")
        if not isinstance(data, Mapping):
            raise TypeError("open pull requests data must be an object")
        repository_data = data.get("repository")
        if not isinstance(repository_data, Mapping):
            raise TypeError("open pull requests repository must be an object")
        connection = repository_data.get("pullRequests")
        if not isinstance(connection, Mapping):
            raise TypeError("open pull requests connection must be an object")
        nodes = connection.get("nodes")
        if not isinstance(nodes, list):
            raise TypeError("open pull requests nodes must be an array")
        for node in nodes:
            if not isinstance(node, Mapping):
                raise TypeError("open pull request must be an object")
            number = node.get("number")
            is_draft = node.get("isDraft")
            body = node.get("body")
            if type(number) is not int or number < 1:
                raise TypeError("open pull request number must be a positive integer")
            if number in seen_numbers:
                raise TypeError("open pull requests contain a duplicate number")
            if not isinstance(is_draft, bool):
                raise TypeError("open pull request isDraft must be a boolean")
            if not isinstance(body, str):
                raise TypeError("open pull request body must be a string")
            pull_requests.append(
                {"number": number, "isDraft": is_draft, "body": body}
            )
            seen_numbers.add(number)
        page_info = connection.get("pageInfo")
        if not isinstance(page_info, Mapping):
            raise TypeError("open pull requests pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            return pull_requests
        if has_next_page is not True:
            raise TypeError("open pull requests hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("open pull requests endCursor must be a unique string")
        seen_cursors.add(cursor)


def _open_pull_request_snapshot(path: str) -> list[dict[str, object]]:
    """Load the immutable single-arbiter open-PR snapshot fail-closed."""
    try:
        with open(path, encoding="utf-8") as source:
            payload = json.load(source)
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError("open pull request snapshot is unavailable") from error
    if not isinstance(payload, list):
        raise TypeError("open pull request snapshot must be an array")
    values: list[dict[str, object]] = []
    seen: set[int] = set()
    for item in payload:
        if not isinstance(item, Mapping):
            raise TypeError("open pull request snapshot item must be an object")
        number, draft, body = item.get("number"), item.get("isDraft"), item.get("body")
        if type(number) is not int or number < 1 or number in seen:
            raise TypeError("open pull request snapshot number is invalid")
        if not isinstance(draft, bool) or not isinstance(body, str):
            raise TypeError("open pull request snapshot item is invalid")
        seen.add(number)
        values.append({"number": number, "isDraft": draft, "body": body})
    return values


def closing_open_pull_request_errors(
    *,
    repository: str,
    current_pull_request: int,
    referenced_issues: Sequence[issue_contract.Issue],
    open_pull_requests: Sequence[Mapping[str, object]],
) -> list[str]:
    """Require the canonical Issue to have this single open PR as its closer.

    Draft PRs are included deliberately: waiting until a sibling is Ready
    leaves a check-to-Ready race in which two Draft PRs can both pass.
    """

    errors: list[str] = []
    for issue in referenced_issues:
        closers = _open_pull_request_closers(
            repository=repository,
            issue_number=issue.number,
            open_pull_requests=open_pull_requests,
        )
        if closers != {current_pull_request}:
            rendered = ", ".join(f"#{number}" for number in sorted(closers)) or "なし"
            errors.append(
                f"Issue #{issue.number}をclosingするopen PRは自身だけである必要があります: {rendered}"
            )
    return errors


def _canonical_issue_identity(
    *,
    repository: str,
    referenced_issues: Sequence[issue_contract.Issue],
) -> tuple[tuple[int, str, str, str, str], ...]:
    """Return the immutable fields used by the final canonical-Issue fence."""

    identity: list[tuple[int, str, str, str, str]] = []
    for issue in referenced_issues:
        number = issue.number
        state = issue.state
        body = issue.body
        url = issue.url
        updated_at = issue.updated_at
        if type(number) is not int or number < 1:
            raise TypeError("canonical Issue number must be a positive integer")
        if not isinstance(state, str) or not state:
            raise TypeError("canonical Issue state must be a non-empty string")
        if not isinstance(body, str):
            raise TypeError("canonical Issue body must be a string")
        if not isinstance(url, str) or not url:
            raise TypeError("canonical Issue url must be a non-empty string")
        canonical_url = f"https://github.com/{repository}/issues/{number}"
        if url.casefold() != canonical_url.casefold():
            raise ValueError("canonical Issue url does not match the repository")
        if not isinstance(updated_at, str) or not updated_at:
            raise TypeError("canonical Issue updated_at must be a non-empty string")
        identity.append((number, state, body, url, updated_at))
    return tuple(identity)


def _required_timestamp_text(value: object, field: str) -> str:
    """Validate and preserve an immutable GitHub timestamp string."""

    if not isinstance(value, str):
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    if _timestamp(value, field) is None:
        raise TypeError(f"{field} must be an ISO-8601 timestamp")
    return value


def _final_review_evidence_is_fresh(
    reviews: Sequence[Mapping[str, object]],
    reactions: Mapping[int, Sequence[Mapping[str, object]]],
    review_bot: str,
    head: str,
    comment: Mapping[str, object],
    issue_updated_at: datetime | None,
) -> bool:
    """Require final completion evidence after both its marker and Issue edits."""

    marker_time = _marker_updated_time(comment)
    if marker_time is None:
        return False
    freshness_floor = max(
        marker_time,
        issue_updated_at or marker_time,
    )
    review_times = _review_completion_times(
        reviews, review_bot, head, _marker_updated_at(comment)
    )
    if any(submitted_at > freshness_floor for submitted_at in review_times):
        return True
    reaction_time = _bot_plus_one_time(
        comment, reactions, review_bot, comment["created_at"]
    )
    return reaction_time is not None and reaction_time > freshness_floor


def _review_markers(comments: Sequence[Mapping[str, object]]) -> list[tuple[str, str, Mapping[str, object]]]:
    markers: list[tuple[str, str, Mapping[str, object]]] = []
    for comment in comments:
        body = comment["body"]
        if not isinstance(body, str):
            raise TypeError("comment body must be a string")
        if _CODEX_REVIEW_TRIGGER.search(body) is None:
            continue
        for match in _MARKER_PATTERN.finditer(body):
            markers.append((match.group("phase"), match.group("head").lower(), comment))
    return markers


def _resolved_thread_has_author_reply(
    thread: Mapping[str, object], author_login: str
) -> bool:
    """Return whether a resolved review thread has a reply from the PR author.

    The first thread comment is the review root.  A response on the root itself
    is not evidence that its author addressed the finding, so only later
    comments count.
    """

    comments = thread.get("comments")
    if not isinstance(comments, Sequence) or isinstance(comments, (str, bytes)):
        return False
    if not comments:
        return False
    for comment in comments[1:]:
        if not isinstance(comment, Mapping):
            raise TypeError("review thread comment must be an object")
        comment_author = comment.get("author")
        comment_login = _bot_login(comment_author)
        if comment_login is None:
            continue
        if comment_login.casefold() == author_login.casefold():
            return True
        association = comment.get("authorAssociation")
        if (
            isinstance(association, str)
            and association.upper() in _TRUSTED_REPLY_ASSOCIATIONS
        ):
            return True
    return False


def _is_self_check(check: Mapping[str, object]) -> bool:
    """Exclude only the governance contexts that would otherwise self-cycle."""

    return check.get("name", check.get("context")) in _SELF_CHECK_NAMES


def _check_error(check: Mapping[str, object]) -> str | None:
    name = check.get("name", check.get("context"))
    if _is_self_check(check):
        return None
    if not isinstance(name, str) or not name:
        raise TypeError("status check name must be a string")

    status = check.get("status")
    conclusion = check.get("conclusion")
    if status == "COMPLETED" and isinstance(conclusion, str):
        if conclusion.upper() in _SUCCESSFUL_CONCLUSIONS:
            return None
    # StatusContext values are returned as a terminal state instead of a
    # CheckRun status/conclusion pair by some gh versions.
    state = check.get("state")
    if isinstance(state, str) and state.upper() in _SUCCESSFUL_CONCLUSIONS:
        return None
    return f"CI check が未完了または失敗しています: {name}"


def readiness_errors(
    pull_request: Mapping[str, object],
    threads: Sequence[Mapping[str, object]],
    comments: Sequence[Mapping[str, object]],
    reactions: Mapping[int, Sequence[Mapping[str, object]]],
    review_bot: str,
    require_draft: bool,
    referenced_issues: Sequence[issue_contract.Issue] = (),
) -> list[str]:
    """Return every unmet PR readiness condition without changing GitHub state."""

    errors: list[str] = []
    head = pull_request["headRefOid"]
    if not isinstance(head, str) or re.fullmatch(r"[0-9a-fA-F]{40}", head) is None:
        raise ValueError("pull request headRefOid must be a 40-character SHA")
    head = head.lower()

    is_draft = pull_request["isDraft"]
    if not isinstance(is_draft, bool):
        raise TypeError("pull request isDraft must be a boolean")
    if require_draft and not is_draft:
        errors.append("Draft PR でのみ readiness gate を実行できます")

    status_rollup = pull_request["statusCheckRollup"]
    if not isinstance(status_rollup, Sequence) or isinstance(status_rollup, (str, bytes)):
        raise TypeError("statusCheckRollup must be a sequence")
    for check in status_rollup:
        if not isinstance(check, Mapping):
            raise TypeError("status check must be an object")
        if _is_self_check(check):
            continue
        error = _check_error(check)
        if error is not None:
            errors.append(error)
    if not any(
        not _is_self_check(check)
        for check in status_rollup
    ):
        errors.append("CI check を取得できません")

    unresolved = [thread for thread in threads if thread.get("isResolved") is not True]
    if unresolved:
        errors.append(f"未resolve review thread が {len(unresolved)} 件あります")

    author = pull_request.get("author")
    author_login = _bot_login(author)
    if author is not None and author_login is None:
        raise TypeError("pull request author.login must be a string")
    if author_login is not None:
        missing_replies = [
            thread
            for thread in threads
            if thread.get("isResolved") is True
            and not _resolved_thread_has_author_reply(thread, author_login)
        ]
        if missing_replies:
            errors.append(
                f"resolve 済み review thread に PR author の reply が {len(missing_replies)} 件ありません"
            )

    reviews = pull_request["reviews"]
    if not isinstance(reviews, Sequence) or isinstance(reviews, (str, bytes)):
        raise TypeError("reviews must be a sequence")
    typed_reviews: list[Mapping[str, object]] = []
    for review in reviews:
        if not isinstance(review, Mapping):
            raise TypeError("review must be an object")
        typed_reviews.append(review)

    issue_updated_at = _latest_issue_updated_time(referenced_issues)
    if referenced_issues and issue_updated_at is None:
        errors.append("参照Issue snapshotのupdated_atが不正です")

    markers = _review_markers(comments)
    initial_markers = [marker for marker in markers if marker[0] == "initial"]
    final_markers = [marker for marker in markers if marker[0] == "final"]

    if not initial_markers:
        errors.append("initial review marker がありません")
    elif not any(
        _review_is_for(
            typed_reviews,
            review_bot,
            marker_head,
            _marker_updated_at(comment),
            final_comment["created_at"],
        )
        or _bot_plus_one(
            comment,
            reactions,
            review_bot,
            comment["created_at"],
            final_comment["created_at"],
        )
        for _phase, marker_head, comment in initial_markers
        for _final_phase, _final_head, final_comment in final_markers
    ):
        errors.append("initial review に review bot の完了記録がありません")

    current_final_markers = [
        marker for marker in final_markers if marker[1] == head
    ]
    if current_final_markers:
        # A later final marker supersedes earlier evidence, including when an
        # old initial marker was edited into a final marker.
        marker_times = [
            _marker_updated_time(marker[2]) for marker in current_final_markers
        ]
        if any(marker_time is None for marker_time in marker_times):
            current_final_markers = []
        else:
            current_final_markers = [
                max(
                    zip(current_final_markers, marker_times),
                    key=lambda item: item[1],
                )[0]
            ]
    if not final_markers:
        errors.append("final review marker がありません")
    elif not current_final_markers:
        errors.append("final review marker が最新HEADを指していません")
    elif not any(
        _final_review_evidence_is_fresh(
            typed_reviews,
            reactions,
            review_bot,
            head,
            comment,
            issue_updated_at,
        )
        for _phase, _marker_head, comment in current_final_markers
    ):
        errors.append("final review に参照Issue更新後の review bot 完了記録がありません")

    return errors


def _gh_json(*arguments: str) -> Any:
    completed = subprocess.run(
        ["gh", *arguments],
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(completed.stdout)


def _repository_name(repository: str | None) -> str:
    if repository is not None:
        return repository
    payload = _gh_json("repo", "view", "--json", "nameWithOwner")
    return payload["nameWithOwner"]


def _verify_pr_boundary_unchanged(
    repository: str,
    pull_request: int,
    initial_base: str,
    initial_head: str,
) -> None:
    """Fail closed if the PR boundary moved while readiness was evaluated."""

    payload = _gh_json(
        "pr",
        "view",
        str(pull_request),
        "--repo",
        repository,
        "--json",
        "baseRefOid,headRefOid",
    )
    if not isinstance(payload, dict):
        raise TypeError("pull request boundary response must be an object")
    current_base = payload.get("baseRefOid")
    current_head = payload.get("headRefOid")
    if not isinstance(current_base, str) or not isinstance(current_head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    if (current_base, current_head) != (initial_base, initial_head):
        raise ValueError("pull request base/head changed during readiness check")


def _verify_final_readiness_snapshot_unchanged(
    *,
    repository: str,
    pull_request: int,
    initial_base: str,
    initial_head: str,
    initial_body: str,
    initial_updated_at: str,
    initial_issue_identity: tuple[tuple[int, str, str, str, str], ...],
    initial_closers: frozenset[int],
    open_pull_requests: Sequence[Mapping[str, object]] | None = None,
) -> None:
    """Fence every mutable input that justified a successful readiness result."""

    payload = _gh_json(
        "pr",
        "view",
        str(pull_request),
        "--repo",
        repository,
        "--json",
        "baseRefOid,headRefOid,body,updatedAt",
    )
    if not isinstance(payload, dict):
        raise TypeError("pull request final snapshot response must be an object")
    current_base = payload.get("baseRefOid")
    current_head = payload.get("headRefOid")
    current_body = payload.get("body")
    current_updated_at = _required_timestamp_text(
        payload.get("updatedAt"), "pull request updatedAt"
    )
    if not isinstance(current_base, str) or not isinstance(current_head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    if not isinstance(current_body, str):
        raise TypeError("pull request body must be a string")
    if (current_base, current_head) != (initial_base, initial_head):
        raise ValueError("pull request base/head changed during readiness check")
    if current_body != initial_body:
        raise ValueError("pull request body changed during readiness check")
    if current_updated_at != initial_updated_at:
        raise ValueError("pull request updatedAt changed during readiness check")

    current_issues = issue_contract.referenced_issue_snapshot(
        repository=repository,
        base_sha=initial_base,
        head_sha=initial_head,
    )
    current_issue_identity = _canonical_issue_identity(
        repository=repository,
        referenced_issues=current_issues,
    )
    if current_issue_identity != initial_issue_identity:
        raise ValueError("canonical Issue snapshot changed during readiness check")
    if len(current_issues) != 1:
        raise ValueError("canonical Issue snapshot is no longer exactly one Issue")

    current_closers = frozenset(
        number
        for number in _open_pull_request_closers(
            repository=repository,
            issue_number=current_issues[0].number,
            open_pull_requests=(
                list(open_pull_requests)
                if open_pull_requests is not None
                else _open_pull_requests(repository)
            ),
        )
    )
    if current_closers != initial_closers:
        raise ValueError("open PR closer set changed during readiness check")


def _open_pull_request_closers(
    *,
    repository: str,
    issue_number: int,
    open_pull_requests: Sequence[Mapping[str, object]],
) -> set[int]:
    """Return every open PR that closes one Issue, including Draft PRs."""

    closers: set[int] = set()
    for pull_request in open_pull_requests:
        number = pull_request.get("number")
        is_draft = pull_request.get("isDraft")
        body = pull_request.get("body")
        if type(number) is not int or number < 1:
            raise TypeError("open pull request number must be a positive integer")
        if not isinstance(is_draft, bool):
            raise TypeError("open pull request isDraft must be a boolean")
        if not isinstance(body, str):
            raise TypeError("open pull request body must be a string")
        if issue_number in issue_contract.closing_issue_numbers(body, repository):
            closers.add(number)
    return closers


def _expected_boundary(
    expected_base: str | None, expected_head: str | None
) -> tuple[str, str] | None:
    """Validate an optional caller-supplied immutable PR boundary."""

    if (expected_base is None) != (expected_head is None):
        raise ValueError("expected base and head SHA must be provided together")
    if expected_base is None:
        return None
    if (
        re.fullmatch(r"[0-9a-fA-F]{40}", expected_base) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", expected_head) is None
    ):
        raise ValueError("expected base and head SHA must be 40-character SHAs")
    return expected_base.lower(), expected_head.lower()


def _require_boundary(
    base: object, head: object, expected: tuple[str, str] | None
) -> tuple[str, str]:
    if not isinstance(base, str) or not isinstance(head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    if (
        re.fullmatch(r"[0-9a-fA-F]{40}", base) is None
        or re.fullmatch(r"[0-9a-fA-F]{40}", head) is None
    ):
        raise ValueError("pull request baseRefOid/headRefOid must be 40-character SHAs")
    boundary = base.lower(), head.lower()
    if expected is not None and boundary != expected:
        raise ValueError("pull request initial base/head does not match expected boundary")
    return boundary


def _review_threads(repository: str, pull_request: int) -> list[dict[str, object]]:
    query = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: 100, after: $cursor) {
        nodes {
          id
          isResolved
          comments(first: 100) {
            nodes {
              author { login }
              authorAssociation
            }
            pageInfo { hasNextPage endCursor }
          }
        }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    owner, name = repository.split("/", maxsplit=1)
    threads: list[dict[str, object]] = []
    cursor: str | None = None
    seen_cursors: set[str] = set()
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
            "-F",
            f"number={pull_request}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        payload = _gh_json(*arguments)
        connection = payload["data"]["repository"]["pullRequest"]["reviewThreads"]
        if not isinstance(connection, Mapping):
            raise TypeError("reviewThreads must be an object")
        nodes = connection["nodes"]
        if not isinstance(nodes, list):
            raise TypeError("reviewThreads nodes must be an array")
        for node in nodes:
            if not isinstance(node, dict):
                raise TypeError("review thread must be an object")
            thread_comments = node.get("comments")
            # Compatibility for old gh fixtures.  In a live GraphQL response
            # this key is always present because it is explicitly selected.
            if thread_comments is not None:
                if not isinstance(thread_comments, Mapping):
                    raise TypeError("review thread comments must be an object")
                page_info = thread_comments.get("pageInfo")
                if not isinstance(page_info, Mapping):
                    raise TypeError("review thread comments pageInfo must be an object")
                if page_info.get("hasNextPage") is not False:
                    raise ValueError("thread comments are truncated")
                comment_nodes = thread_comments.get("nodes")
                if not isinstance(comment_nodes, list):
                    raise TypeError("review thread comments nodes must be an array")
                node = {**node, "comments": comment_nodes}
            threads.append(node)
        page_info = connection["pageInfo"]
        if not isinstance(page_info, Mapping):
            raise TypeError("reviewThreads pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            return threads
        if has_next_page is not True:
            raise TypeError("reviewThreads hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("reviewThreads endCursor must be a string")
        seen_cursors.add(cursor)


def _reviews(repository: str, pull_request: int) -> list[dict[str, object]]:
    """Read every pull-request review page, failing closed on invalid cursors."""

    query = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviews(first: 100, after: $cursor) {
        nodes { author { login } commit { oid } state submittedAt }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    owner, name = repository.split("/", maxsplit=1)
    reviews: list[dict[str, object]] = []
    cursor: str | None = None
    seen_cursors: set[str] = set()
    while True:
        arguments = [
            "api",
            "graphql",
            "-f",
            f"query={query}",
            "-F",
            f"owner={owner}",
            "-F",
            f"name={name}",
            "-F",
            f"number={pull_request}",
        ]
        if cursor is not None:
            arguments.extend(("-F", f"cursor={cursor}"))
        payload = _gh_json(*arguments)
        connection = payload["data"]["repository"]["pullRequest"]["reviews"]
        if not isinstance(connection, Mapping):
            raise TypeError("reviews must be an object")
        nodes = connection["nodes"]
        if not isinstance(nodes, list):
            raise TypeError("reviews nodes must be an array")
        for node in nodes:
            if not isinstance(node, dict):
                raise TypeError("review must be an object")
            reviews.append(node)
        page_info = connection["pageInfo"]
        if not isinstance(page_info, Mapping):
            raise TypeError("reviews pageInfo must be an object")
        has_next_page = page_info.get("hasNextPage")
        if has_next_page is False:
            return reviews
        if has_next_page is not True:
            raise TypeError("reviews hasNextPage must be a boolean")
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor or cursor in seen_cursors:
            raise TypeError("reviews endCursor must be a string")
        seen_cursors.add(cursor)


def _paginated_api_array(endpoint: str) -> list[dict[str, object]]:
    """Fetch every REST page and normalize gh --slurp output to one array."""

    payload = _gh_json("api", endpoint, "--paginate", "--slurp")
    if not isinstance(payload, list):
        raise TypeError("paginated GitHub API response must be an array")
    if not payload:
        return []
    if all(isinstance(item, Mapping) for item in payload):
        return list(payload)
    flattened: list[dict[str, object]] = []
    for page in payload:
        if not isinstance(page, list):
            raise TypeError("paginated GitHub API page must be an array")
        for item in page:
            if not isinstance(item, dict):
                raise TypeError("GitHub API item must be an object")
            flattened.append(item)
    return flattened


def _comment_reactions(
    repository: str, comments: Sequence[Mapping[str, object]]
) -> dict[int, list[dict[str, object]]]:
    reactions: dict[int, list[dict[str, object]]] = {}
    for comment in comments:
        comment_id = comment["id"]
        if not isinstance(comment_id, int):
            raise TypeError("comment id must be an integer")
        reactions[comment_id] = _paginated_api_array(
            f"repos/{repository}/issues/comments/{comment_id}/reactions"
        )
    return reactions


def _governance_check_error(
    repository: str, pull_request: int, base_branch: object, base_sha: str, head: str,
    evidence: dict[str, object] | None = None,
) -> str | None:
    """Require exactly one terminal trusted Check Run and its Actions evidence."""
    if not isinstance(base_branch, str) or not re.fullmatch(r"[A-Za-z0-9._/-]+", base_branch):
        raise TypeError("baseRefName must be a safe branch name")
    protection = _gh_json("api", f"repos/{repository}/branches/{base_branch}/protection/required_status_checks")
    if not isinstance(protection, Mapping):
        raise TypeError("required status checks response must be an object")
    checks = protection.get("checks")
    if not isinstance(checks, list):
        raise TypeError("required status checks checks must be an array")
    trusted_apps = [item.get("app_id") for item in checks if isinstance(item, Mapping) and item.get("context") == _TRUSTED_CHECK]
    latch_apps = [item.get("app_id") for item in checks if isinstance(item, Mapping) and item.get("context") == _LATCH_CHECK]
    if len(trusted_apps) != 1 or type(trusted_apps[0]) is not int or trusted_apps[0] < 1:
        return "branch protection trusted Check Run App binding is missing or ambiguous"
    if latch_apps != [15368]:
        return "branch protection review latch App binding is not exact"
    app_id = trusted_apps[0]
    raw_pages = _gh_json("api", "--paginate", "--slurp", f"repos/{repository}/commits/{head}/check-runs?check_name={_TRUSTED_CHECK.replace(' ', '%20')}&app_id={app_id}&filter=all&per_page=100")
    if not isinstance(raw_pages, list) or not all(isinstance(page, Mapping) for page in raw_pages):
        raise TypeError("check-runs pagination response must contain page objects")
    runs: list[Mapping[str, object]] = []
    for page in raw_pages:
        page_runs = page.get("check_runs")
        if not isinstance(page_runs, list) or not all(isinstance(run, Mapping) for run in page_runs):
            raise TypeError("check-runs page must contain an array")
        runs.extend(page_runs)
    matches = [run for run in runs if isinstance(run, Mapping) and run.get("name") == _TRUSTED_CHECK and run.get("head_sha", "").lower() == head.lower() and isinstance(run.get("app"), Mapping) and run["app"].get("id") == app_id]
    if len(matches) != 1:
        return "trusted Check Run must have exactly one matching App/head run"
    run = matches[0]
    if run.get("status") != "completed" or run.get("conclusion") != "success":
        return "trusted Check Run is not completed successfully"
    external_id = run.get("external_id")
    details = run.get("details_url")
    if external_id != f"krr-governance/v1/{head.lower()}":
        return "trusted Check Run external_id is invalid"
    if not isinstance(details, str):
        return "trusted Check Run details_url lacks exact source_run_id evidence"
    source_ids = parse_qs(urlparse(details).query, keep_blank_values=True).get("source_run_id", [])
    if len(source_ids) != 1 or re.fullmatch(r"[1-9][0-9]*", source_ids[0]) is None:
        return "trusted Check Run details_url lacks exact source_run_id evidence"
    latch_payload = _gh_json("api", "--paginate", "--slurp", f"repos/{repository}/commits/{head}/check-runs?check_name={_LATCH_CHECK.replace(' ', '%20')}&app_id=15368&filter=all&per_page=100")
    if not isinstance(latch_payload, list) or not all(isinstance(page, Mapping) for page in latch_payload):
        raise TypeError("latch Check Run pagination response must contain page objects")
    latch_runs = [item for page in latch_payload for item in (page.get("check_runs") if isinstance(page.get("check_runs"), list) else [])]
    if not all(isinstance(item, Mapping) for item in latch_runs):
        raise TypeError("latch Check Run page must contain an array")
    latch_candidates = [item for item in latch_runs if item.get("name") == _LATCH_CHECK and item.get("head_sha", "").lower() == head.lower() and isinstance(item.get("app"), Mapping) and item["app"].get("id") == 15368]
    same_source = []
    for item in latch_candidates:
        details = item.get("details_url")
        ids = re.findall(r"/actions/runs/([1-9][0-9]*)/?$", urlparse(details).path) if isinstance(details, str) else []
        if len(ids) == 1 and ids[0] == source_ids[0] and not urlparse(details).query:
            same_source.append(item)
    if len(same_source) != 1:
        return "review latch Check Run for the trusted source must have exactly one matching run"
    latch = same_source[0]
    if latch.get("status") != "completed" or latch.get("conclusion") != "success":
        return "review latch Check Run is not completed successfully"
    source = _gh_json("api", f"repos/{repository}/actions/runs/{source_ids[0]}")
    if not isinstance(source, Mapping):
        return "trusted source Actions run response is invalid"
    if (
        source.get("id") != int(source_ids[0])
        or source.get("name") != "PR governance review sensor"
        or source.get("event") not in {"pull_request", "pull_request_review", "pull_request_review_comment"}
        or source.get("run_attempt") != 1
        or source.get("head_sha", "").lower() != head.lower()
        or source.get("status") != "completed"
        or source.get("conclusion") != "success"
        or not isinstance(source.get("path"), str)
        or source.get("path", "").split("@", 1)[0] != ".github/workflows/pr-governance-review-events.yml"
        or ("@" in source.get("path", "") and (
            re.fullmatch(r"[A-Za-z0-9._/-]+", source.get("path", "").split("@", 1)[1]) is None
            or source.get("path", "").split("@", 1)[1].startswith("/")
            or "//" in source.get("path", "").split("@", 1)[1]
            or any(part in {".", ".."} for part in source.get("path", "").split("@", 1)[1].split("/"))
        ))
    ):
        return "trusted source Actions run evidence does not match"
    pull_requests = source.get("pull_requests")
    if not isinstance(pull_requests, list) or len(pull_requests) != 1 or not isinstance(pull_requests[0], Mapping) or pull_requests[0].get("number") != pull_request:
        return "trusted source Actions run PR identity does not match"
    source_pr = pull_requests[0]
    source_repo = source.get("repository")
    source_base = source_pr.get("base")
    source_head = source_pr.get("head")
    if (
        not isinstance(source_repo, Mapping) or source_repo.get("full_name") != repository
        or not isinstance(source_base, Mapping) or source_base.get("sha", "").lower() != base_sha.lower()
        or source_base.get("ref") != base_branch
        or not isinstance(source_base.get("repo"), Mapping) or source_base["repo"].get("full_name") != repository
        or not isinstance(source_head, Mapping) or source_head.get("sha", "").lower() != head.lower()
        or not isinstance(source_head.get("repo"), Mapping) or source_head["repo"].get("full_name") != repository
    ):
        return "trusted source Actions run PR boundary does not match"
    latest_candidates: list[Mapping[str, object]] = []
    for event_name in ("pull_request", "pull_request_review", "pull_request_review_comment"):
        payload = _gh_json("api", "--paginate", "--slurp", f"repos/{repository}/actions/workflows/pr-governance-review-events.yml/runs?event={event_name}&per_page=100")
        if not isinstance(payload, list) or not all(isinstance(page, Mapping) for page in payload):
            raise TypeError("sensor workflow run pagination response is invalid")
        for page in payload:
            values = page.get("workflow_runs")
            if not isinstance(values, list) or not all(isinstance(value, Mapping) for value in values):
                raise TypeError("sensor workflow run page is invalid")
            for value in values:
                runs_pr = value.get("pull_requests")
                repo = value.get("repository")
                path = value.get("path", "")
                if (
                    value.get("name") == "PR governance review sensor"
                    and value.get("event") == event_name
                    and value.get("run_attempt") == 1
                    and value.get("head_sha", "").lower() == head.lower()
                    and isinstance(repo, Mapping) and repo.get("full_name") == repository
                    and isinstance(runs_pr, list) and len(runs_pr) == 1
                    and isinstance(runs_pr[0], Mapping) and runs_pr[0].get("number") == pull_request
                    and isinstance(runs_pr[0].get("base"), Mapping)
                    and runs_pr[0]["base"].get("sha", "").lower() == base_sha.lower()
                    and runs_pr[0]["base"].get("ref") == base_branch
                    and isinstance(runs_pr[0]["base"].get("repo"), Mapping)
                    and runs_pr[0]["base"]["repo"].get("full_name") == repository
                    and isinstance(runs_pr[0].get("head"), Mapping)
                    and runs_pr[0]["head"].get("sha", "").lower() == head.lower()
                    and isinstance(runs_pr[0]["head"].get("repo"), Mapping)
                    and runs_pr[0]["head"]["repo"].get("full_name") == repository
                    and isinstance(path, str) and path.split("@", 1)[0] == ".github/workflows/pr-governance-review-events.yml"
                ):
                    latest_candidates.append(value)
    if not latest_candidates:
        return "trusted source Actions run is not the latest sensor generation"
    latest = max(latest_candidates, key=lambda value: (value.get("run_number", 0), value.get("id", 0)))
    if latest.get("id") != int(source_ids[0]):
        return "trusted source Actions run is not the latest sensor generation"
    if evidence is not None:
        evidence.update({
            "protection": tuple(sorted((str(item.get("context")), item.get("app_id")) for item in checks if isinstance(item, Mapping))),
            "check": tuple(run.get(key) for key in ("id", "name", "head_sha", "external_id", "status", "conclusion", "details_url")),
            "latch": tuple(latch.get(key) for key in ("id", "name", "head_sha", "status", "conclusion", "details_url")),
            "source": tuple(source.get(key) for key in ("id", "name", "path", "event", "run_attempt", "head_sha", "status", "conclusion")),
            "source_pr": (source_pr.get("number"), source_base.get("sha"), source_base.get("ref"), source_base["repo"].get("full_name"), source_head.get("sha"), source_head["repo"].get("full_name")),
        })
    return None


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, required=True, help="pull request number")
    parser.add_argument("--repository", help="GitHub repository as OWNER/REPOSITORY")
    parser.add_argument("--expected-base-sha")
    parser.add_argument("--expected-head-sha")
    parser.add_argument("--open-pull-snapshot")
    draft_group = parser.add_mutually_exclusive_group()
    draft_group.add_argument(
        "--require-draft", dest="require_draft", action="store_true", default=True
    )
    draft_group.add_argument(
        "--allow-ready", dest="require_draft", action="store_false"
    )
    arguments = parser.parse_args(argv)
    if (arguments.expected_base_sha is None) != (arguments.expected_head_sha is None):
        parser.error("--expected-base-sha and --expected-head-sha must be provided together")
    expected_boundary = _expected_boundary(
        arguments.expected_base_sha, arguments.expected_head_sha
    )

    repository = _repository_name(arguments.repository)
    pull_request = _gh_json(
        "pr",
        "view",
        str(arguments.pr),
        "--repo",
        repository,
        "--json",
        "isDraft,baseRefOid,headRefOid,baseRefName,body,updatedAt,statusCheckRollup,reviews,author",
    )
    if not isinstance(pull_request, dict):
        raise TypeError("pull request response must be an object")
    current_reviews = pull_request.get("reviews")
    if not isinstance(current_reviews, list):
        raise TypeError("pull request reviews must be an array")
    # gh pr view exposes a bounded connection.  A full boundary (or an empty
    # compatibility response) requires explicit GraphQL cursor pagination.
    if not current_reviews or len(current_reviews) >= 100:
        pull_request["reviews"] = _reviews(repository, arguments.pr)
    comments = _paginated_api_array(
        f"repos/{repository}/issues/{arguments.pr}/comments"
    )
    threads = _review_threads(repository, arguments.pr)
    reactions = _comment_reactions(repository, comments)
    base, head = _require_boundary(
        pull_request.get("baseRefOid"), pull_request.get("headRefOid"), expected_boundary
    )
    initial_updated_at = _required_timestamp_text(
        pull_request.get("updatedAt"), "pull request updatedAt"
    )
    referenced_issues = issue_contract.referenced_issue_snapshot(
        repository=repository,
        base_sha=base,
        head_sha=head,
    )
    initial_body = pull_request.get("body")
    errors = closing_reference_errors(
        repository=repository,
        body=initial_body,
        referenced_issues=referenced_issues,
    )
    governance_evidence: dict[str, object] = {}
    if not arguments.require_draft:
        governance_error = _governance_check_error(
            repository, arguments.pr, pull_request.get("baseRefName"), base, head, governance_evidence
        )
        if governance_error is not None:
            errors.append(governance_error)
    initial_issue_identity: tuple[tuple[int, str, str, str, str], ...] = ()
    initial_closers: frozenset[int] = frozenset()
    if referenced_issues and not errors:
        open_pull_requests = (
            _open_pull_request_snapshot(arguments.open_pull_snapshot)
            if arguments.open_pull_snapshot is not None
            else _open_pull_requests(repository)
        )
        errors.extend(
            closing_open_pull_request_errors(
                repository=repository,
                current_pull_request=arguments.pr,
                referenced_issues=referenced_issues,
                open_pull_requests=open_pull_requests,
            )
        )
        if not errors:
            initial_issue_identity = _canonical_issue_identity(
                repository=repository,
                referenced_issues=referenced_issues,
            )
            initial_closers = frozenset(
                _open_pull_request_closers(
                    repository=repository,
                    issue_number=referenced_issues[0].number,
                    open_pull_requests=open_pull_requests,
                )
            )
    errors.extend(
        readiness_errors(
            pull_request,
            threads,
            comments,
            reactions,
            review_bot="chatgpt-codex-connector",
            require_draft=arguments.require_draft,
            referenced_issues=referenced_issues,
        )
    )
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    if not isinstance(initial_body, str):
        raise TypeError("pull request body must be a string")
    _verify_final_readiness_snapshot_unchanged(
        repository=repository,
        pull_request=arguments.pr,
        initial_base=base,
        initial_head=head,
        initial_body=initial_body,
        initial_updated_at=initial_updated_at,
        initial_issue_identity=initial_issue_identity,
        initial_closers=initial_closers,
        open_pull_requests=(
            open_pull_requests
            if arguments.open_pull_snapshot is not None and referenced_issues and not errors
            else None
        ),
    )
    if not arguments.require_draft:
        final_governance_evidence: dict[str, object] = {}
        governance_error = _governance_check_error(
            repository, arguments.pr, pull_request.get("baseRefName"), base, head, final_governance_evidence
        )
        if governance_error is not None or final_governance_evidence != governance_evidence:
            raise ValueError("governance evidence changed during readiness check")
    print(f"PR #{arguments.pr} is ready")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
