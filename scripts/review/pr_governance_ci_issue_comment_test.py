from __future__ import annotations

import re
import unittest
from pathlib import Path


class GovernanceCiAndIssueContractTest(unittest.TestCase):
    def setUp(self) -> None:
        root = Path(__file__).parents[2]
        self.dispatcher = (root / ".github/workflows/pr-governance.yml").read_text(encoding="utf-8")
        self.writer = (root / "scripts/review/pr_governance_status_writer.py").read_text(encoding="utf-8")
        self.writer_workflow = (root / ".github/workflows/pr-governance-status-writer.yml").read_text(encoding="utf-8")

    def test_ci_and_release_generations_bind_path_repo_pr_base_head_and_attempt(self) -> None:
        for name, path in (
            ("CI", ".github/workflows/test-and-build.yml"),
            ("release-preflight", ".github/workflows/release-preflight.yml"),
        ):
            self.assertIn(f'generation(number, base, head, "{name}", "{path}", evidence)', self.writer)
            self.assertIn(f'run.get("name") == name and workflow_path_matches(run.get("path"), path)', self.writer)
        for text in (
            'run.get("event") == "pull_request"', 'item.get("number") == number',
            'run_base.get("sha") == base', 'run_head.get("sha") == head',
            'run.get("workflow_id") == workflow_id', 'type(run.get("run_attempt")) is int',
            'Default-branch CI workflow ID is invalid.', 'if evidence is None:\n        trusted_workflow_blob(path, base, head)', 'return max(matches, key=lambda item:',
        ):
            self.assertIn(text, self.writer)

    def test_governed_pull_request_workflows_have_no_path_filter(self) -> None:
        """Every governed PR path must start CI and release-preflight generation."""
        root = Path(__file__).parents[2]
        for relative in (".github/workflows/test-and-build.yml", ".github/workflows/release-preflight.yml"):
            text = (root / relative).read_text(encoding="utf-8")
            pull_request = re.search(r"(?ms)^  pull_request:\n(?P<body>.*?)(?=^  [A-Za-z0-9_-]+:|\Z)", text)
            self.assertIsNotNone(pull_request, relative)
            body = pull_request.group("body")
            self.assertNotRegex(body, r"(?m)^    paths(?:-ignore)?:")

        ci_text = (root / ".github/workflows/test-and-build.yml").read_text(encoding="utf-8")
        push = re.search(r"(?ms)^  push:\n(?P<body>.*?)(?=^  pull_request:)", ci_text)
        self.assertIsNotNone(push)
        self.assertRegex(push.group("body"), r"(?m)^    paths:")

    def test_success_re_reads_ci_generation_from_one_final_shared_snapshot_before_post(self) -> None:
        self.assertIn("final_evidence_for_pr(decision.head, initial_evidence)", self.writer)
        # The final shared snapshot must build the query through urlencode so
        # the bounded endpoint remains correct even when a future parameter
        # contains characters requiring escaping.
        self.assertIn(
            '"repos/{REPOSITORY}/actions/runs?" + urlencode({"head_sha": head, "per_page": 100})',
            self.writer,
        )
        self.assertIn("def finalize_decision", self.writer)
        self.assertIn("latest != generations", self.writer)
        self.assertIn("CI generation changed during governance revalidation.", self.writer)
        self.assertIn("check_changed_since(decision.head, decision.pending_check_fingerprint)", self.writer)

    def test_default_branch_writer_uses_protected_environment_and_split_tokens(self) -> None:
        self.assertIn("environment: pr-governance", self.writer_workflow)
        self.assertIn("Writer SHA does not match default branch head.", self.writer_workflow)
        self.assertIn("Writer workflow differs from default branch.", self.writer_workflow)
        self.assertIn("ref: ${{ github.sha }}", self.writer_workflow)
        self.assertIn("bootstrap-validation bound this immutable dispatch SHA", self.writer_workflow)
        self.assertIn("persist-credentials: false", self.writer_workflow)
        self.assertIn("permission-checks: write", self.writer_workflow)
        self.assertIn("def read_environment(*, default_token: bool = False)", self.writer)
        self.assertIn('return {"GH_TOKEN": token, "PATH": os.environ["PATH"]}', self.writer)
        self.assertIn("environment = {\"GH_TOKEN\": token, \"PATH\": environment[\"PATH\"]}", self.writer)
        self.assertIn("DEFAULT_READ_TOKEN: ${{ github.token }}", self.writer_workflow)
        self.assertIn("Create read-only governance App token", self.writer_workflow)

    def test_issue_comment_and_issue_events_are_bounded_to_one_default_branch_arbiter(self) -> None:
        self.assertIn("issue_comment:", self.dispatcher)
        self.assertIn("issues:", self.dispatcher)
        self.assertIn("workflow_dispatch:", self.writer_workflow)
        # The dispatcher passes only its immutable run ID; it never passes a
        # caller-controlled count, PR ref, SHA, or target set to the writer.
        self.assertNotIn("invalidated_count", self.writer_workflow)
        self.assertNotIn("invalidated_count", self.dispatcher)
        self.assertIn("dispatcher_run_id:", self.writer_workflow)
        self.assertNotRegex(self.writer_workflow, r"inputs\.pr(?:\s|}}|\])")

    def test_ready_and_merge_harness_rechecks_the_same_gate_immediately_before_merge(self) -> None:
        root = Path(__file__).parents[2]
        for relative in (
            "AGENTS.md",
            ".codex/skills/impl-release/SKILL.md",
            ".agents/skills/impl-release/SKILL.md",
            ".codex/skills/create_pull_request/SKILL.md",
            ".agents/skills/create_pull_request/SKILL.md",
        ):
            with self.subTest(path=relative):
                text = (root / relative).read_text(encoding="utf-8")
                self.assertIn("gh pr merge", text)
                self.assertIn("just pr-ready-check", text)
                self.assertIn("直前", text)


if __name__ == "__main__":
    unittest.main()
