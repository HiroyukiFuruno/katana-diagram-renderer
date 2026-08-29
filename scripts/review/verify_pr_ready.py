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
from typing import Any


_MARKER_PATTERN = re.compile(
    r"<!--\s*krr-review\s+phase=(?P<phase>initial|final)\s+"
    r"head=(?P<head>[0-9a-fA-F]{40})\s*-->"
)
_SUCCESSFUL_CONCLUSIONS = {"SUCCESS", "NEUTRAL", "SKIPPED"}
_SELF_CHECK_NAMES = frozenset({"PR governance", "KRR / PR governance (trusted)"})
_CODEX_REVIEW_TRIGGER = re.compile(r"(?m)^\s*@codex\s+review\s*$")
_TRUSTED_REPLY_ASSOCIATIONS = frozenset({"COLLABORATOR", "MEMBER", "OWNER"})


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


def _review_is_for(
    reviews: Sequence[Mapping[str, object]],
    review_bot: str,
    head: str,
    not_before: object,
    before: object | None = None,
) -> bool:
    not_before_time = _timestamp(not_before, "marker created_at")
    if not_before_time is None:
        raise TypeError("marker created_at must be an ISO-8601 timestamp")
    before_time = _timestamp(before, "marker created_at")
    return any(
        _is_review_bot(_bot_login(review.get("author")), review_bot)
        and isinstance(review.get("commit"), Mapping)
        and review["commit"].get("oid") == head
        and (submitted_at := _timestamp(review.get("submittedAt"), "review submittedAt"))
        is not None
        and submitted_at >= not_before_time
        and (before_time is None or submitted_at < before_time)
        for review in reviews
    )


def _bot_plus_one(
    comment: Mapping[str, object],
    reactions: Mapping[int, Sequence[Mapping[str, object]]],
    review_bot: str,
    not_before: object,
    before: object | None = None,
) -> bool:
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
        return False
    edited_time = _timestamp(edited_at, "marker updated_at")
    if edited_time is None:
        return False
    if edited_time < not_before_time:
        return False

    def is_timely(reaction: Mapping[str, object]) -> bool:
        created_at = reaction.get("created_at")
        if created_at is None:
            return False
        reaction_time = _timestamp(created_at, "reaction created_at")
        return (
            reaction_time is not None
            and reaction_time >= edited_time
            and (before_time is None or reaction_time < before_time)
        )

    return any(
        reaction.get("content") == "+1"
        and _is_review_bot(_bot_login(reaction.get("user")), review_bot)
        and is_timely(reaction)
        for reaction in reactions.get(comment_id, ())
    )


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
            comment["created_at"],
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
    if not final_markers:
        errors.append("final review marker がありません")
    elif not current_final_markers:
        errors.append("final review marker が最新HEADを指していません")
    elif not any(
        _review_is_for(
            typed_reviews,
            review_bot,
            head,
            comment["created_at"],
        )
        or _bot_plus_one(
            comment,
            reactions,
            review_bot,
            comment["created_at"],
        )
        for _phase, _marker_head, comment in current_final_markers
    ):
        errors.append("final review に review bot の完了記録がありません")

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
        if not page_info["hasNextPage"]:
            return threads
        cursor = page_info["endCursor"]
        if not isinstance(cursor, str):
            raise TypeError("reviewThreads endCursor must be a string")


def _reviews(repository: str, pull_request: int) -> list[dict[str, object]]:
    """Read every pull-request review page, failing closed on invalid cursors."""

    query = """
query($owner: String!, $name: String!, $number: Int!, $cursor: String) {
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviews(first: 100, after: $cursor) {
        nodes { author { login } commit { oid } submittedAt }
        pageInfo { hasNextPage endCursor }
      }
    }
  }
}
"""
    owner, name = repository.split("/", maxsplit=1)
    reviews: list[dict[str, object]] = []
    cursor: str | None = None
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
        if page_info.get("hasNextPage") is False:
            return reviews
        cursor = page_info.get("endCursor")
        if not isinstance(cursor, str) or not cursor:
            raise TypeError("reviews endCursor must be a string")


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
        "isDraft,headRefOid,statusCheckRollup,reviews,author",
    )
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
    errors = readiness_errors(
        pull_request,
        threads,
        comments,
        reactions,
        review_bot="chatgpt-codex-connector",
        require_draft=arguments.require_draft,
    )
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PR #{arguments.pr} is ready")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
