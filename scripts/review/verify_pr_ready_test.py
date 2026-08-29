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
        "headRefOid": HEAD,
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
        )

    def test_accepts_two_phase_review_on_current_head(self) -> None:
        self.assertEqual(self.errors(), [])

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

    def test_accepts_edited_marker_with_new_bot_reaction(self) -> None:
        _, _, comments, reactions = successful_state()
        comments[1] = marker(
            2, "final", HEAD, updated_at="2026-08-29T03:04:00Z"
        )
        reactions[2][0]["created_at"] = "2026-08-29T03:04:30Z"
        self.assertEqual(self.errors(comments=comments, reactions=reactions), [])

    def test_rejects_review_completed_before_its_marker(self) -> None:
        pull_request, _, _, _ = successful_state()
        reviews = pull_request["reviews"]
        assert isinstance(reviews, list)
        reviews[0]["submittedAt"] = "2026-08-29T03:00:00Z"
        self.assertIn(
            "initial",
            " ".join(self.errors(pull_request=pull_request)),
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
            "headRefOid": HEAD,
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
                    "submittedAt": "2026-08-29T03:01:30Z",
                },
                {
                    "author": {"login": BOT},
                    "commit": {"oid": HEAD},
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

        with patch.object(subject, "_gh_json", side_effect=gh_json):
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

        with patch.object(subject, "_gh_json", side_effect=gh_json):
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
                                                "submittedAt": "2026-08-29T03:01:30Z",
                                            },
                                            {
                                                "author": {"login": BOT},
                                                "commit": {"oid": HEAD},
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

        with patch.object(subject, "_gh_json", side_effect=gh_json):
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


if __name__ == "__main__":
    unittest.main()
