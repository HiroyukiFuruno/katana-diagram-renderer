#!/usr/bin/env python3
"""Verify that a pull request has completed the required review workflow."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
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
        "KRR / PR governance (trusted)",
        "KRR / PR governance review latch",
    }
)
_CODEX_REVIEW_TRIGGER = re.compile(r"(?m)^\s*@codex\s+review\s*$")
_TRUSTED_REPLY_ASSOCIATIONS = frozenset({"COLLABORATOR", "MEMBER", "OWNER"})
_MAX_CLOSING_ISSUE_TARGETS = 256


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
    """Require every Issue found in base..HEAD commits to close from the PR body."""

    if not isinstance(body, str):
        raise TypeError("pull request body must be a string")
    referenced_numbers = {issue.number for issue in referenced_issues}
    closing_numbers = issue_contract.closing_issue_numbers(body, repository)
    missing = sorted(referenced_numbers - closing_numbers)
    extra = sorted(closing_numbers - referenced_numbers)
    if not missing and not extra:
        return []
    details: list[str] = []
    if missing:
        details.append(
            "不足=" + ", ".join(f"#{number}" for number in missing)
        )
    if extra:
        details.append(
            "余分=" + ", ".join(f"#{number}" for number in extra)
        )
    return [
        "PR本文のGitHub closing Issue集合がcommit範囲参照Issue集合と一致しません: "
        + "; ".join(details)
    ]


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


def closing_target_capacity_errors(
    *,
    repository: str,
    current_pull_request: int,
    referenced_issues: Sequence[issue_contract.Issue],
    open_pull_requests: Sequence[Mapping[str, object]],
) -> list[str]:
    """Keep the normal closing-PR flow within GitHub's 256-target limit."""

    errors: list[str] = []
    for issue in referenced_issues:
        non_draft_targets = 0
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
            if number == current_pull_request or is_draft:
                continue
            if issue.number in issue_contract.closing_issue_numbers(body, repository):
                non_draft_targets += 1
        targets = non_draft_targets + 1
        if targets > _MAX_CLOSING_ISSUE_TARGETS:
            errors.append(
                f"Issue #{issue.number}のclosing PR target数が{_MAX_CLOSING_ISSUE_TARGETS}を超えます: {targets}"
            )
    return errors


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


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pr", type=int, required=True, help="pull request number")
    parser.add_argument("--repository", help="GitHub repository as OWNER/REPOSITORY")
    draft_group = parser.add_mutually_exclusive_group()
    draft_group.add_argument(
        "--require-draft", dest="require_draft", action="store_true", default=True
    )
    draft_group.add_argument(
        "--allow-ready", dest="require_draft", action="store_false"
    )
    arguments = parser.parse_args(argv)

    repository = _repository_name(arguments.repository)
    pull_request = _gh_json(
        "pr",
        "view",
        str(arguments.pr),
        "--repo",
        repository,
        "--json",
        "isDraft,baseRefOid,headRefOid,body,statusCheckRollup,reviews,author",
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
    base = pull_request.get("baseRefOid")
    head = pull_request.get("headRefOid")
    if not isinstance(base, str) or not isinstance(head, str):
        raise TypeError("pull request baseRefOid/headRefOid must be strings")
    referenced_issues = issue_contract.referenced_issue_snapshot(
        repository=repository,
        base_sha=base,
        head_sha=head,
    )
    errors = closing_reference_errors(
        repository=repository,
        body=pull_request.get("body"),
        referenced_issues=referenced_issues,
    )
    if referenced_issues and not errors:
        errors.extend(
            closing_target_capacity_errors(
                repository=repository,
                current_pull_request=arguments.pr,
                referenced_issues=referenced_issues,
                open_pull_requests=_open_pull_requests(repository),
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
    _verify_pr_boundary_unchanged(repository, arguments.pr, base, head)
    print(f"PR #{arguments.pr} is ready")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
