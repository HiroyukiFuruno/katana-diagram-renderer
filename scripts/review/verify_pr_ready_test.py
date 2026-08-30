from __future__ import annotations

import sys
import unittest
from copy import deepcopy
from pathlib import Path
from unittest.mock import patch
from urllib.parse import parse_qs, urlparse

sys.path.insert(0, str(Path(__file__).parent))

import verify_pr_ready as subject


HEAD = "a" * 40
INITIAL_HEAD = "b" * 40
BOT = "chatgpt-codex-connector"


def marker(
    comment_id: int,
    phase: str,
    head: str,
    updated_at: str | None = None,
) -> dict[str, object]:
    created_at = f"2026-08-29T03:0{comment_id}:00Z"
    return {
        "id": comment_id,
        "body": f"<!-- krr-review phase={phase} head={head} -->\n@codex review",
        "created_at": created_at,
        "updated_at": updated_at or created_at,
        "user": {"login": "HiroyukiFuruno"},
    }


def successful_state() -> tuple[
    dict[str, object],
    list[dict[str, object]],
    list[dict[str, object]],
    dict[int, list[dict[str, object]]],
]:
    pull_request = {
        "isDraft": True,
        "baseRefOid": "c" * 40,
        "headRefOid": HEAD,
        "body": "Closes #64",
        "updatedAt": "2026-08-29T03:03:00Z",
        "statusCheckRollup": [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            }
        ],
        "reviews": [
            {
                "author": {"login": BOT},
                "commit": {"oid": INITIAL_HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:01:30Z",
            }
        ],
    }
    threads = [{"id": "thread-1", "isResolved": True}]
    comments = [
        marker(1, "initial", INITIAL_HEAD),
        marker(2, "final", HEAD),
    ]
    reactions = {
        1: [],
        2: [
            {
                "content": "+1",
                "created_at": "2026-08-29T03:02:30Z",
                "user": {"login": f"{BOT}[bot]"},
            }
        ],
    }
    return pull_request, threads, comments, reactions


def current_canonical_closer() -> list[dict[str, object]]:
    return [{"number": 72, "isDraft": True, "body": "Closes #64"}]


class VerifyPrReadyTest(unittest.TestCase):
    def errors(
        self,
        pull_request: dict[str, object] | None = None,
        threads: list[dict[str, object]] | None = None,
        comments: list[dict[str, object]] | None = None,
        reactions: dict[int, list[dict[str, object]]] | None = None,
        referenced_issues: tuple[subject.issue_contract.Issue, ...] = (),
    ) -> list[str]:
        default_pr, default_threads, default_comments, default_reactions = (
            successful_state()
        )
        return subject.readiness_errors(
            pull_request or default_pr,
            threads if threads is not None else default_threads,
            comments if comments is not None else default_comments,
            reactions if reactions is not None else default_reactions,
            review_bot=BOT,
            require_draft=True,
            referenced_issues=referenced_issues,
        )

    def issue(self, number: int, updated_at: str) -> subject.issue_contract.Issue:
        return subject.issue_contract.Issue(
            number=number,
            state="OPEN",
            body="Issue body",
            url=f"https://github.com/owner/repo/issues/{number}",
            updated_at=updated_at,
        )

    def test_accepts_two_phase_review_on_current_head(self) -> None:
        self.assertEqual(self.errors(), [])

    def test_rejects_final_evidence_before_referenced_issue_edit(self) -> None:
        errors = self.errors(
            referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),)
        )
        self.assertIn("参照Issue更新後", " ".join(errors))

    def test_accepts_final_evidence_after_referenced_issue_edit(self) -> None:
        _, _, comments, reactions = successful_state()
        reactions[2][0]["created_at"] = "2026-08-29T03:04:01Z"
        self.assertEqual(
            self.errors(
                comments=comments,
                reactions=reactions,
                referenced_issues=(self.issue(64, "2026-08-29T03:04:00Z"),),
            ),
            [],
        )

    def test_uses_latest_referenced_issue_edit_for_final_evidence(self) -> None:
        _, _, comments, reactions = successful_state()
        reactions[2][0]["created_at"] = "2026-08-29T03:04:01Z"
        errors = self.errors(
            comments=comments,
            reactions=reactions,
            referenced_issues=(
                self.issue(64, "2026-08-29T03:03:00Z"),
                self.issue(65, "2026-08-29T03:05:00Z"),
            ),
        )
        self.assertIn("参照Issue更新後", " ".join(errors))

    def test_rejects_malformed_referenced_issue_timestamp(self) -> None:
        errors = self.errors(
            referenced_issues=(self.issue(64, "not-a-timestamp"),)
        )
        self.assertIn("snapshot", " ".join(errors))

    def test_rejects_missing_referenced_issue_timestamp(self) -> None:
        errors = self.errors(referenced_issues=(self.issue(64, ""),))
        self.assertIn("snapshot", " ".join(errors))

    def test_closing_contract_accepts_pr_72_style_body(self) -> None:
        self.assertEqual(
            subject.closing_reference_errors(
                repository="owner/repo",
                body="Closes #64",
                referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),),
            ),
            [],
        )

    def test_closing_contract_rejects_zero_canonical_open_issues(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="",
            referenced_issues=(),
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_extra_closing_reference_without_a_canonical_issue(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Fixes #64",
            referenced_issues=(),
        )
        self.assertIn("余分=#64", " ".join(errors))

    def test_closing_contract_rejects_missing_wrong_and_refs_only_references(self) -> None:
        referenced_issues = (self.issue(64, "2026-08-29T03:03:00Z"),)
        for body in ("", "Closes #65", "Refs #64"):
            with self.subTest(body=body):
                errors = subject.closing_reference_errors(
                    repository="owner/repo",
                    body=body,
                    referenced_issues=referenced_issues,
                )
                self.assertIn("#64", " ".join(errors))

    def test_closing_contract_rejects_extra_same_repo_closing_reference(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64\nFixes #65",
            referenced_issues=(self.issue(64, "2026-08-29T03:03:00Z"),),
        )
        self.assertIn("余分=#65", " ".join(errors))

    def test_closing_contract_rejects_multiple_matching_issues(self) -> None:
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64\nFixes https://github.com/owner/repo/issues/65",
            referenced_issues=(
                self.issue(64, "2026-08-29T03:03:00Z"),
                self.issue(65, "2026-08-29T03:03:00Z"),
            ),
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_multiple_keyword_variants_and_same_repo_url(self) -> None:
        referenced_issues = (
            self.issue(64, "2026-08-29T03:03:00Z"),
            self.issue(65, "2026-08-29T03:03:00Z"),
            self.issue(66, "2026-08-29T03:03:00Z"),
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body=(
                "fixed #64\n"
                "Resolve: https://github.com/owner/repo/issues/65\n"
                "CLOSED #66\n"
                "Fixes https://github.com/other/repo/issues/67"
            ),
            referenced_issues=referenced_issues,
        )
        self.assertIn("ちょうど1件", " ".join(errors))

    def test_closing_contract_rejects_closed_canonical_issue(self) -> None:
        closed_issue = subject.issue_contract.Issue(
            number=64,
            state="CLOSED",
            body="Issue body",
            url="https://github.com/owner/repo/issues/64",
            updated_at="2026-08-29T03:03:00Z",
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64",
            referenced_issues=(closed_issue,),
        )
        self.assertIn("OPEN", " ".join(errors))

    def test_closing_contract_rejects_noncanonical_issue_url(self) -> None:
        issue = subject.issue_contract.Issue(
            number=64,
            state="OPEN",
            body="Issue body",
            url="https://github.com/example/other/issues/64",
            updated_at="2026-08-29T03:03:00Z",
        )
        errors = subject.closing_reference_errors(
            repository="owner/repo",
            body="Closes #64",
            referenced_issues=(issue,),
        )
        self.assertIn("canonical", " ".join(errors))

    def test_open_pull_request_contract_allows_only_the_current_canonical_closer(self) -> None:
        issue = self.issue(64, "2026-08-29T03:03:00Z")

        def pull_requests(count: int, *, sibling_is_draft: bool) -> list[dict[str, object]]:
            return current_canonical_closer() + [
                {
                    "number": 1_000 + index,
                    "isDraft": sibling_is_draft,
                    "body": "Closes #64",
                }
                for index in range(count)
            ]

        self.assertEqual(
            subject.closing_open_pull_request_errors(
                repository="owner/repo",
                current_pull_request=72,
                referenced_issues=(issue,),
                open_pull_requests=pull_requests(0, sibling_is_draft=False),
            ),
            [],
        )
        for sibling_is_draft in (False, True):
            with self.subTest(sibling_is_draft=sibling_is_draft):
                open_pull_requests = pull_requests(1, sibling_is_draft=sibling_is_draft)
                errors = subject.closing_open_pull_request_errors(
                    repository="owner/repo",
                    current_pull_request=72,
                    referenced_issues=(issue,),
                    open_pull_requests=open_pull_requests,
                )
                self.assertIn("open PRは自身だけ", " ".join(errors))
                self.assertIn("#72, #1000", " ".join(errors))

    def test_open_pull_requests_reads_all_pages(self) -> None:
        def payload(nodes: list[dict[str, object]], has_next_page: bool, cursor: str | None) -> dict[str, object]:
            return {
                "data": {
                    "repository": {
                        "pullRequests": {
                            "nodes": nodes,
                            "pageInfo": {
                                "hasNextPage": has_next_page,
                                "endCursor": cursor,
                            },
                        }
                    }
                }
            }

        first_page = payload(
            [{"number": 71, "isDraft": False, "body": "Closes #64"}],
            True,
            "next-page",
        )
        second_page = payload(
            [{"number": 72, "isDraft": True, "body": "Closes #64"}],
            False,
            None,
        )

        def gh_json(*arguments: str) -> object:
            if "cursor=next-page" in arguments:
                return second_page
            return first_page

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            self.assertEqual(
                [item["number"] for item in subject._open_pull_requests("owner/repo")],
                [71, 72],
            )

    def test_open_pull_requests_fails_closed_on_malformed_response(self) -> None:
        malformed = {
            "data": {
                "repository": {
                    "pullRequests": {
                        "nodes": [{"number": 72, "isDraft": True, "body": None}],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=malformed):
            with self.assertRaisesRegex(TypeError, "body"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_fails_closed_on_duplicate_numbers(self) -> None:
        first_page = {
            "data": {
                "repository": {
                    "pullRequests": {
                        "nodes": [{"number": 72, "isDraft": True, "body": "Closes #64"}],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        duplicate_page = {
            "data": {
                "repository": {
                    "pullRequests": {
                        "nodes": [{"number": 72, "isDraft": True, "body": "Closes #64"}],
                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", side_effect=[first_page, duplicate_page]):
            with self.assertRaisesRegex(TypeError, "duplicate"):
                subject._open_pull_requests("owner/repo")

    def test_open_pull_requests_fails_closed_on_repeated_cursor(self) -> None:
        first_page = {
            "data": {
                "repository": {
                    "pullRequests": {
                        "nodes": [{"number": 72, "isDraft": True, "body": "Closes #64"}],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        repeated_cursor_page = {
            "data": {
                "repository": {
                    "pullRequests": {
                        "nodes": [{"number": 73, "isDraft": True, "body": "Closes #64"}],
                        "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                    }
                }
            }
        }
        with patch.object(
            subject, "_gh_json", side_effect=[first_page, repeated_cursor_page]
        ):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._open_pull_requests("owner/repo")

    def test_rejects_ready_pr_before_the_gate(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["isDraft"] = False
        self.assertIn("Draft", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_final_review_marker_for_an_old_head(self) -> None:
        _, _, comments, _ = successful_state()
        comments[1] = marker(2, "final", INITIAL_HEAD)
        self.assertIn("最新HEAD", " ".join(self.errors(comments=comments)))

    def test_rejects_unresolved_review_threads(self) -> None:
        threads = [{"id": "thread-1", "isResolved": False}]
        self.assertIn("未resolve", " ".join(self.errors(threads=threads)))

    def test_rejects_failed_or_pending_checks(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": "",
            }
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_ignores_trusted_governance_pending_check(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance (trusted check)",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_ignores_review_latch_failure_when_regular_ci_succeeds(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance review latch",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_rejects_regular_ci_failure_alongside_review_latch_failure(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance review latch",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "FAILURE",
            },
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_pending_regular_ci_alongside_trusted_check(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "KRR / PR governance (trusted check)",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
        ]
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_keeps_legacy_governance_context_compatibility(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "PR governance",
                "status": "IN_PROGRESS",
                "conclusion": "",
            },
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "COMPLETED",
                "conclusion": "SUCCESS",
            },
        ]
        self.assertEqual(self.errors(pull_request=pull_request), [])

    def test_rejects_final_marker_without_bot_completion(self) -> None:
        self.assertIn(
            "final",
            " ".join(self.errors(reactions={1: [], 2: []})),
        )

    def test_rejects_edited_marker_with_old_bot_reaction(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:00Z"
        )
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_rejects_final_bot_reaction_when_marker_edit_time_is_missing(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1].pop("updated_at")
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_rejects_final_bot_reaction_when_marker_edit_precedes_creation(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1]["updated_at"] = "2026-08-29T03:01:00Z"
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_accepts_edited_marker_with_new_bot_reaction(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:00Z"
        )
        reactions[2][0]["created_at"] = "2026-08-29T03:04:30Z"
        self.assertEqual(self.errors(comments=comments, reactions=reactions), [])

    def test_rejects_edited_initial_marker_reused_as_final_evidence(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["submittedAt"] = "2026-08-29T03:00:30Z"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:03:00Z",
            }
        )
        comments[0]["body"] = (
            f"<!-- krr-review phase=final head={HEAD} -->\n@codex review"
        )
        comments[0]["updated_at"] = "2026-08-29T03:04:00Z"
        comments.append(
            {
                "id": 3,
                "body": (
                    f"<!-- krr-review phase=initial head={INITIAL_HEAD} -->\n"
                    "@codex review"
                ),
                "created_at": "2026-08-29T03:00:00Z",
                "updated_at": "2026-08-29T03:00:00Z",
                "user": {"login": "HiroyukiFuruno"},
            }
        )
        reactions[1] = []
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_rejects_review_submitted_in_same_second_as_marker_edit(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        comments[1]["updated_at"] = "2026-08-29T03:04:00Z"
        reactions[2] = []
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:04:00Z",
            }
        )
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_accepts_review_submitted_after_marker_edit_second(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        comments[1]["updated_at"] = "2026-08-29T03:04:00Z"
        reactions[2] = []
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:04:01Z",
            }
        )
        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                comments=comments,
                reactions=reactions,
            ),
            [],
        )

    def test_rejects_edited_marker_with_same_second_bot_reaction(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:00Z"
        )
        reactions[2][0]["created_at"] = "2026-08-29T03:04:00Z"
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_rejects_invalid_bot_reaction_timestamp(self) -> None:
        _, _, comments, reactions = successful_state()
        reactions[2][0]["created_at"] = "not-a-timestamp"
        self.assertIn(
            "final",
            " ".join(self.errors(comments=comments, reactions=reactions)),
        )

    def test_rejects_review_completed_before_its_marker(self) -> None:
        pull_request, _, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["submittedAt"] = "2026-08-29T03:00:00Z"
        self.assertIn(
            "initial",
            " ".join(self.errors(pull_request=pull_request)),
        )

    def test_rejects_dismissed_review_as_final_evidence(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["state"] = "DISMISSED"
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "DISMISSED",
                "submittedAt": "2026-08-29T03:03:30Z",
            }
        )
        reactions[2] = []
        errors = self.errors(
            pull_request=pull_request,
            comments=comments,
            reactions=reactions,
        )
        self.assertIn("initial", " ".join(errors))
        self.assertIn("final", " ".join(errors))

    def test_accepts_approved_review_as_valid_evidence(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews.append(
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "APPROVED",
                "submittedAt": "2026-08-29T03:03:30Z",
            }
        )
        self.assertEqual(
            [],
            self.errors(
                pull_request=pull_request,
                comments=comments,
                reactions=reactions,
            ),
        )

    def test_rejects_marker_without_the_codex_review_trigger(self) -> None:
        _, _, comments, reactions = successful_state()
        for comment in comments:
            body = comment["body"]
            assert isinstance(body, str)
            comment["body"] = body.replace("\n@codex review", "")
        errors = " ".join(self.errors(comments=comments, reactions=reactions))
        self.assertIn("initial", errors)
        self.assertIn("final", errors)

    def test_rejects_missing_ci_results(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["statusCheckRollup"] = []
        self.assertIn("CI", " ".join(self.errors(pull_request=pull_request)))

    def test_rejects_one_review_used_for_both_review_phases(self) -> None:
        pull_request, _, comments, _ = successful_state()
        comments[0] = marker(1, "initial", HEAD)
        pull_request["reviews"] = [
            {
                "author": {"login": BOT},
                "commit": {"oid": HEAD},
                "state": "COMMENTED",
                "submittedAt": "2026-08-29T03:03:00Z",
            }
        ]
        self.assertIn(
            "initial",
            " ".join(
                self.errors(
                    pull_request=pull_request,
                    comments=comments,
                    reactions={1: [], 2: []},
                )
            ),
        )

    def test_rejects_resolved_thread_without_author_reply(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}, "createdAt": "2026-08-29T03:00:30Z"},
                ],
            }
        ]

        errors = self.errors(
            pull_request=pull_request,
            threads=threads,
            comments=comments,
            reactions=reactions,
        )
        self.assertIn("reply", " ".join(errors).lower())

    def test_accepts_resolved_thread_with_author_reply_after_root(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        pull_request["author"] = {"login": "HiroyukiFuruno"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}, "createdAt": "2026-08-29T03:00:30Z"},
                    {"author": {"login": "HiroyukiFuruno"}, "createdAt": "2026-08-29T03:00:45Z"},
                ],
            }
        ]

        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
                reactions=reactions,
            ),
            [],
        )

    def test_accepts_resolved_thread_with_trusted_maintainer_reply_to_bot_pr(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        pull_request["author"] = {"login": "dependabot[bot]"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}},
                    {
                        "author": {"login": "maintainer"},
                        "authorAssociation": "MEMBER",
                    },
                ],
            }
        ]

        self.assertEqual(
            self.errors(
                pull_request=pull_request,
                threads=threads,
                comments=comments,
                reactions=reactions,
            ),
            [],
        )

    def test_rejects_resolved_thread_with_untrusted_reply_to_bot_pr(self) -> None:
        pull_request, _, comments, reactions = successful_state()
        pull_request["author"] = {"login": "dependabot[bot]"}
        threads = [
            {
                "id": "thread-1",
                "isResolved": True,
                "comments": [
                    {"author": {"login": "reviewer"}},
                    {
                        "author": {"login": "contributor"},
                        "authorAssociation": "CONTRIBUTOR",
                    },
                ],
            }
        ]

        errors = self.errors(
            pull_request=pull_request,
            threads=threads,
            comments=comments,
            reactions=reactions,
        )
        self.assertIn("reply", " ".join(errors).lower())

    def test_reads_issue_comments_past_the_first_api_page(self) -> None:
        pull_request = {
            "isDraft": True,
            "baseRefOid": "c" * 40,
            "headRefOid": HEAD,
            "body": "Closes #64",
            "updatedAt": "2026-08-29T03:03:00Z",
            "statusCheckRollup": [
                {
                    "__typename": "CheckRun",
                    "name": "CI",
                    "status": "COMPLETED",
                    "conclusion": "SUCCESS",
                }
            ],
            "reviews": [
                {
                    "author": {"login": BOT},
                    "commit": {"oid": INITIAL_HEAD},
                    "state": "COMMENTED",
                    "submittedAt": "2026-08-29T03:01:30Z",
                },
                {
                    "author": {"login": BOT},
                    "commit": {"oid": HEAD},
                    "state": "COMMENTED",
                    "submittedAt": "2026-08-29T03:03:30Z",
                },
            ],
        }
        filler = [
            {
                "id": index,
                "body": f"filler-{index}",
                "created_at": "2026-08-29T03:00:00Z",
                "user": {"login": "reviewer"},
            }
            for index in range(1, 31)
        ]
        all_comments = filler + [
            {
                "id": 31,
                "body": f"<!-- krr-review phase=initial head={INITIAL_HEAD} -->\n@codex review",
                "created_at": "2026-08-29T03:01:00Z",
                "user": {"login": "HiroyukiFuruno"},
            },
            {
                "id": 32,
                "body": f"<!-- krr-review phase=final head={HEAD} -->\n@codex review",
                "created_at": "2026-08-29T03:03:00Z",
                "user": {"login": "HiroyukiFuruno"},
            },
        ]

        def gh_json(*arguments: str) -> object:
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[:2] == ("api", "repos/owner/repo/issues/72/comments"):
                if "--paginate" in arguments or any("page=2" in arg for arg in arguments):
                    return all_comments
                return filler
            if arguments[:1] == ("api",) and arguments[1].startswith(
                "repos/owner/repo/issues/comments/"
            ):
                if arguments[1].endswith("/32/reactions"):
                    return [
                        {"content": "heart", "user": {"login": "reviewer"}}
                        for _ in range(100)
                    ] + [
                        {
                            "content": "+1",
                            "created_at": "2026-08-29T03:03:30Z",
                            "user": {"login": f"{BOT}[bot]"},
                        }
                    ]
                return []
            if arguments[0:2] == ("api", "graphql"):
                return {
                    "data": {
                        "repository": {
                            "pullRequest": {
                                "reviewThreads": {
                                    "nodes": [{"id": "thread-1", "isResolved": True}],
                                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                                }
                            }
                        }
                    }
                }
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_reads_reaction_past_the_first_api_page(self) -> None:
        pull_request, threads, comments, _ = successful_state()
        comments[0]["id"] = 31
        comments[1]["id"] = 32

        def gh_json(*arguments: str) -> object:
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[:2] == ("api", "repos/owner/repo/issues/72/comments"):
                return comments
            if arguments[:2] == ("api", "repos/owner/repo/issues/comments/31/reactions"):
                return []
            if arguments[:2] == ("api", "repos/owner/repo/issues/comments/32/reactions"):
                if "--paginate" in arguments or any("page=2" in arg for arg in arguments):
                    return [
                        {"content": "heart", "user": {"login": "reviewer"}}
                        for _ in range(100)
                    ] + [
                        {
                            "content": "+1",
                            "created_at": "2026-08-29T03:02:30Z",
                            "user": {"login": f"{BOT}[bot]"},
                        }
                    ]
                return []
            if arguments[0:2] == ("api", "graphql"):
                return {
                    "data": {
                        "repository": {
                            "pullRequest": {
                                "reviewThreads": {
                                    "nodes": threads,
                                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                                }
                            }
                        }
                    }
                }
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_reads_review_past_the_first_graphql_page(self) -> None:
        pull_request, threads, comments, _ = successful_state()
        pull_request["reviews"] = []

        def gh_json(*arguments: str) -> object:
            if arguments[:2] == ("pr", "view"):
                return pull_request
            if arguments[:2] == ("api", "repos/owner/repo/issues/72/comments"):
                return comments
            if arguments[:2] == ("api", "repos/owner/repo/issues/comments/1/reactions"):
                return []
            if arguments[:2] == ("api", "repos/owner/repo/issues/comments/2/reactions"):
                return [
                    {
                        "content": "+1",
                        "created_at": "2026-08-29T03:02:30Z",
                        "user": {"login": f"{BOT}[bot]"},
                    }
                ]
            if arguments[0:2] == ("api", "graphql"):
                query = next((arg for arg in arguments if "reviews" in arg), "")
                if query:
                    has_cursor = any(
                        argument in {"reviews-cursor", "cursor=reviews-cursor"}
                        for argument in arguments
                    )
                    if not has_cursor:
                        review_nodes = [
                            {
                                "author": {"login": "reviewer"},
                                "commit": {"oid": INITIAL_HEAD},
                                "state": "COMMENTED",
                                "submittedAt": "2026-08-29T03:00:00Z",
                            }
                            for _ in range(100)
                        ]
                        return {
                            "data": {
                                "repository": {
                                    "pullRequest": {
                                        "reviews": {
                                            "nodes": review_nodes,
                                            "pageInfo": {
                                                "hasNextPage": True,
                                                "endCursor": "reviews-cursor",
                                            },
                                        }
                                    }
                                }
                            }
                        }
                    return {
                        "data": {
                            "repository": {
                                "pullRequest": {
                                    "reviews": {
                                        "nodes": [
                                            {
                                                "author": {"login": BOT},
                                                "commit": {"oid": INITIAL_HEAD},
                                                "state": "COMMENTED",
                                                "submittedAt": "2026-08-29T03:01:30Z",
                                            },
                                            {
                                                "author": {"login": BOT},
                                                "commit": {"oid": HEAD},
                                                "state": "COMMENTED",
                                                "submittedAt": "2026-08-29T03:03:30Z",
                                            },
                                        ],
                                        "pageInfo": {"hasNextPage": False, "endCursor": None},
                                    }
                                }
                            }
                        }
                    }
                return {
                    "data": {
                        "repository": {
                            "pullRequest": {
                                "reviewThreads": {
                                    "nodes": threads,
                                    "pageInfo": {"hasNextPage": False, "endCursor": None},
                                }
                            }
                        }
                    }
                }
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(subject.main(["--pr", "72", "--repository", "owner/repo"]), 0)

    def test_fails_closed_when_review_thread_comments_are_truncated(self) -> None:
        payload = {
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [
                                {
                                    "id": "thread-1",
                                    "isResolved": True,
                                    "comments": {
                                        "nodes": [],
                                        "pageInfo": {
                                            "hasNextPage": True,
                                            "endCursor": "comments-cursor",
                                        },
                                    },
                                }
                            ],
                            "pageInfo": {"hasNextPage": False, "endCursor": None},
                        }
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(ValueError, "thread comments"):
                subject._review_threads("owner/repo", 72)

    def test_fails_closed_when_review_thread_cursor_repeats(self) -> None:
        payload = {
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviewThreads": {
                            "nodes": [],
                            "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                        }
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._review_threads("owner/repo", 72)

    def test_fails_closed_when_review_cursor_repeats(self) -> None:
        payload = {
            "data": {
                "repository": {
                    "pullRequest": {
                        "reviews": {
                            "nodes": [],
                            "pageInfo": {"hasNextPage": True, "endCursor": "again"},
                        }
                    }
                }
            }
        }
        with patch.object(subject, "_gh_json", return_value=payload):
            with self.assertRaisesRegex(TypeError, "endCursor"):
                subject._reviews("owner/repo", 72)

    def test_rejects_boundary_changed_during_readiness_check(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})
        changed_pull_request = dict(pull_request)
        changed_pull_request["headRefOid"] = "d" * 40

        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_malformed_expected_snapshot_sha(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected base and head"):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "not-a-sha",
                    "--expected-head-sha",
                    HEAD,
                ]
            )

    def test_rejects_malformed_expected_head_snapshot_sha(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected base and head"):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "c" * 40,
                    "--expected-head-sha",
                    "not-a-sha",
                ]
            )

    def test_rejects_missing_expected_snapshot_sha(self) -> None:
        with self.assertRaises(SystemExit):
            subject.main(
                [
                    "--pr",
                    "72",
                    "--repository",
                    "owner/repo",
                    "--expected-base-sha",
                    "c" * 40,
                ]
            )

    def test_rejects_initial_snapshot_different_from_expected(self) -> None:
        pull_request, _, _, _ = successful_state()
        pull_request["baseRefOid"] = "d" * 40
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=[]
        ), patch.object(subject, "_review_threads", return_value=[]), patch.object(
            subject, "_comment_reactions", return_value={}
        ):
            with self.assertRaisesRegex(ValueError, "initial base/head"):
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                )

    def test_rejects_success_final_snapshot_different_from_expected(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["headRefOid"] = "d" * 40
        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed|snapshot"):
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                )

    def test_accepts_matching_start_and_final_snapshots(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                ),
                0,
            )

    def test_rejects_issue_edit_between_identical_pr_boundaries(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        initial_issue = self.issue(64, "2026-08-29T03:00:00Z")
        edited_issue = self.issue(64, "2026-08-29T03:03:00Z")

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            side_effect=[(initial_issue,), (edited_issue,)],
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "canonical Issue snapshot changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_issue_body_change_with_the_same_updated_at(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        initial_issue = self.issue(64, "2026-08-29T03:00:00Z")
        changed_issue = subject.issue_contract.Issue(
            number=64,
            state="OPEN",
            body="Changed Issue body",
            url="https://github.com/owner/repo/issues/64",
            updated_at="2026-08-29T03:00:00Z",
        )

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            side_effect=[(initial_issue,), (changed_issue,)],
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "canonical Issue snapshot changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_pr_updated_at_change_after_an_aba_body_mutation(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["updatedAt"] = "2026-08-29T03:04:00Z"

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, changed_pull_request]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(subject, "_comment_reactions", return_value=reactions), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "updatedAt changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rechecks_ci_status_rollup_immediately_before_success(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        final_snapshot = deepcopy(pull_request)
        final_snapshot["statusCheckRollup"] = [
            {
                "__typename": "CheckRun",
                "name": "CI",
                "status": "IN_PROGRESS",
                "conclusion": None,
            }
        ]

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, final_snapshot]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(subject, "_comment_reactions", return_value=reactions), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "CI status changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_body_change_after_initial_closing_contract(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        changed_pull_request = dict(pull_request)
        changed_pull_request["body"] = "Closes #65"

        with patch.object(
            subject, "_gh_json", side_effect=[pull_request, changed_pull_request]
        ), patch.object(subject, "_paginated_api_array", return_value=comments), patch.object(
            subject, "_review_threads", return_value=threads
        ), patch.object(subject, "_comment_reactions", return_value=reactions), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "pull request body changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_rejects_new_open_closer_after_initial_contract(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        initial_open_pull_requests = current_canonical_closer()
        changed_open_pull_requests = initial_open_pull_requests + [
            {"number": 73, "isDraft": True, "body": "Fixes #64"}
        ]

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract,
            "referenced_issue_snapshot",
            return_value=(self.issue(64, "2026-08-29T03:00:00Z"),),
        ), patch.object(
            subject,
            "_open_pull_requests",
            side_effect=[initial_open_pull_requests, changed_open_pull_requests],
        ):
            with self.assertRaisesRegex(ValueError, "open PR closer set changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_existing_readiness_error_is_not_masked_by_snapshot_refence(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        pull_request["isDraft"] = False
        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged") as verify_snapshot:
            self.assertEqual(
                subject.main(
                    [
                        "--pr",
                        "72",
                        "--repository",
                        "owner/repo",
                        "--expected-base-sha",
                        "c" * 40,
                        "--expected-head-sha",
                        HEAD,
                    ]
                ),
                1,
            )
            verify_snapshot.assert_not_called()

    def test_pr_ready_check_wires_one_snapshot_to_both_readiness_gates(self) -> None:
        justfile = (Path(__file__).parents[2] / "Justfile").read_text(encoding="utf-8")
        check_start = justfile.index("pr-ready-check pr:")
        check_end = justfile.index("\n\n# ", check_start)
        check = justfile[check_start:check_end]
        self.assertIn("verify_push_issue.py --pr-number \"$pr\" --pr-base-sha \"$base_sha\" --pr-head-sha \"$head_sha\"", check)
        self.assertIn("baseRefOid,headRefOid,headRefName,isDraft", check)
        self.assertIn('"require-draft" if fields[5] else "allow-ready"', check)
        self.assertIn('gh repo view --json nameWithOwner', check)
        self.assertIn('gh api graphql -f query=', check)
        self.assertIn('target{__typename ... on Commit {oid}}', check)
        self.assertIn('target.get("__typename") == "Commit"', check)
        self.assertIn('--trusted-default-sha "$trusted_default_sha"', check)
        self.assertIn("verify_pr_ready.py --pr \"$pr\" --repository \"$repository\" \"--$readiness_mode\" --expected-base-sha \"$base_sha\" --expected-head-sha \"$head_sha\"", check)
        self.assertLess(check.index("verify_push_issue.py"), check.index("verify_pr_ready.py"))

    def test_rejects_base_changed_with_head_unchanged_during_readiness_check(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})
        changed_pull_request = dict(pull_request)
        changed_pull_request["baseRefOid"] = "e" * 40

        snapshot_count = 0

        def two_snapshots(*arguments: str) -> object:
            nonlocal snapshot_count
            self.assertEqual(arguments[:2], ("pr", "view"))
            snapshot_count += 1
            return pull_request if snapshot_count == 1 else changed_pull_request

        with patch.object(subject, "_gh_json", side_effect=two_snapshots), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            with self.assertRaisesRegex(ValueError, "base/head changed"):
                subject.main(["--pr", "72", "--repository", "owner/repo"])

    def test_fails_closed_when_final_boundary_response_is_not_an_object(self) -> None:
        with patch.object(subject, "_gh_json", return_value=[]):
            with self.assertRaisesRegex(TypeError, "boundary response"):
                subject._verify_pr_boundary_unchanged(
                    "owner/repo", 72, "c" * 40, "a" * 40
                )

    def test_accepts_unchanged_boundary_after_readiness_check(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64"})

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ):
            self.assertEqual(
                subject.main(["--pr", "72", "--repository", "owner/repo"]), 0
            )

    def test_does_not_fetch_final_boundary_when_readiness_has_errors(self) -> None:
        pull_request, threads, comments, reactions = successful_state()
        pull_request.update({"baseRefOid": "c" * 40, "body": "Closes #64", "isDraft": False})

        with patch.object(subject, "_gh_json", return_value=pull_request), patch.object(
            subject, "_paginated_api_array", return_value=comments
        ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
            subject, "_comment_reactions", return_value=reactions
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=(self.issue(64, "2026-08-29T03:00:00Z"),)
        ), patch.object(subject, "_open_pull_requests", return_value=current_canonical_closer()
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged") as verify_snapshot:
            self.assertEqual(
                subject.main(["--pr", "72", "--repository", "owner/repo"]), 1
            )
            verify_snapshot.assert_not_called()


class StrictGovernanceCheckRunTest(unittest.TestCase):
    """Contract fixtures for the non-Draft trusted Check Run gate."""

    repository = "owner/repo"
    pull_request = 72
    base = "c" * 40
    head = HEAD
    branch = "master"
    app_id = 42
    source_run_id = 901

    def _source(self, *, event: str = "pull_request_review") -> dict[str, object]:
        return {
            "id": self.source_run_id,
            "name": "PR governance review sensor",
            "path": ".github/workflows/pr-governance-review-events.yml@refs/pull/72/merge",
            "event": event,
            "run_number": 1,
            "run_attempt": 1,
            "head_sha": self.head,
            "status": "completed",
            "conclusion": "success",
            "repository": {"full_name": self.repository},
            "pull_requests": [
                {
                    "number": self.pull_request,
                    "base": {
                        "sha": self.base,
                        "ref": self.branch,
                        "repo": {"full_name": self.repository},
                    },
                    "head": {
                        "sha": self.head,
                        "repo": {"full_name": self.repository},
                    },
                }
            ],
        }

    def _run(self) -> dict[str, object]:
        return {
            "id": 101,
            "name": subject._TRUSTED_CHECK,
            "head_sha": self.head,
            "app": {"id": self.app_id},
            "status": "completed",
            "conclusion": "success",
            "external_id": f"krr-governance/v1/{self.head}",
            "details_url": (
                "https://github.com/owner/repo/actions/runs/123"
                f"?source_run_id={self.source_run_id}"
            ),
        }

    def _latch_run(self) -> dict[str, object]:
        return {
            "id": 201,
            "name": subject._LATCH_CHECK,
            "head_sha": self.head,
            "app": {"id": 15368},
            "status": "completed",
            "conclusion": "success",
            "details_url": (
                f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}"
            ),
        }

    def _protection(self, checks: list[dict[str, object]] | None = None) -> dict[str, object]:
        return {
            "checks": checks
            if checks is not None
            else [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ]
        }

    def _gate(
        self,
        *,
        pages: list[dict[str, object]] | None = None,
        latch_pages: list[dict[str, object]] | None = None,
        protection: dict[str, object] | None = None,
        source: dict[str, object] | None = None,
        source_history: list[dict[str, object]] | None = None,
        source_history_pages: list[list[dict[str, object]]] | None = None,
        exclude_trusted_governance_check: bool = False,
    ) -> str | None:
        check_pages = pages if pages is not None else [{"check_runs": [self._run()]}]
        latch_check_pages = (
            latch_pages
            if latch_pages is not None
            else [{"check_runs": [self._latch_run()]}]
        )
        source_run = source if source is not None else self._source()
        history_runs = source_history if source_history is not None else [source_run]
        history_pages = (
            source_history_pages
            if source_history_pages is not None
            else [history_runs]
        )
        required = protection if protection is not None else self._protection()

        def gh_json(*arguments: str) -> object:
            endpoint = next(
                (
                    argument
                    for argument in arguments
                    if argument.startswith(f"repos/{self.repository}/")
                ),
                "",
            )
            if endpoint.endswith("/protection/required_status_checks"):
                return required
            if "/check-runs?" in endpoint:
                if "review" in endpoint and "latch" in endpoint:
                    return latch_check_pages
                if exclude_trusted_governance_check:
                    raise AssertionError("internal mode must not read the trusted Check Run")
                return check_pages
            if endpoint in {
                f"repos/{self.repository}/actions/runs/{self.source_run_id}",
                f"repos/{self.repository}/actions/runs/{source_run['id']}",
            }:
                return source_run
            if endpoint.startswith(
                f"repos/{self.repository}/actions/workflows/"
                "pr-governance-review-events.yml/runs?"
            ):
                query = parse_qs(urlparse(endpoint).query)
                requested_events = query.get("event", [])
                return [
                    {
                        "workflow_runs": [
                            run
                            for run in page
                            if not requested_events or run.get("event") in requested_events
                        ]
                    }
                    for page in history_pages
                ]
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json):
            return subject._governance_check_error(
                self.repository,
                self.pull_request,
                self.branch,
                self.base,
                self.head,
                exclude_trusted_governance_check=exclude_trusted_governance_check,
            )

    def _allow_ready(
        self, sources: list[dict[str, object]], *, exclude_trusted_governance_check: bool = False
    ) -> int:
        pull = {
            "isDraft": False,
            "baseRefOid": self.base,
            "headRefOid": self.head,
            "baseRefName": self.branch,
            "body": "Closes #64",
            "updatedAt": "2026-08-29T03:03:00Z",
            "statusCheckRollup": [],
            "reviews": [{}],
        }
        source_queue = list(sources)
        latest_source = source_queue[0]

        def gh_json(*arguments: str) -> object:
            nonlocal latest_source
            if arguments[:2] == ("pr", "view"):
                return pull
            endpoint = next(
                (
                    argument
                    for argument in arguments
                    if argument.startswith(f"repos/{self.repository}/")
                ),
                "",
            )
            if endpoint.endswith("/protection/required_status_checks"):
                return self._protection()
            if "/check-runs?" in endpoint:
                if "review" in endpoint and "latch" in endpoint:
                    return [{"check_runs": [self._latch_run()]}]
                if exclude_trusted_governance_check:
                    raise AssertionError("internal mode must not read the trusted Check Run")
                return [{"check_runs": [self._run()]}]
            if endpoint == f"repos/{self.repository}/actions/runs/{self.source_run_id}":
                if not source_queue:
                    raise AssertionError("unexpected extra source-run read")
                latest_source = source_queue.pop(0)
                return latest_source
            if endpoint.startswith(
                f"repos/{self.repository}/actions/workflows/"
                "pr-governance-review-events.yml/runs?"
            ):
                query = parse_qs(urlparse(endpoint).query)
                requested_events = query.get("event", [])
                return [{
                    "workflow_runs": [
                        latest_source
                    ] if not requested_events or latest_source.get("event") in requested_events else []
                }]
            raise AssertionError(f"unexpected gh call: {arguments}")

        with patch.object(subject, "_gh_json", side_effect=gh_json), patch.object(
            subject, "_paginated_api_array", return_value=[]
        ), patch.object(subject, "_review_threads", return_value=[]), patch.object(
            subject, "_comment_reactions", return_value={}
        ), patch.object(
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
        ), patch.object(subject, "closing_reference_errors", return_value=[]), patch.object(
            subject, "readiness_errors", return_value=[]
        ), patch.object(subject, "_verify_final_readiness_snapshot_unchanged"):
            return subject.main(
                [
                    "--pr",
                    str(self.pull_request),
                    "--repository",
                    self.repository,
                    "--allow-ready",
                    *(
                        ["--exclude-trusted-governance-check"]
                        if exclude_trusted_governance_check
                        else []
                    ),
                ]
            )

    def test_draft_gate_does_not_require_trusted_or_latch_check_runs(self) -> None:
        for state, governance_checks in (
            ("absent", []),
            (
                "pending",
                [
                    {
                        "__typename": "CheckRun",
                        "name": subject._TRUSTED_CHECK,
                        "status": "IN_PROGRESS",
                        "conclusion": None,
                    },
                    {
                        "__typename": "CheckRun",
                        "name": subject._LATCH_CHECK,
                        "status": "COMPLETED",
                        "conclusion": "FAILURE",
                    },
                ],
            ),
        ):
            with self.subTest(state=state):
                pull, threads, comments, reactions = successful_state()
                pull["statusCheckRollup"] = [
                    {
                        "__typename": "CheckRun",
                        "name": "CI",
                        "status": "COMPLETED",
                        "conclusion": "SUCCESS",
                    },
                    *governance_checks,
                ]
                with patch.object(subject, "_gh_json", return_value=pull), patch.object(
                    subject, "_paginated_api_array", return_value=comments
                ), patch.object(subject, "_review_threads", return_value=threads), patch.object(
                    subject, "_comment_reactions", return_value=reactions
                ), patch.object(
                    subject.issue_contract, "referenced_issue_snapshot", return_value=()
                ), patch.object(subject, "closing_reference_errors", return_value=[]), patch.object(
                    subject, "_verify_final_readiness_snapshot_unchanged"
                ), patch.object(subject, "_governance_check_error") as governance:
                    self.assertEqual(
                        subject.main(["--pr", "72", "--repository", self.repository]), 0
                    )
                governance.assert_not_called()

    def test_allow_ready_accepts_exact_trusted_check_and_all_supported_sensor_events(self) -> None:
        for event in (
            "pull_request",
            "pull_request_review",
            "pull_request_review_comment",
        ):
            with self.subTest(event=event):
                self.assertIsNone(self._gate(source=self._source(event=event)))
        self.assertEqual(self._allow_ready([self._source(), self._source()]), 0)

    def test_internal_writer_mode_excludes_only_the_trusted_check_output(self) -> None:
        self.assertIsNone(self._gate(exclude_trusted_governance_check=True))
        self.assertEqual(
            self._allow_ready(
                [self._source(), self._source()],
                exclude_trusted_governance_check=True,
            ),
            0,
        )

    def test_internal_writer_mode_requires_allow_ready(self) -> None:
        with self.assertRaises(SystemExit):
            subject.main(
                [
                    "--pr",
                    str(self.pull_request),
                    "--repository",
                    self.repository,
                    "--exclude-trusted-governance-check",
                ]
            )

    def test_governance_check_rejects_missing_invalid_or_ambiguous_trusted_check_runs(self) -> None:
        invalid_runs: dict[str, list[dict[str, object]]] = {
            "absent": [],
            "pending": [{**self._run(), "status": "in_progress", "conclusion": None}],
            "failure": [{**self._run(), "conclusion": "failure"}],
            "duplicate": [self._run(), {**self._run(), "id": 102}],
            "foreign_app": [{**self._run(), "app": {"id": self.app_id + 1}}],
            "wrong_head": [{**self._run(), "head_sha": "b" * 40}],
            "wrong_external": [{**self._run(), "external_id": "krr-governance/v1/wrong"}],
            "duplicate_source_query": [
                {
                    **self._run(),
                    "details_url": "https://github.com/owner/repo/actions/runs/123?source_run_id=901&source_run_id=902",
                }
            ],
        }
        for name, runs in invalid_runs.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(pages=[{"check_runs": runs}]))

    def test_governance_check_requires_exact_branch_protection_app_bindings(self) -> None:
        variants = {
            "trusted_missing": [{"context": subject._LATCH_CHECK, "app_id": 15368}],
            "trusted_duplicate": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
            "trusted_unbound": [
                {"context": subject._TRUSTED_CHECK, "app_id": None},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
            "latch_wrong_app": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15369},
            ],
            "latch_duplicate": [
                {"context": subject._TRUSTED_CHECK, "app_id": self.app_id},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
                {"context": subject._LATCH_CHECK, "app_id": 15368},
            ],
        }
        for name, checks in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(protection=self._protection(checks)))

    def test_allow_ready_binds_latch_to_the_trusted_source_run(self) -> None:
        invalid_latches: dict[str, list[dict[str, object]]] = {
            "absent": [],
            "pending": [{**self._latch_run(), "status": "in_progress", "conclusion": None}],
            "failure": [{**self._latch_run(), "conclusion": "failure"}],
            "foreign_app": [{**self._latch_run(), "app": {"id": 15369}}],
            "wrong_head": [{**self._latch_run(), "head_sha": "b" * 40}],
            "wrong_source": [
                {
                    **self._latch_run(),
                    "details_url": f"https://github.com/{self.repository}/actions/runs/902",
                }
            ],
            "duplicate_for_source": [self._latch_run(), {**self._latch_run(), "id": 202}],
        }
        for name, pages in invalid_latches.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(latch_pages=[{"check_runs": pages}]))
        older_latch = {
            **self._latch_run(),
            "id": 202,
            "details_url": f"https://github.com/{self.repository}/actions/runs/900",
        }
        self.assertIsNone(
            self._gate(latch_pages=[{"check_runs": [older_latch, self._latch_run()]}])
        )

    def test_latch_url_accepts_exact_run_and_job_urls_only(self) -> None:
        run_url = f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}"
        self.assertEqual(
            subject._latch_source_run_id(run_url, self.repository),
            str(self.source_run_id),
        )
        self.assertEqual(
            subject._latch_source_run_id(
                f"{run_url}/job/123", self.repository
            ),
            str(self.source_run_id),
        )
        for url in (
            f"https://github.com/other/repo/actions/runs/{self.source_run_id}",
            f"https://evil.example/{self.repository}/actions/runs/{self.source_run_id}",
            f"https://github.com@evil.example/{self.repository}/actions/runs/{self.source_run_id}",
            f"https://github.com:443/{self.repository}/actions/runs/{self.source_run_id}",
            f"{run_url}?next=1",
            f"{run_url}#fragment",
            f"{run_url}/job/0",
            f"{run_url}/extra",
        ):
            with self.subTest(url=url):
                self.assertIsNone(subject._latch_source_run_id(url, self.repository))

    def test_latch_url_uses_the_configured_github_server_exactly(self) -> None:
        url = f"https://ghe.example/{self.repository}/actions/runs/{self.source_run_id}"
        with patch.dict(subject.os.environ, {"GITHUB_SERVER_URL": "https://ghe.example"}):
            self.assertEqual(
                subject._latch_source_run_id(url, self.repository),
                str(self.source_run_id),
            )
            self.assertIsNone(
                subject._latch_source_run_id(
                    f"https://github.com/{self.repository}/actions/runs/{self.source_run_id}",
                    self.repository,
                )
            )

    def test_latch_job_url_accepts_only_the_current_trusted_source(self) -> None:
        current = {
            **self._latch_run(),
            "details_url": (
                f"https://github.com/{self.repository}/actions/runs/"
                f"{self.source_run_id}/job/456"
            ),
        }
        old = {
            **self._latch_run(),
            "id": 202,
            "details_url": f"https://github.com/{self.repository}/actions/runs/900/job/456",
        }
        self.assertIsNone(self._gate(latch_pages=[{"check_runs": [old, current]}]))

    def test_internal_mode_binds_latch_to_the_unique_latest_sensor_generation(self) -> None:
        old = self._source()
        latest = self._source(event="pull_request_review_comment")
        latest.update({"id": 902, "run_number": 2})
        latch = {
            **self._latch_run(),
            "details_url": f"https://github.com/{self.repository}/actions/runs/902/job/456",
        }
        self.assertIsNone(
            self._gate(
                source=latest,
                source_history=[old, latest],
                latch_pages=[{"check_runs": [latch]}],
                exclude_trusted_governance_check=True,
            )
        )

    def test_internal_mode_rejects_ambiguous_latest_sensor_generation(self) -> None:
        latest_a = self._source(event="pull_request_review")
        latest_b = self._source(event="pull_request_review_comment")
        latest_a.update({"id": 902, "run_number": 2})
        latest_b.update({"id": 902, "run_number": 2})
        self.assertIsNotNone(
            self._gate(
                source_history=[latest_a, latest_b],
                exclude_trusted_governance_check=True,
            )
        )

    def test_sensor_history_fails_closed_on_a_truncated_page(self) -> None:
        with patch.object(
            subject,
            "_gh_json",
            return_value=[{"workflow_runs": [], "truncated": True}],
        ):
            with self.assertRaisesRegex(TypeError, "sensor workflow run page is truncated"):
                subject._latest_sensor_generation(
                    repository=self.repository,
                    pull_request=self.pull_request,
                    base_branch=self.branch,
                    base_sha=self.base,
                    head=self.head,
                )

    def test_governance_check_rejects_mismatched_source_run_identity(self) -> None:
        variants: dict[str, dict[str, object]] = {}
        for field, value in (
            ("id", self.source_run_id + 1),
            ("name", "other workflow"),
            ("path", ".github/workflows/other.yml@master"),
            ("event", "workflow_dispatch"),
            ("run_attempt", 2),
            ("status", "in_progress"),
            ("conclusion", "failure"),
            ("repository", {"full_name": "other/repo"}),
            ("head_sha", "b" * 40),
        ):
            source = self._source()
            source[field] = value
            variants[field] = source
        for name, source in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(source=source))

    def test_governance_check_rejects_mismatched_source_pr_binding(self) -> None:
        variants: dict[str, dict[str, object]] = {}
        for name, path, value in (
            ("number", ("number",), self.pull_request + 1),
            ("base_sha", ("base", "sha"), "d" * 40),
            ("base_ref", ("base", "ref"), "release"),
            ("base_repo", ("base", "repo", "full_name"), "other/repo"),
            ("head_sha", ("head", "sha"), "b" * 40),
            ("head_repo", ("head", "repo", "full_name"), "other/repo"),
        ):
            source = self._source()
            current: dict[str, object] = source["pull_requests"][0]  # type: ignore[index]
            for key in path[:-1]:
                current = current[key]  # type: ignore[assignment,index]
            current[path[-1]] = value
            variants[name] = source
        for name, source in variants.items():
            with self.subTest(name=name):
                self.assertIsNotNone(self._gate(source=source))

    def test_governance_check_reads_a_matching_run_from_page_two(self) -> None:
        self.assertIsNone(
            self._gate(pages=[{"check_runs": []}, {"check_runs": [self._run()]}])
        )

    def test_governance_check_reads_a_matching_latch_from_page_two(self) -> None:
        self.assertIsNone(
            self._gate(latch_pages=[{"check_runs": []}, {"check_runs": [self._latch_run()]}])
        )

    def test_governance_check_rejects_a_trusted_source_that_is_not_latest(self) -> None:
        for name, status, conclusion in (
            ("requested", "queued", None),
            ("in_progress", "in_progress", None),
            ("completed", "completed", "success"),
        ):
            with self.subTest(name=name):
                newer = self._source(event="pull_request_review_comment")
                newer.update({
                    "id": self.source_run_id + 1,
                    "run_number": 2,
                    "status": status,
                    "conclusion": conclusion,
                })
                self.assertIsNotNone(
                    self._gate(source_history_pages=[[self._source()], [newer]])
                )

    def test_governance_check_rejects_a_non_success_latest_snapshot_for_the_same_run(self) -> None:
        stale = self._source()
        stale.update({"status": "completed", "conclusion": "failure"})
        self.assertIsNotNone(self._gate(source_history=[stale]))

    def test_allow_ready_rejects_source_base_change_between_initial_and_final_evidence(self) -> None:
        changed = deepcopy(self._source())
        changed["pull_requests"][0]["base"]["sha"] = "d" * 40  # type: ignore[index]
        with self.assertRaisesRegex(ValueError, "governance evidence changed"):
            self._allow_ready([self._source(), changed])


if __name__ == "__main__":
    unittest.main()
