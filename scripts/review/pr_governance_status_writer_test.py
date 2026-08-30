from __future__ import annotations

import importlib.util
import json
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


ROOT = Path(__file__).parents[2]
SPEC = importlib.util.spec_from_file_location(
    "pr_governance_status_writer", ROOT / "scripts/review/pr_governance_status_writer.py"
)
assert SPEC is not None and SPEC.loader is not None
WRITER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = WRITER
SPEC.loader.exec_module(WRITER)


class StatusWriterUnitTest(unittest.TestCase):
    def setUp(self) -> None:
        self.dispatch_boundary = patch.dict(
            os.environ,
            {
                "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[]",
                "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[]", "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "0", "GOVERNANCE_CHECK_MANIFEST": "[]",
            },
        )
        self.dispatch_boundary.start()
        self.addCleanup(self.dispatch_boundary.stop)

    def identity(self):
        return patch.multiple(
            WRITER, REPOSITORY="owner/repository", SERVER_URL="https://github.com", WRITER_RUN_ID="99"
        )

    @staticmethod
    def pull(number: int, body: object = "Fixes #64", *, draft: bool = False) -> dict[str, object]:
        return {
            "number": number, "state": "open", "draft": draft, "body": body,
            "base": {"sha": "b" * 40, "ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": "a" * 40, "ref": "governance", "repo": {"full_name": "owner/repository"}},
        }

    @staticmethod
    def generation(identifier: int = 900, attempt: int = 1, status: str = "completed", conclusion: object = "success") -> dict[str, object]:
        return {
            "id": identifier, "run_number": 8, "run_attempt": attempt, "name": "CI",
            "path": ".github/workflows/test-and-build.yml", "event": "pull_request",
            "workflow_id": 44,
            "head_sha": "a" * 40, "status": status, "conclusion": conclusion,
            "repository": {"full_name": "owner/repository"},
            "pull_requests": [{"number": 72, "base": {"sha": "b" * 40, "ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}}}],
        }

    @staticmethod
    def snapshot(numbers: tuple[int, ...], claimants: dict[str, frozenset[int]] | None = None, *, drafts: frozenset[int] = frozenset()) -> object:
        return WRITER.OpenSnapshot(
            numbers,
            {} if claimants is None else claimants,
            tuple({"number": number, "isDraft": number in drafts, "body": "Fixes #64", "head_sha": f"{number:040x}"[-40:]} for number in numbers),
        )

    def test_canonical_issue_requires_exactly_one_closer(self) -> None:
        self.assertEqual(WRITER.canonical_issue("Fixes #64"), "64")
        self.assertIsNone(WRITER.canonical_issue("Fixes #64; closes #65"))
        self.assertIsNone(WRITER.canonical_issue("No closer"))

    def test_canonical_issue_accepts_optional_colon_like_local_parser(self) -> None:
        with self.identity():
            self.assertEqual(WRITER.canonical_issue("Closes: #64"), "64")
            self.assertEqual(
                WRITER.canonical_issue(
                    "Fixes: https://github.com/owner/repository/issues/64"
                ),
                "64",
            )
            self.assertEqual(WRITER.canonical_issue("Resolves :\t#64"), "64")

    def test_canonical_issue_does_not_treat_colon_text_as_a_closing_reference(self) -> None:
        with self.identity():
            for body in (
                "encloses: #64",
                "Closes:: #64",
                "Closes #64x",
                "Closes: https://github.com/other/repository/issues/64",
            ):
                self.assertIsNone(WRITER.canonical_issue(body), body)

    def test_full_url_closer_must_target_the_current_repository(self) -> None:
        with self.identity():
            self.assertEqual(
                WRITER.canonical_issue("Fixes https://github.com/owner/repository/issues/64"),
                "64",
            )
            self.assertIsNone(
                WRITER.canonical_issue("Fixes https://github.com/other/repository/issues/64")
            )
            self.assertEqual(
                WRITER.canonical_issue(
                    "Fixes #64; fixes https://github.com/other/repository/issues/65"
                ),
                "64",
            )

    def test_workflow_path_accepts_github_at_default_branch_not_arbitrary_suffix(self) -> None:
        expected = ".github/workflows/test-and-build.yml"
        self.assertTrue(WRITER.workflow_path_matches(expected, expected))
        self.assertTrue(WRITER.workflow_path_matches(expected + "@main", expected))
        self.assertTrue(WRITER.workflow_path_matches(expected + "@refs/heads/master", expected))
        for value in (expected + "@../main", expected + "@", expected + "@main//evil", expected + "@/main"):
            with self.subTest(value=value):
                self.assertFalse(WRITER.workflow_path_matches(value, expected))

    def test_sensor_blob_uses_default_base_and_head_api_routes(self) -> None:
        calls: list[str] = []
        def api(endpoint: str):
            calls.append(endpoint)
            return {"sha": "c" * 40}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", side_effect=api):
            WRITER.trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", "b" * 40, "a" * 40)
        self.assertEqual(calls, [
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "d" * 40,
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "b" * 40,
            "repos/owner/repository/contents/.github/workflows/pr-governance-review-events.yml?ref=" + "a" * 40,
        ])

    def test_sensor_blob_rejects_pr_modified_bytes(self) -> None:
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), \
             patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}, {"sha": "c" * 40}, {"sha": "d" * 40}]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", "b" * 40, "a" * 40)

    def test_blob_cache_reuses_default_and_base_bytes_across_pr_heads(self) -> None:
        cache: dict[tuple[str, str], str] = {}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", return_value={"sha": "c" * 40}) as api:
            WRITER.trusted_workflow_blob(".github/workflows/test-and-build.yml", "b" * 40, "a" * 40, cache)
            WRITER.trusted_workflow_blob(".github/workflows/test-and-build.yml", "b" * 40, "e" * 40, cache)
        self.assertEqual(api.call_count, 4)

    def test_malformed_sibling_is_a_canonical_issue_claimant(self) -> None:
        source = {
            "number": 72, "state": "open", "body": "Fixes #64",
            "base": {"sha": "b" * 40}, "head": {"sha": "a" * 40},
        }
        sibling = {
            "number": 73, "state": "open", "body": "Fixes #64; closes #65",
            "base": {"sha": "b" * 40}, "head": {"sha": "c" * 40},
        }
        with patch.object(WRITER, "pull", return_value=source):
            self.assertFalse(WRITER.final_closer_is_unique(
                72, "64", "b" * 40, "a" * 40, WRITER.pr_body_sha256("Fixes #64"),
                {"64": frozenset({72, 73})},
            ))

    def test_open_snapshot_rejects_two_governed_prs_with_the_same_head(self) -> None:
        shared = "a" * 40
        records = [
            self.pull(72),
            self.pull(73),
        ]
        for record in records:
            record["head"]["sha"] = shared  # type: ignore[index]
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), \
             patch.object(WRITER, "pages", return_value=[records]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_newer_check_fence_rejects_terminal_write(self) -> None:
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
                self.assertTrue(WRITER.check_changed_since("a" * 40, 101))

    def test_dispatcher_input_is_bound_to_one_default_branch_run(self) -> None:
        run = {
            "id": 88, "name": WRITER.DISPATCHER_NAME,
            "path": ".github/workflows/pr-governance.yml@master",
            "event": "issues", "head_sha": "d" * 40,
            "repository": {"full_name": "owner/repository"},
            "run_number": 4, "run_attempt": 1, "status": "in_progress",
        }
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40}), \
             patch.object(WRITER, "api_json", return_value=run) as api:
            self.assertEqual(WRITER.trusted_dispatcher_source(88), WRITER.DispatcherSource(88, "issues", 1))
        self.assertTrue(api.call_args.kwargs["default_token"])
        for field, value in (("event", "push"), ("head_sha", "e" * 40), ("path", ".github/workflows/pr-governance.yml.evil@master"), ("run_attempt", 2), ("run_attempt", 0), ("run_attempt", True), ("run_attempt", "1")):
            invalid = {**run, field: value}
            with self.subTest(field=field), self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40}), \
                 patch.object(WRITER, "api_json", return_value=invalid):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.trusted_dispatcher_source(88)

    def test_writer_and_sensor_attempt_boundaries_reject_bool_zero_string_and_retry(self) -> None:
        head = "a" * 40
        writer_run = {
            "id": 99, "name": "PR governance status writer",
            "path": ".github/workflows/pr-governance-status-writer.yml@master",
            "event": "workflow_dispatch", "head_sha": head,
            "repository": {"full_name": "owner/repository"}, "status": "in_progress", "run_attempt": 1,
        }
        with self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GITHUB_SHA": head}), \
             patch.object(WRITER, "api_json", return_value=writer_run):
            WRITER.ensure_writer_run_is_active()
        for attempt in (0, True, "1", 2):
            with self.subTest(writer_attempt=attempt), self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GITHUB_SHA": head}), \
                 patch.object(WRITER, "api_json", return_value=writer_run | {"run_attempt": attempt}):
                with self.assertRaises(WRITER.NoPostGovernanceError):
                    WRITER.ensure_writer_run_is_active()

        sensor_run = self.generation(attempt=1) | {"name": "PR governance review sensor", "event": "pull_request_review", "head_sha": head, "path": ".github/workflows/pr-governance-review-events.yml@master"}
        with self.identity(), patch.object(WRITER, "trusted_workflow_blob"), patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": [sensor_run]}]):
            self.assertEqual(WRITER.sensor(72, "b" * 40, head), 900)
        for attempt in (0, True, "1", 2):
            with self.subTest(sensor_attempt=attempt), self.identity(), patch.object(WRITER, "trusted_workflow_blob"), patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": [sensor_run | {"run_attempt": attempt}]}]):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.sensor(72, "b" * 40, head)

    def test_observed_invalidations_returns_only_exact_current_carry_markers(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        with self.identity():
            carry_check = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 1)}
            fresh = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        terminal = {"status": "completed", "conclusion": "success", "details_url": WRITER.dispatcher_invalidation_url(source, 1)}
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[carry_check, fresh]):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", ())
            self.assertEqual(scoped.numbers, (72, 73))
            self.assertEqual(carry, frozenset({72}))
        with self.identity(), patch.object(WRITER, "check_run", return_value=terminal):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())
        draft_snapshot = WRITER.OpenSnapshot(
            (72,), {}, ({"number": 72, "isDraft": True, "head_sha": "a" * 40},)
        )
        with self.identity(), patch.object(WRITER, "check_run", return_value=carry_check):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(draft_snapshot, source, "all", ())
        # A writer-owned pending URL is not a dispatcher carry marker.
        for invalid in (None, fresh | {"details_url": "https://github.com/owner/repository/actions/runs/99"}):
            with self.subTest(invalid=invalid), self.identity(), patch.object(WRITER, "check_run", return_value=invalid):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.observed_invalidations(snapshot, source, "all", ())

    def test_early_scope_requires_the_exact_event_boundary_and_all_scope_accepts_ordered_priority(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "workflow_run", 1)
        with self.identity():
            marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        stale = {"status": "completed", "conclusion": "success", "details_url": "https://github.com/owner/repository/actions/runs/7"}
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "early", (72,))
        self.assertEqual(scoped.numbers, (72,))
        self.assertEqual(carry, frozenset())
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[marker, stale]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", (73, 72))
        self.assertEqual(scoped.numbers, (72, 73))
        self.assertEqual(carry, frozenset())
        self.assertEqual(WRITER.governance_order(scoped, carry, (73, 72)), (73, 72))
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, WRITER.DispatcherSource(88, "schedule", 1), "early", (72,))
        with self.identity(), patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", (72, 72))

    def test_all_scope_preserves_only_the_bound_early_success_and_skips_its_rewrite(self) -> None:
        head, base = "a" * 40, "b" * 40
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "body": "Fixes #64", "head_sha": head}, {"number": 73, "isDraft": False, "body": "Fixes #64", "head_sha": "c" * 40}),
        )
        source = WRITER.DispatcherSource(88, "workflow_run", 1)
        query = {
            "source_run_id": "1", "ci_workflow_id": "2", "ci_run_id": "3", "ci_run_number": "4", "ci_run_attempt": "1",
            "ci_status": "completed", "ci_conclusion": "success", "release_workflow_id": "5", "release_run_id": "6",
            "release_run_number": "7", "release_run_attempt": "1", "release_status": "completed", "release_conclusion": "success",
            "pr_base_sha": base, "pr_head_sha": head,
            "pr_body_sha256": WRITER.pr_body_sha256("Fixes #64"),
        }
        early_success = {
            "id": 711, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": f"krr-governance/v1/{head}/writer-71", "updated_at": "now", "app": {"id": 42},
            "status": "completed", "conclusion": "success",
            "details_url": "https://github.com/owner/repository/actions/runs/71?" + WRITER.urlencode(query),
        }
        with self.identity():
            marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        # The preserved source is located via its exact writer-71 external
        # generation, not the current all-writer dispatcher generation.
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "object_pages", return_value=[{"check_runs": [early_success]}]), \
             patch.object(WRITER, "check_run", return_value=marker):
            scoped, carry = WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 71)
        self.assertEqual(scoped.numbers, (73,))
        self.assertEqual(carry, frozenset())
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "object_pages", return_value=[{"check_runs": [early_success]}]), \
             patch.object(WRITER, "check_run", return_value=marker):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 72)
        for digest in (None, "d" * 64):
            stale_query = dict(query)
            if digest is None:
                del stale_query["pr_body_sha256"]
            else:
                stale_query["pr_body_sha256"] = digest
            stale = {
                **early_success,
                "details_url": "https://github.com/owner/repository/actions/runs/71?" + WRITER.urlencode(stale_query),
            }
            with self.subTest(digest=digest), self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
                 patch.object(WRITER, "object_pages", return_value=[{"check_runs": [stale]}]), \
                 patch.object(WRITER, "check_run", return_value=marker):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.observed_invalidations(snapshot, source, "all", (72, 73), (72,), 71)

    def test_all_scope_fails_closed_when_a_new_open_pr_missed_the_all_open_invalidation(self) -> None:
        snapshot = WRITER.OpenSnapshot(
            (72, 73), {},
            ({"number": 72, "isDraft": False, "head_sha": "a" * 40}, {"number": 73, "isDraft": False, "head_sha": "b" * 40}),
        )
        source = WRITER.DispatcherSource(88, "issues", 1)
        marker = {"status": "in_progress", "conclusion": None, "details_url": WRITER.dispatcher_invalidation_url(source, 0)}
        with self.identity(), patch.object(WRITER, "check_run", side_effect=[marker, None]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.observed_invalidations(snapshot, source, "all", ())

    def test_main_rejects_noncanonical_or_invalid_dispatch_target_inputs_before_api_reads(self) -> None:
        for scope, targets in (("all", "[ ]"), ("all", "[72,72]"), ("early", "[true]"), ("invalid", "[]")):
            with self.subTest(scope=scope, targets=targets), self.identity(), \
                 patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": scope, "GOVERNANCE_TARGET_NUMBERS": targets}), \
                 patch.object(WRITER, "trusted_dispatcher_source") as source:
                self.assertEqual(WRITER.main(), 1)
                source.assert_not_called()

    def test_production_manifest_binds_exact_ids_in_preserved_event_unrelated_order(self) -> None:
        """The all writer receives the dispatcher POST IDs, never a name-only lookup."""
        snapshot = self.snapshot((1, 72, 73))
        source = WRITER.DispatcherSource(88, "issues", 1)
        environment = {
            "GITHUB_ACTIONS": "true",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all",
            "GOVERNANCE_TARGET_NUMBERS": "[72,73]",
            "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]",
            "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
            # preserved source, related claimant, then unrelated snapshot PR.
            "GOVERNANCE_CHECK_MANIFEST": "[[72,701],[73,702],[1,703]]",
        }
        with self.identity(), patch.dict(os.environ, environment), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=source), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=None):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(
            WRITER._bound_check_runs,
            {
                (f"{1:040x}", f"krr-governance/v1/{1:040x}/dispatcher-88"): 703,
                (f"{72:040x}", f"krr-governance/v1/{72:040x}/writer-71"): 701,
                (f"{73:040x}", f"krr-governance/v1/{73:040x}/dispatcher-88"): 702,
            },
        )

    def test_production_main_uses_manifest_ids_for_preserved_and_terminal_dispatcher_generations(self) -> None:
        snapshot = self.snapshot((1, 72, 73))
        heads = {item["number"]: item["head_sha"] for item in snapshot.pull_requests}
        base = "b" * 40
        early_query = {
            "source_run_id": "1", "ci_workflow_id": "2", "ci_run_id": "3", "ci_run_number": "4", "ci_run_attempt": "1",
            "ci_status": "completed", "ci_conclusion": "success", "release_workflow_id": "5", "release_run_id": "6",
            "release_run_number": "7", "release_run_attempt": "1", "release_status": "completed", "release_conclusion": "success",
            "pr_base_sha": base, "pr_head_sha": heads[72], "pr_body_sha256": WRITER.pr_body_sha256("Fixes #64"),
        }
        values: dict[int, dict[str, object]] = {
            701: {"id": 701, "name": WRITER.CHECK_NAME, "head_sha": heads[72], "external_id": f"krr-governance/v1/{heads[72]}/writer-71", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "completed", "conclusion": "success", "details_url": f"https://github.com/owner/repository/actions/runs/71?{WRITER.urlencode(early_query)}"},
            702: {"id": 702, "name": WRITER.CHECK_NAME, "head_sha": heads[73], "external_id": f"krr-governance/v1/{heads[73]}/dispatcher-88", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/88?dispatcher_run_id=88&carry_pending=0"},
            703: {"id": 703, "name": WRITER.CHECK_NAME, "head_sha": heads[1], "external_id": f"krr-governance/v1/{heads[1]}/dispatcher-88", "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": f"https://github.com/owner/repository/actions/runs/88?dispatcher_run_id=88&carry_pending=0"},
        }
        dispatcher = {"id": 88, "name": WRITER.DISPATCHER_NAME, "path": ".github/workflows/pr-governance.yml@master", "event": "issues", "head_sha": "d" * 40, "repository": {"full_name": "owner/repository"}, "run_number": 1, "run_attempt": 1, "status": "in_progress"}
        writer = {"id": 99, "name": "PR governance status writer", "path": ".github/workflows/pr-governance-status-writer.yml@master", "event": "workflow_dispatch", "head_sha": "d" * 40, "repository": {"full_name": "owner/repository"}, "run_attempt": 1, "status": "in_progress"}
        reads: list[int] = []; terminal_ids: list[int] = []
        def api(endpoint: str, *, default_token: bool = False) -> object:
            if endpoint.endswith("/actions/runs/88"):
                return dispatcher
            if endpoint.endswith("/actions/runs/99"):
                return writer
            identifier = int(endpoint.rsplit("/", 1)[1]); reads.append(identifier)
            return values[identifier]
        def write(arguments: list[str], *, check_write: bool = False, default_token: bool = False) -> str:
            self.assertTrue(check_write)
            identifier = int(next(value.rsplit("/", 1)[1] for value in arguments if "/check-runs/" in value))
            details = next(value.split("=", 1)[1] for value in arguments if value.startswith("details_url="))
            terminal_ids.append(identifier); values[identifier] = values[identifier] | {"status": "completed", "conclusion": "failure", "details_url": details}
            return json.dumps(values[identifier])
        def terminal(number: int, _claimants: object, _path: str, _evidence: object, *, defer_terminal: bool) -> None:
            self.assertTrue(defer_terminal)
            value = WRITER.check_run(heads[number])
            WRITER.write_check(heads[number], state="failure", description="fixture", details_url="https://github.com/owner/repository/actions/runs/88", existing=value)
            return None
        environment = {
            "GITHUB_ACTIONS": "true", "GITHUB_SHA": "d" * 40, "KRR_GOVERNANCE_CHECK_APP_ID": "42",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[72,73]",
            "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]", "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
            "GOVERNANCE_CHECK_MANIFEST": "[[72,701],[73,702],[1,703]]",
        }
        with self.identity(), patch.dict(os.environ, environment), patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "api_json", side_effect=api), patch.object(WRITER, "command", side_effect=write), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=terminal), patch.object(WRITER, "pace_check_write"):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(terminal_ids, [702, 703])
        self.assertIn(701, reads)
        self.assertEqual(reads.count(702), 3)
        self.assertEqual(reads.count(703), 3)

    def test_production_manifest_rejects_reordered_or_duplicate_ids_before_revalidation(self) -> None:
        snapshot = self.snapshot((1, 72, 73))
        source = WRITER.DispatcherSource(88, "issues", 1)
        base = {
            "GITHUB_ACTIONS": "true",
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all",
            "GOVERNANCE_TARGET_NUMBERS": "[72,73]", "GOVERNANCE_PRESERVED_TARGET_NUMBERS": "[72]",
            "GOVERNANCE_PRESERVED_WRITER_RUN_ID": "71",
        }
        for manifest in ("[[72,701],[73,702]]", "[[72,701],[73,702],[1,703],[74,704]]", "[[72,701],[1,703],[73,702]]", "[[72,701],[73,701],[1,703]]"):
            with self.subTest(manifest=manifest), self.identity(), patch.dict(os.environ, base | {"GOVERNANCE_CHECK_MANIFEST": manifest}), \
                 patch.object(WRITER, "trusted_dispatcher_source", return_value=source), \
                 patch.object(WRITER, "open_snapshot", return_value=snapshot), \
                 patch.object(WRITER, "observed_invalidations") as observed:
                self.assertEqual(WRITER.main(), 1)
                observed.assert_not_called()

    def test_bound_manifest_id_is_reread_by_id_before_an_immutable_write(self) -> None:
        head = "a" * 40
        external = f"krr-governance/v1/{head}/writer-71"
        value = {
            "id": 701, "name": WRITER.CHECK_NAME, "head_sha": head, "external_id": external,
            "updated_at": "2026-08-30T00:00:00Z", "app": {"id": 42},
        }
        WRITER._bound_check_runs.clear()
        WRITER._bound_check_runs[(head, external)] = 701
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "api_json", return_value=value) as api:
            self.assertEqual(WRITER.check_run_for_external_id(head, external), value)
        api.assert_called_once_with("repos/owner/repository/check-runs/701")

    def test_all_writer_never_posts_a_replacement_when_a_bound_generation_disappears(self) -> None:
        head = "a" * 40
        external = f"krr-governance/v1/{head}/dispatcher-88"
        WRITER._bound_check_runs.clear()
        WRITER._bound_check_runs[(head, external)] = 701
        with self.identity(), patch.dict(os.environ, {"GITHUB_ACTIONS": "true", "GOVERNANCE_SCOPE": "all"}), \
             patch.object(WRITER, "api_json", return_value=None), patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.write_check(
                    head, state="in_progress", description="missing", details_url="https://github.com/owner/repository/actions/runs/88",
                )
        command.assert_not_called()

    def test_terminal_rebind_rejects_default_branch_advance(self) -> None:
        responses = [
            {"default_branch": "master"},
            {"object": {"sha": "e" * 40}},
        ]
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40}), \
             patch.object(WRITER, "api_json", side_effect=responses):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.rebind_trusted_default_writer()

    def test_later_invalidator_details_fingerprint_blocks_terminal_patch(self) -> None:
        head = "a" * 40
        baseline = {
            "id": 12, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": WRITER.check_external_id(head), "updated_at": "one",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/88", "app": {"id": 42},
        }
        later = {**baseline, "updated_at": "two", "details_url": "https://github.com/owner/repository/actions/runs/89"}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=later):
            self.assertTrue(WRITER.check_changed_since(head, WRITER.check_fingerprint(baseline)))

    def test_main_finalizes_each_pr_before_processing_the_next(self) -> None:
        snapshot = self.snapshot((72, 73))
        first = WRITER.PendingDecision(72, "a" * 40, "b" * 40, (), "failure", "failed", None, None, None, "c" * 64)
        second = WRITER.PendingDecision(73, "c" * 40, "d" * 40, (), "failure", "failed", None, None, None, "c" * 64)
        calls: list[str] = []
        def process(number, *_args, **_kwargs):
            calls.append(f"process-{number}")
            return first if number == 72 else second
        def final(head, _evidence):
            calls.append(f"evidence-{head[0]}")
            return WRITER.EvidenceSnapshot({}, {}, {})
        def finalize(decision, *_args):
            calls.append(f"finalize-{decision.number}")
            return True
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=process), patch.object(WRITER, "final_evidence_for_pr", side_effect=final), \
             patch.object(WRITER, "finalize_decision", side_effect=finalize):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(calls, ["process-72", "evidence-a", "finalize-72", "process-73", "evidence-c", "finalize-73"])

    def test_main_continues_after_one_pr_fails(self) -> None:
        with patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), patch.object(WRITER, "REPOSITORY", "owner/repository"), \
             patch.object(WRITER, "SERVER_URL", "https://github.com"), \
             patch.object(WRITER, "WRITER_RUN_ID", "99"), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=self.snapshot((72, 73), {"64": frozenset({72, 73})})), \
             patch.object(WRITER, "observed_invalidations", return_value=(self.snapshot((72, 73), {"64": frozenset({72, 73})}), frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=[WRITER.GovernanceError("bad"), None]) as process:
            self.assertEqual(WRITER.main(), 1)
            self.assertEqual(process.call_count, 2)

    def test_open_pulls_collects_late_pages_without_a_target_limit(self) -> None:
        payload = [[{"number": number, "state": "open"} for number in range(1, 101)], [{"number": 301, "state": "open"}]]
        with self.identity(), patch.object(WRITER, "command", return_value=json.dumps(payload)):
            self.assertEqual(WRITER.open_pulls()[-1], 301)

    def test_single_snapshot_indexes_300_prs_and_multi_issue_claimants_in_one_paged_read(self) -> None:
        pages = []
        for start in (1, 101, 201):
            pages.append([{"number": number, "state": "open", "draft": False, "body": "Fixes #64" if number != 250 else "Fixes #64; closes #65", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": f"{number:040x}"[-40:], "repo": {"full_name": "owner/repository"}}} for number in range(start, start + 100)])
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "command", return_value=json.dumps(pages)) as command:
            snapshot = WRITER.open_snapshot()
        self.assertEqual(len(snapshot.numbers), 300)
        self.assertEqual(snapshot.claimants["64"], frozenset(range(1, 301)))
        self.assertEqual(snapshot.claimants["65"], frozenset({250}))
        self.assertEqual(command.call_count, 1)

    def test_snapshot_normalizes_missing_body_but_rejects_duplicate_pr(self) -> None:
        valid = {"number": 72, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"repo": {"full_name": "owner/repository"}}}
        bodyless = {
            "number": 71, "state": "open", "draft": False, "body": None,
            "base": {"ref": "master", "repo": {"full_name": "owner/repository"}},
            "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}},
        }
        complete = {**valid, "head": {"sha": "b" * 40, "repo": {"full_name": "owner/repository"}}}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "command", return_value=json.dumps([[bodyless, complete]])):
            snapshot = WRITER.open_snapshot()
        self.assertEqual(snapshot.numbers, (71, 72))
        self.assertEqual(snapshot.pull_requests[0]["body"], "")
        self.assertEqual(snapshot.claimants["64"], frozenset({72}))
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "command", return_value=json.dumps([[complete], [complete]])):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_snapshot_excludes_fork_claimant_from_local_canonical_issue(self) -> None:
        local = {"number": 72, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "a" * 40, "repo": {"full_name": "owner/repository"}}}
        fork = {"number": 73, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "fork/repository"}}}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "command", return_value=json.dumps([[local, fork]])):
            snapshot = WRITER.open_snapshot()
        self.assertEqual(snapshot.numbers, (72,))
        self.assertEqual(snapshot.claimants["64"], frozenset({72}))

    def test_snapshot_rejects_a_duplicate_fork_on_later_page(self) -> None:
        fork = {"number": 73, "state": "open", "draft": False, "body": "Fixes #64", "base": {"ref": "master", "repo": {"full_name": "owner/repository"}}, "head": {"sha": "b" * 40, "repo": {"full_name": "fork/repository"}}}
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "command", return_value=json.dumps([[fork], [fork]])):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_snapshot()

    def test_300_pr_main_processes_all_after_one_failure_with_one_snapshot(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)), {"64": frozenset(range(1, 301))})
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), patch.object(WRITER, "open_snapshot", return_value=snapshot) as open_snapshot, \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=[WRITER.GovernanceError("one failed"), *([None] * 299)]) as process:
            self.assertEqual(WRITER.main(), 1)
        open_snapshot.assert_called_once_with()
        self.assertEqual(process.call_count, 300)

    def test_event_reserves_100_terminal_write_budget(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, 99, "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision) as process, \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=False) as finalize:
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(process.call_count, 300)
        self.assertEqual(finalize.call_count, 100)

    def test_all_open_event_priority_keeps_the_source_inside_the_first_100_terminal_writes(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 201)))
        finalized: list[int] = []

        def decision(number: int, *_args, **_kwargs):
            return WRITER.PendingDecision(number, f"{number:040x}"[-40:], "b" * 40, 99, "failure", "bad", None, None, None, "c" * 64)

        def finalize(value, *_args):
            finalized.append(value.number)
            return True

        with self.identity(), patch.dict(os.environ, {
            "GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_SCOPE": "all", "GOVERNANCE_TARGET_NUMBERS": "[150,149]",
        }), patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", side_effect=decision) as process, \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", side_effect=finalize):
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(process.call_args_list[0].args[0], 150)
        self.assertEqual(process.call_args_list[1].args[0], 149)
        self.assertEqual(finalized[:2], [150, 149])
        self.assertEqual(len(finalized), 100)
        self.assertNotIn(100, finalized)

    def test_carry_precedes_fresh_targets_and_converges_with_any_dispatcher_gaps(self) -> None:
        numbers = tuple(range(1, 451))
        remaining = frozenset(numbers)
        # Each writer can safely terminalize only 50 event targets when every
        # target needs a POST+PATCH.  A new dispatcher may have arbitrary
        # skipped IDs, but carry is recorded on every unprocessed head.
        for _dispatcher_id in (7, 8, 91, 1042, 1043, 99999, 100003, 100004, 900000):
            ordered = WRITER.governance_order(self.snapshot(numbers), remaining)
            self.assertEqual(ordered[:len(remaining)], tuple(sorted(remaining)))
            completed = frozenset(ordered[:50])
            remaining = remaining - completed
        self.assertEqual(remaining, frozenset())

    def test_200_permanent_drafts_do_not_starve_carried_or_fresh_terminal_targets(self) -> None:
        numbers = tuple(range(1, 406))
        drafts = frozenset((*range(1, 201), *range(206, 406)))
        snapshot = self.snapshot(numbers, drafts=drafts)
        # PR #204/#205 were previously budget-deferred.  They must run first,
        # then fresh terminal PR #201-#203, before any of 400 Draft targets.
        ordered = WRITER.governance_order(snapshot, frozenset({204, 205}))
        self.assertEqual(ordered[:5], (204, 205, 201, 202, 203))
        self.assertEqual(set(ordered[5:]), set(drafts))

    def test_300_ready_pr_main_stays_within_split_token_transport_budgets(self) -> None:
        numbers = tuple(range(1, 301))
        snapshot = WRITER.OpenSnapshot(numbers, {"64": frozenset(numbers)}, tuple({"number": number, "isDraft": False, "body": "Fixes #64"} for number in numbers))
        evidence = WRITER.EvidenceSnapshot({}, {}, {})
        def current(number: int):
            value = self.pull(number)
            value["head"]["sha"] = f"{number:040x}"[-40:]  # type: ignore[index]
            return value
        generation = WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success")
        release = WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")
        transport: dict[str, int] = {"app_rest": 0, "default_rest": 0, "graphql": 0}
        class Result:
            returncode = 0
        def verifier_transport(arguments, **kwargs):
            command, environment = arguments, kwargs["env"]
            self.assertEqual(environment, {"GH_TOKEN": "app-read", "PATH": os.environ["PATH"]})
            self.assertEqual(command[0], sys.executable)
            if command[1].endswith("verify_push_issue.py"):
                transport["app_rest"] += 3  # PR range, changed paths, referenced Issue.
            elif command[1].endswith("verify_pr_ready.py"):
                transport["app_rest"] += 3  # PR, comments/reactions and cached closer inputs.
                transport["graphql"] += 2  # review threads and reviews.
            else:
                self.fail(f"unexpected verifier command: {command}")
            return Result()
        with self.identity(), patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=evidence), patch.object(WRITER, "pull", side_effect=current), \
             patch.object(WRITER, "write_governance_check", side_effect=range(1, 1000)) as post, patch.object(WRITER.subprocess, "run", side_effect=verifier_transport), \
             patch.object(WRITER, "check_baseline", return_value=0) as baseline, \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=[generation, release] * 900), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False), \
             patch.object(WRITER, "check_fence", return_value=(False, 0, False)) as check_run_fence, \
             patch.object(WRITER, "rebind_trusted_default_writer"), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=evidence) as final_evidence, \
             patch.dict(os.environ, {"GH_TOKEN": "app-read", "DEFAULT_READ_TOKEN": "default-read", "CHECK_WRITE_TOKEN": "check-write", "KRR_GOVERNANCE_CHECK_APP_ID": "42", "GOVERNANCE_DISPATCHER_RUN_ID": "88"}):
            self.assertEqual(WRITER.main(), 0)
        # The dispatcher owns event invalidation.  The writer emits one
        # terminal status per PR, never a duplicate pending status.
        self.assertEqual(post.call_count, 200)
        self.assertEqual(final_evidence.call_count, 200)
        self.assertEqual(check_run_fence.call_count, 200)
        self.assertEqual(baseline.call_count, 300)
        # The real writer API calls are bounded separately: final source
        # pulls/status fences use DEFAULT_READ_TOKEN, while verifier evidence
        # uses the App read token; GraphQL has its own point budget.
        # Conservative real-transport accounting.  The App installation token
        # carries verifier/evidence traffic; only terminal source/fence reads
        # use github.token.  Keeping the boundaries separate is essential:
        # the latter has the lower Actions rate ceiling.
        transport["app_rest"] += 2408
        transport["default_rest"] += 906
        transport["graphql"] += 1200
        self.assertLess(transport["app_rest"], 4500)
        self.assertLess(transport["default_rest"], 950)
        self.assertLess(transport["graphql"], 2500)

    def test_initial_pr_read_uses_app_token_but_final_closer_uses_default_token(self) -> None:
        current = self.pull(72)
        calls: list[bool] = []
        def api(_endpoint: str, *, default_token: bool = False):
            calls.append(default_token)
            return current
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master"}), patch.object(WRITER, "api_json", side_effect=api):
            self.assertEqual(WRITER.pull(72)["number"], 72)
            self.assertTrue(WRITER.final_closer_is_unique(
                72, "64", "b" * 40, "a" * 40, WRITER.pr_body_sha256("Fixes #64"),
                {"64": frozenset({72})},
            ))
        self.assertEqual(calls, [False, True])

    def test_check_baseline_uses_the_app_read_path_before_default_fence(self) -> None:
        value = {"id": 12, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_baseline("a" * 40), WRITER.check_fingerprint(value))

    def test_evidence_snapshot_pages_each_workflow_once_for_all_300_prs(self) -> None:
        calls: list[str] = []
        def pages(endpoint: str):
            calls.append(endpoint)
            return [{"workflow_runs": []}]
        def api(endpoint: str):
            return {"id": 44 if endpoint.endswith("test-and-build.yml") else 45}
        with self.identity(), patch.object(WRITER, "object_pages", side_effect=pages), patch.object(WRITER, "api_json", side_effect=api):
            evidence = WRITER.evidence_snapshot()
        self.assertEqual(len(calls), 5)
        self.assertEqual(evidence.workflow_ids, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45})

    def test_final_evidence_is_bounded_to_three_head_specific_requests(self) -> None:
        calls: list[str] = []
        def pages(endpoint: str):
            calls.append(endpoint)
            return [{"workflow_runs": []}]
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        with self.identity(), patch.object(WRITER, "object_pages", side_effect=pages):
            WRITER.final_evidence_for_pr("a" * 40, initial)
        self.assertEqual(len(calls), 3)
        self.assertTrue(all("head_sha=" + "a" * 40 in endpoint for endpoint in calls))

    def test_final_evidence_keeps_a_run_beyond_the_first_100_head_matches(self) -> None:
        older = [{"id": number, "event": "pull_request"} for number in range(1, 101)]
        newer = {"id": 101, "event": "pull_request"}
        initial = WRITER.EvidenceSnapshot({}, {".github/workflows/test-and-build.yml": 44, ".github/workflows/release-preflight.yml": 45}, {})
        with self.identity(), patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": older}, {"workflow_runs": [newer]}]):
            evidence = WRITER.final_evidence_for_pr("a" * 40, initial)
        self.assertIn(newer, evidence.workflow_runs[".github/workflows/test-and-build.yml"])

    def test_final_head_pages_choose_the_101st_rerun_and_sensor(self) -> None:
        older_ci = self.generation(900, 1)
        newer_ci = self.generation(901, 2)
        sensor_template = self.generation(700, 1)
        sensor_template.update({"name": "PR governance review sensor", "path": ".github/workflows/pr-governance-review-events.yml@main", "event": "pull_request"})
        sensor_newer = dict(sensor_template); sensor_newer.update({"id": 701, "run_number": 9})
        cache = {
            (path, ref): "c" * 40
            for path in (".github/workflows/test-and-build.yml", ".github/workflows/pr-governance-review-events.yml")
            for ref in ("d" * 40, "b" * 40, "a" * 40)
        }
        evidence = WRITER.EvidenceSnapshot({"pull_request": tuple([sensor_template] * 100 + [sensor_newer]), "pull_request_review": (), "pull_request_review_comment": ()}, {".github/workflows/test-and-build.yml": 44}, {".github/workflows/test-and-build.yml": tuple([older_ci] * 100 + [newer_ci])}, cache)
        with self.identity(), patch.dict(os.environ, {"GITHUB_SHA": "d" * 40}):
            selected = WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml", evidence)
            sensor = WRITER.sensor(72, "b" * 40, "a" * 40, evidence)
        self.assertEqual(selected.identifier, 901)
        self.assertEqual(sensor, 701)

    def test_open_pulls_rejects_duplicate_across_pages(self) -> None:
        payload = [[{"number": 72, "state": "open"}], [{"number": 72, "state": "open"}]]
        with self.identity(), patch.object(WRITER, "command", return_value=json.dumps(payload)):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.open_pulls()

    def test_open_pulls_rejects_malformed_or_nonopen_values(self) -> None:
        for payload in ([{}], [[{"number": "72", "state": "open"}]], [[{"number": 72, "state": "closed"}]]):
            with self.subTest(payload=payload), self.identity(), patch.object(WRITER, "command", return_value=json.dumps(payload)):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.open_pulls()

    def test_current_pull_rejects_foreign_base_or_head_repository(self) -> None:
        value = self.pull(72)
        value["head"]["repo"]["full_name"] = "foreign/repository"  # type: ignore[index]
        with self.identity(), patch.object(WRITER, "api_json", return_value=value):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.pull(72)

    def test_current_pull_rejects_a_nondefault_base_branch(self) -> None:
        value = self.pull(72); value["base"]["ref"] = "release/old"  # type: ignore[index]
        with self.identity(), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), patch.object(WRITER, "api_json", return_value=value):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.pull(72)

    def test_pages_rejects_non_array_pages_and_non_object_items(self) -> None:
        for payload in ({}, ["bad"], [["bad"]]):
            with self.subTest(payload=payload), patch.object(WRITER, "command", return_value=json.dumps(payload)):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.pages("ignored")

    def test_check_pages_reach_a_later_pending_fence(self) -> None:
        item = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
        payload = [{"check_runs": []}, {"check_runs": [item]}]
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "command", return_value=json.dumps(payload)):
            self.assertEqual(WRITER.check_run("a" * 40), item)

    def test_check_run_ignores_foreign_and_historical_generations(self) -> None:
        for mutate in (lambda value: value["app"].update(id=7), lambda value: value.update(external_id="wrong")):
            value = {"id": 99, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}}
            mutate(value)
            with self.subTest(value=value), self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "object_pages", return_value=[{"check_runs": [value]}]):
                self.assertIsNone(WRITER.check_run("a" * 40))

    def test_check_fence_re_reads_same_id_and_evidence(self) -> None:
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}, "status": "completed", "conclusion": "success", "details_url": "https://github.test/run?source_run_id=77"}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_fence("a" * 40, (), 77), (True, 1, False))

    def test_check_fence_deduplicates_identical_in_progress_pending_evidence(self) -> None:
        details = "https://github.test/run"
        value = {"id": 102, "name": WRITER.CHECK_NAME, "head_sha": "a" * 40, "external_id": WRITER.check_external_id("a" * 40), "updated_at": "now", "app": {"id": 42}, "status": "in_progress", "conclusion": None, "details_url": details}
        with patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), patch.object(WRITER, "check_run", return_value=value):
            self.assertEqual(WRITER.check_fence("a" * 40, WRITER.check_fingerprint(value), 77, desired_state="pending", desired_target=details), (False, 0, True))

    def test_read_and_write_gh_boundaries_do_not_share_tokens(self) -> None:
        captured: list[dict[str, str]] = []
        class Result:
            returncode = 0
            stdout = "{}"
        def run(*_args, **kwargs):
            captured.append(kwargs["env"])
            return Result()
        previous = os.environ.get("CHECK_WRITE_TOKEN")
        previous_read = os.environ.get("GH_TOKEN")
        previous_default = os.environ.get("DEFAULT_READ_TOKEN")
        os.environ["CHECK_WRITE_TOKEN"] = "secret-check-token"; os.environ["GH_TOKEN"] = "read-token"; os.environ["DEFAULT_READ_TOKEN"] = "default-read-token"
        try:
            with patch.object(WRITER.subprocess, "run", side_effect=run):
                WRITER.command(["repos/owner/repository"])
                WRITER.command(["repos/owner/repository/pulls/72"], default_token=True)
                WRITER.command(["--method", "POST", "ignored"], check_write=True)
            self.assertEqual(captured[0], {"GH_TOKEN": "read-token", "PATH": os.environ["PATH"]})
            self.assertEqual(captured[1], {"GH_TOKEN": "default-read-token", "PATH": os.environ["PATH"]})
            self.assertEqual(captured[2], {"GH_TOKEN": "secret-check-token", "PATH": os.environ["PATH"]})
        finally:
            if previous is None: del os.environ["CHECK_WRITE_TOKEN"]
            else: os.environ["CHECK_WRITE_TOKEN"] = previous
            if previous_read is None: del os.environ["GH_TOKEN"]
            else: os.environ["GH_TOKEN"] = previous_read
            if previous_default is None: del os.environ["DEFAULT_READ_TOKEN"]
            else: os.environ["DEFAULT_READ_TOKEN"] = previous_default

    def test_contract_verifiers_receive_only_the_read_token(self) -> None:
        class Result:
            returncode = 0
        captured: list[tuple[list[str], dict[str, str]]] = []
        previous = {key: os.environ.get(key) for key in ("GH_TOKEN", "CHECK_WRITE_TOKEN", "KRR_GOVERNANCE_APP_PRIVATE_KEY")}
        os.environ.update({"GH_TOKEN": "read-token", "CHECK_WRITE_TOKEN": "write-secret", "KRR_GOVERNANCE_APP_PRIVATE_KEY": "private-secret"})
        try:
            def run(*_args, **kwargs):
                captured.append((_args[0], kwargs["env"]))
                return Result()
            with self.identity(), patch.object(WRITER.subprocess, "run", side_effect=run):
                self.assertEqual(WRITER.contract(72, "b" * 40, "a" * 40, "branch", False, "/tmp/snapshot.json"), "success")
            self.assertEqual([value[1] for value in captured], [{"GH_TOKEN": "read-token", "PATH": os.environ["PATH"]}] * 2)
            self.assertNotIn("--open-pull-snapshot", captured[0][0])
            self.assertIn("--exclude-trusted-governance-check", captured[1][0])
            self.assertEqual(captured[1][0][-2:], ["--open-pull-snapshot", "/tmp/snapshot.json"])
        finally:
            for key, value in previous.items():
                if value is None: os.environ.pop(key, None)
                else: os.environ[key] = value

    def test_generation_selects_latest_same_head_attempt(self) -> None:
        first = self.generation(900, 1)
        second = self.generation(901, 2)
        second["path"] = ".github/workflows/test-and-build.yml@main"
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
             patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": [first, second]}]):
            value = WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")
        self.assertEqual(value.identifier, 901)
        self.assertEqual(value.attempt, 2)

    def test_generation_rejects_pr_modified_or_missing_workflow_blob(self) -> None:
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}, {"sha": "c" * 40}, {"sha": "d" * 40}]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", return_value={"sha": True}):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_generation_rejects_foreign_base_head_or_multiple_pr_binding(self) -> None:
        for mutate in (
            lambda run: run.update(repository={"full_name": "foreign/repository"}),
            lambda run: run["pull_requests"][0]["head"].update(sha="c" * 40),
            lambda run: run.update(pull_requests=[]),
        ):
            run = self.generation(); mutate(run)
            with self.subTest(run=run), patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
                 patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": [run]}]):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_generation_rejects_a_run_with_wrong_default_workflow_id(self) -> None:
        run = self.generation(); run["workflow_id"] = 45
        with patch.dict(os.environ, {"GITHUB_REF_NAME": "master", "GITHUB_SHA": "d" * 40}), self.identity(), \
             patch.object(WRITER, "api_json", side_effect=[{"sha": "c" * 40}] * 3 + [{"id": 44}]), \
             patch.object(WRITER, "object_pages", return_value=[{"workflow_runs": [run]}]):
            with self.assertRaises(WRITER.GovernanceError):
                WRITER.generation(72, "b" * 40, "a" * 40, "CI", ".github/workflows/test-and-build.yml")

    def test_success_target_url_binds_workflow_and_generation_ids(self) -> None:
        values = (
            WRITER.Generation("CI", "x", 44, 101, 8, 2, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 102, 9, 1, "completed", "success"),
        )
        with self.identity():
            url = WRITER.target_url(
                source_run_id=77, generations=values, base="b" * 40, head="a" * 40,
                body_sha256="c" * 64,
            )
        self.assertIn("source_run_id=77", url)
        self.assertIn("ci_workflow_id=44", url)
        self.assertIn("ci_run_attempt=2", url)
        self.assertIn("ci_run_number=8", url)
        self.assertIn("ci_status=completed", url)
        self.assertIn("ci_conclusion=success", url)
        self.assertIn("release_workflow_id=45", url)
        self.assertIn("pr_base_sha=" + "b" * 40, url)
        self.assertIn("pr_head_sha=" + "a" * 40, url)
        self.assertIn("pr_body_sha256=" + "c" * 64, url)

    def test_body_digest_rejects_non_text_nul_and_invalid_utf8_scalars(self) -> None:
        self.assertEqual(
            WRITER.pr_body_sha256("Fixes #64"),
            "807aa69d375bfa66f74b64ac2143fa2c9511a011eb57ab8b4883f052d7ceb65f",
        )
        for value in (None, 64, "Fixes #64\0", "Fixes #64\ud800"):
            with self.subTest(value=repr(value)):
                with self.assertRaises(WRITER.GovernanceError):
                    WRITER.pr_body_sha256(value)

    def test_success_evidence_requires_the_exact_body_digest(self) -> None:
        values = (
            WRITER.Generation("CI", "x", 44, 101, 8, 2, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 102, 9, 1, "completed", "success"),
        )
        with self.identity():
            desired = WRITER.target_url(
                source_run_id=77, generations=values, base="b" * 40, head="a" * 40,
                body_sha256="c" * 64,
            )
        self.assertTrue(WRITER._same_check_evidence(desired.replace("/99?", "/100?"), desired))
        self.assertFalse(WRITER._same_check_evidence(desired.replace("&pr_body_sha256=" + "c" * 64, ""), desired))
        self.assertFalse(WRITER._same_check_evidence(desired.replace("c" * 64, "d" * 64), desired))

    def test_verdict_handles_requested_success_and_terminal_failure(self) -> None:
        template = WRITER.Generation("CI", "p", 44, 1, 1, 1, "completed", "success")
        self.assertEqual(WRITER.verdict(template), "success")
        self.assertEqual(WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, "queued", None)), "pending")
        self.assertEqual(WRITER.verdict(template.__class__("CI", "p", 44, 1, 1, 1, "completed", "failure")), "failure")

    def test_draft_process_stays_pending_without_sensor_or_ci(self) -> None:
        current = self.pull(72, draft=True)
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, \
             patch.object(WRITER, "contract", return_value="pending"), \
             patch.object(WRITER, "check_changed_since", return_value=False), \
             patch.object(WRITER, "sensor") as sensor, patch.object(WRITER, "generation") as generation:
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_count, 2)
        sensor.assert_not_called(); generation.assert_not_called()

    def test_bodyless_pr_defers_one_fail_closed_decision_to_the_budgeted_writer(self) -> None:
        current = self.pull(72)
        current["body"] = None
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "write_governance_check", return_value=(102,)) as post:
            decision = WRITER.process(72, {}, "/tmp/snapshot.json", defer_terminal=True)
        self.assertEqual(decision.state if decision is not None else None, "failure")
        self.assertEqual(decision.description if decision is not None else None, "Trusted PR governance failed closed.")
        post.assert_not_called()

    def test_deferred_success_carries_the_process_time_body_digest(self) -> None:
        current = self.pull(72)
        generations = (
            WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"),
        )
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), \
             patch.object(WRITER, "generation", side_effect=[*generations, *generations]), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True):
            decision = WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json", defer_terminal=True)
        self.assertEqual(decision.state if decision is not None else None, "success")
        self.assertEqual(
            decision.body_sha256 if decision is not None else None,
            WRITER.pr_body_sha256("Fixes #64"),
        )

    def test_finalize_refuses_success_when_the_pr_body_changes_after_the_decision(self) -> None:
        generations = (
            WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"),
            WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"),
        )
        decision = WRITER.PendingDecision(
            72, "a" * 40, "b" * 40, (101,), "success", "ok", 77, generations, "64",
            WRITER.pr_body_sha256("Fixes #64"),
        )
        with self.identity(), patch.object(WRITER, "final_closer_is_unique", side_effect=[True, False]), \
             patch.object(WRITER, "sensor", return_value=77), \
             patch.object(WRITER, "generation", side_effect=generations), \
             patch.object(WRITER, "check_fence", return_value=(False, 0, False)), \
             patch.object(WRITER, "rebind_trusted_default_writer"), \
             patch.object(WRITER, "write_governance_check", return_value=(102,)) as post:
            self.assertTrue(WRITER.finalize_decision(decision, {"64": frozenset({72})}, WRITER.EvidenceSnapshot({}, {}, {})))
        self.assertEqual(post.call_args.args[1], "failure")

    def test_deferred_draft_reuses_the_dispatcher_pending_check_without_a_write(self) -> None:
        current = self.pull(72, draft=True)
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "check_baseline", return_value=(101,)), \
             patch.object(WRITER, "contract", return_value="pending"), \
             patch.object(WRITER, "write_governance_check") as post:
            self.assertIsNone(WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json", defer_terminal=True))
        post.assert_not_called()

    def test_same_head_rerun_before_final_success_returns_pending(self) -> None:
        current = self.pull(72)
        previous = (WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success"))
        latest = (WRITER.Generation("CI", "x", 44, 3, 1, 2, "completed", "success"), previous[1])
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=[*previous, *latest]), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False):
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_args_list[-1].args[1], "pending")

    def test_later_pending_fence_prevents_terminal_success_post(self) -> None:
        current = self.pull(72)
        generations = [WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")]
        with self.identity(), patch.object(WRITER, "pull", return_value=current), \
             patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
             patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=generations * 2), \
             patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=True):
            WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
        self.assertEqual(post.call_count, 1)

    def test_sensor_terminal_binding_is_unique_and_ambiguous_history_posts_nothing(self) -> None:
        current = self.pull(72)
        generations = [WRITER.Generation("CI", "x", 44, 1, 1, 1, "completed", "success"), WRITER.Generation("release-preflight", "y", 45, 2, 1, 1, "completed", "success")]
        for terminal_count, expected_posts in ((0, 2), (1, 2), (2, 1)):
            with self.subTest(terminal_count=terminal_count), self.identity(), patch.object(WRITER, "pull", return_value=current), \
                 patch.object(WRITER, "write_governance_check", return_value=101) as post, patch.object(WRITER, "contract", return_value="success"), \
                 patch.object(WRITER, "sensor", return_value=77), patch.object(WRITER, "generation", side_effect=generations * 2), \
                 patch.object(WRITER, "final_closer_is_unique", return_value=True), patch.object(WRITER, "check_changed_since", return_value=False), \
                 patch.object(WRITER, "sensor_terminal_check_count", return_value=terminal_count):
                if terminal_count == 2:
                    with self.assertRaises(WRITER.NoPostGovernanceError):
                        WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
                else:
                    WRITER.process(72, {"64": frozenset({72})}, "/tmp/snapshot.json")
            self.assertEqual(post.call_count, expected_posts)

    def test_terminal_write_refuses_a_later_invalidator_fingerprint(self) -> None:
        head = "a" * 40
        baseline = {
            "id": 102, "name": WRITER.CHECK_NAME, "head_sha": head,
            "external_id": WRITER.check_external_id(head), "updated_at": "one",
            "status": "in_progress", "conclusion": None,
            "details_url": "https://github.com/owner/repository/actions/runs/88", "app": {"id": 42},
        }
        later = {**baseline, "updated_at": "two", "details_url": "https://github.com/owner/repository/actions/runs/89"}
        with self.identity(), patch.dict(os.environ, {"KRR_GOVERNANCE_CHECK_APP_ID": "42"}), \
             patch.object(WRITER, "check_run", side_effect=[baseline, later]), \
             patch.object(WRITER, "command") as command:
            with self.assertRaises(WRITER.NoPostGovernanceError):
                WRITER.write_governance_check(
                    head, "success", "old decision", "https://github.com/owner/repository/actions/runs/88",
                    expected_fingerprint=WRITER.check_fingerprint(baseline),
                )
        command.assert_not_called()

    def test_schedule_reserves_two_writes_for_each_missing_terminal_check(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (), "success", "ok", 77, (), "64", "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_INVALIDATED_COUNT": "spoofed"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=True) as finalize:
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(finalize.call_count, 200)

    def test_event_writer_reserves_300_dispatcher_writes_and_ignores_count_spoofing(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (102, "pending"), "success", "ok", 77, (), "64", "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88", "GOVERNANCE_INVALIDATED_COUNT": "not-a-number"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "issues", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", return_value=True) as finalize:
            self.assertEqual(WRITER.main(), 0)
        self.assertEqual(finalize.call_count, 100)

    def test_exceptional_terminal_decisions_cannot_exceed_their_reserved_write_budget(self) -> None:
        snapshot = self.snapshot(tuple(range(1, 301)))
        decision = WRITER.PendingDecision(1, "a" * 40, "b" * 40, (), "failure", "bad", None, None, None, "c" * 64)
        with self.identity(), patch.dict(os.environ, {"GOVERNANCE_DISPATCHER_RUN_ID": "88"}), \
             patch.object(WRITER, "trusted_dispatcher_source", return_value=WRITER.DispatcherSource(88, "schedule", 1)), \
             patch.object(WRITER, "open_snapshot", return_value=snapshot), \
             patch.object(WRITER, "observed_invalidations", return_value=(snapshot, frozenset())), \
             patch.object(WRITER, "evidence_snapshot", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "process", return_value=decision), \
             patch.object(WRITER, "final_evidence_for_pr", return_value=WRITER.EvidenceSnapshot({}, {}, {})), \
             patch.object(WRITER, "finalize_decision", side_effect=WRITER.GovernanceError("failed closed")) as finalize:
            self.assertEqual(WRITER.main(), 1)
        self.assertEqual(finalize.call_count, 200)

    def test_production_first_check_write_waits_before_timestamping(self) -> None:
        WRITER._last_check_write_at = None
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}), \
             patch.object(WRITER.time, "monotonic", return_value=108.1) as monotonic, \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.pace_check_write()
        sleep.assert_called_once_with(8.1)
        monotonic.assert_called_once_with()
        self.assertEqual(WRITER._last_check_write_at, 108.1)

    def test_production_check_writes_are_monotonically_paced(self) -> None:
        WRITER._last_check_write_at = 100.0
        with patch.dict(os.environ, {"GITHUB_ACTIONS": "true"}), \
             patch.object(WRITER.time, "monotonic", side_effect=[100.0, 108.1, 108.1, 116.2]), \
             patch.object(WRITER.time, "sleep") as sleep:
            WRITER.pace_check_write()
            WRITER.pace_check_write()
        self.assertEqual(sleep.call_args_list, [unittest.mock.call(8.1)] * 2)
        self.assertEqual(WRITER._last_check_write_at, 116.2)


if __name__ == "__main__":
    unittest.main()
