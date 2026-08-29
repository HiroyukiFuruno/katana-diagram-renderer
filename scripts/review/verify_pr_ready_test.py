from __future__ import annotations

import sys
import unittest
from pathlib import Path
from unittest.mock import patch

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
        "body": "",
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

    def test_closing_contract_accepts_empty_sets_and_rejects_extra_closing_reference(self) -> None:
        self.assertEqual(
            subject.closing_reference_errors(
                repository="owner/repo",
                body="",
                referenced_issues=(),
            ),
            [],
        )
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

    def test_closing_contract_accepts_exact_same_repo_issue_set(self) -> None:
        self.assertEqual(
            subject.closing_reference_errors(
                repository="owner/repo",
                body="Closes #64\nFixes https://github.com/owner/repo/issues/65",
                referenced_issues=(
                    self.issue(64, "2026-08-29T03:03:00Z"),
                    self.issue(65, "2026-08-29T03:03:00Z"),
                ),
            ),
            [],
        )

    def test_closing_contract_accepts_keyword_variants_and_same_repo_url(self) -> None:
        referenced_issues = (
            self.issue(64, "2026-08-29T03:03:00Z"),
            self.issue(65, "2026-08-29T03:03:00Z"),
            self.issue(66, "2026-08-29T03:03:00Z"),
        )
        self.assertEqual(
            subject.closing_reference_errors(
                repository="owner/repo",
                body=(
                    "fixed #64\n"
                    "Resolve: https://github.com/owner/repo/issues/65\n"
                    "CLOSED #66\n"
                    "Fixes https://github.com/other/repo/issues/67"
                ),
                referenced_issues=referenced_issues,
            ),
            [],
        )

    def test_closing_target_capacity_allows_256_and_rejects_257(self) -> None:
        issue = self.issue(64, "2026-08-29T03:03:00Z")

        def pull_requests(count: int) -> list[dict[str, object]]:
            return [
                {"number": 1_000 + index, "isDraft": False, "body": "Closes #64"}
                for index in range(count)
            ]

        self.assertEqual(
            subject.closing_target_capacity_errors(
                repository="owner/repo",
                current_pull_request=72,
                referenced_issues=(issue,),
                open_pull_requests=pull_requests(255),
            ),
            [],
        )
        errors = subject.closing_target_capacity_errors(
            repository="owner/repo",
            current_pull_request=72,
            referenced_issues=(issue,),
            open_pull_requests=pull_requests(256),
        )
        self.assertIn("257", " ".join(errors))

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
                "name": "KRR / PR governance (trusted)",
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
                "name": "KRR / PR governance (trusted)",
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
            "body": "",
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
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
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
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
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
            subject.issue_contract, "referenced_issue_snapshot", return_value=()
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


if __name__ == "__main__":
    unittest.main()
