#!/usr/bin/env python3
"""Default-branch-only arbiter for the KRR PR-governance Check Run.

The dispatcher intentionally does no decision making.  It invalidates every
current head and starts this program once; this program serializes all final
decisions.  Keeping the API boundary here makes the two token scopes explicit:
the ambient Actions token is read-only and the App token is used only for the
single Check Run PATCH helper.
"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from typing import Any
from urllib.parse import parse_qs, urlencode, urlparse, urlunparse


REPOSITORY = os.environ.get("GITHUB_REPOSITORY", "")
SERVER_URL = os.environ.get("GITHUB_SERVER_URL", "")
WRITER_RUN_ID = os.environ.get("GITHUB_RUN_ID", "")
CHECK_NAME = "KRR / PR governance (trusted check)"
CHECK_EXTERNAL_PREFIX = "krr-governance/v1/"
CHECK_WRITE_INTERVAL_SECONDS = 8.1
DISPATCHER_NAME = "PR governance dispatcher"
DISPATCHER_PATH = ".github/workflows/pr-governance.yml"
WRITER_WORKFLOW_PATH = ".github/workflows/pr-governance-status-writer.yml"
DISPATCHER_EVENTS = frozenset({"pull_request_target", "issue_comment", "issues", "schedule", "workflow_run"})
SHA = re.compile(r"[0-9a-fA-F]{40}")
NUMBER = re.compile(r"[1-9][0-9]*")
CLOSING = re.compile(
    r"\b(?:close(?:s|d)?|fix(?:es|ed)?|resolve(?:s|d)?)\b\s+(?:"
    r"#(?P<short>[1-9][0-9]*)|"
    r"https://github\.com/(?P<owner>[A-Za-z0-9_.-]+)/(?P<repository>[A-Za-z0-9_.-]+)/issues/(?P<full>[1-9][0-9]*)"
    r")\b",
    re.I,
)
_last_check_write_at: float | None = None


class GovernanceError(RuntimeError):
    pass


class NoPostGovernanceError(GovernanceError):
    """Fail a PR without adding another terminal status to an ambiguous latch."""


@dataclass(frozen=True)
class DispatcherSource:
    identifier: int
    event: str
    attempt: int


def read_environment(*, default_token: bool = False) -> dict[str, str]:
    """The verifier and every GET see only the read token, never App secrets."""
    token = os.environ.get("DEFAULT_READ_TOKEN" if default_token else "GH_TOKEN", "")
    if not token:
        raise GovernanceError("Read token is missing.")
    return {"GH_TOKEN": token, "PATH": os.environ["PATH"]}


def command(arguments: list[str], *, check_write: bool = False, default_token: bool = False) -> str:
    environment = os.environ.copy()
    if check_write:
        token = environment.get("CHECK_WRITE_TOKEN", "")
        if not token:
            raise GovernanceError("Check writer token is missing.")
        # Do not permit the read token to cross the write boundary.
        environment = {"GH_TOKEN": token, "PATH": environment["PATH"]}
    else:
        environment = read_environment(default_token=default_token)
    result = subprocess.run(
        ["gh", "api", *arguments], capture_output=True, text=True,
        check=False, env=environment,
    )
    if result.returncode != 0:
        raise GovernanceError("GitHub API request failed.")
    return result.stdout


def api_json(endpoint: str, *, default_token: bool = False) -> Any:
    try:
        return json.loads(command([endpoint], default_token=default_token))
    except json.JSONDecodeError as error:
        raise GovernanceError("GitHub API response is not JSON.") from error


def pages(endpoint: str, *, default_token: bool = False) -> list[list[dict[str, Any]]]:
    try:
        value = json.loads(command(["--paginate", "--slurp", endpoint], default_token=default_token))
    except json.JSONDecodeError as error:
        raise GovernanceError("GitHub pagination response is not JSON.") from error
    if not isinstance(value, list) or not all(isinstance(page, list) for page in value):
        raise GovernanceError("GitHub pagination response is invalid.")
    if not all(all(isinstance(item, dict) for item in page) for page in value):
        raise GovernanceError("GitHub pagination item is invalid.")
    return value


def object_pages(endpoint: str) -> list[dict[str, Any]]:
    try:
        value = json.loads(command(["--paginate", "--slurp", endpoint]))
    except json.JSONDecodeError as error:
        raise GovernanceError("GitHub pagination response is not JSON.") from error
    if not isinstance(value, list) or not all(isinstance(page, dict) for page in value):
        raise GovernanceError("GitHub pagination response is invalid.")
    return value


def open_pulls() -> list[int]:
    numbers: list[int] = []
    seen: set[int] = set()
    for page in pages(f"repos/{REPOSITORY}/pulls?state=open&per_page=100"):
        for pull in page:
            number = pull.get("number")
            if type(number) is not int or number < 1 or number in seen or pull.get("state") != "open":
                raise GovernanceError("Open pull request response is invalid.")
            seen.add(number)
            numbers.append(number)
    return numbers


@dataclass(frozen=True)
class OpenSnapshot:
    numbers: tuple[int, ...]
    claimants: dict[str, frozenset[int]]
    pull_requests: tuple[dict[str, object], ...]


def open_snapshot() -> OpenSnapshot:
    """Take one complete O(N) open-PR snapshot for a serialized writer run."""
    numbers: list[int] = []
    # Validate the complete API stream before applying the local-governance
    # scope.  A fork must not become an Issue claimant, but it also must not
    # hide a malformed duplicate response on a later page.
    seen_all: set[int] = set()
    claimants: dict[str, set[int]] = {}
    pull_requests: list[dict[str, object]] = []
    for page in pages(f"repos/{REPOSITORY}/pulls?state=open&per_page=100"):
        for item in page:
            number = item.get("number")
            body = item.get("body")
            base = item.get("base") if isinstance(item, dict) else None
            head = item.get("head") if isinstance(item, dict) else None
            base_repository = base.get("repo") if isinstance(base, dict) else None
            head_repository = head.get("repo") if isinstance(head, dict) else None
            head_sha = head.get("sha") if isinstance(head, dict) else None
            draft = item.get("draft") if isinstance(item, dict) else None
            # GitHub represents an absent PR description as JSON null.  It is
            # an invalid closer for that individual PR, not a malformed
            # repository-wide snapshot which would strand every other head.
            if body is None:
                body = ""
            if type(number) is not int or number < 1 or number in seen_all or item.get("state") != "open" or not isinstance(body, str) or not isinstance(draft, bool):
                raise GovernanceError("Open pull request snapshot is invalid.")
            seen_all.add(number)
            # Forks and non-default-base PRs cannot be governed by this
            # default-branch App; do not let them claim a local canonical Issue.
            if (
                not isinstance(base_repository, dict) or not isinstance(head_repository, dict)
                or base_repository.get("full_name") != REPOSITORY or head_repository.get("full_name") != REPOSITORY
                or base.get("ref") != os.environ.get("GITHUB_REF_NAME") or not isinstance(head_sha, str) or not SHA.fullmatch(head_sha)
            ):
                continue
            numbers.append(number)
            pull_requests.append({"number": number, "isDraft": draft, "body": body, "head_sha": head_sha})
            for issue in closing_issues(body):
                claimants.setdefault(issue, set()).add(number)
    return OpenSnapshot(tuple(numbers), {issue: frozenset(values) for issue, values in claimants.items()}, tuple(pull_requests))


def pull(number: int, *, default_token: bool = False) -> dict[str, Any]:
    """Read one governed PR using the least-privileged read boundary.

    The initial decision can use the installation read token.  Only the
    terminal closer fence needs the repository workflow token: that isolates
    the small final-CAS budget from the 300-PR verifier/evidence scan.
    """
    value = api_json(f"repos/{REPOSITORY}/pulls/{number}", default_token=default_token)
    base = value.get("base") if isinstance(value, dict) else None
    head = value.get("head") if isinstance(value, dict) else None
    base_repository = base.get("repo") if isinstance(base, dict) else None
    head_repository = head.get("repo") if isinstance(head, dict) else None
    if (
        not isinstance(value, dict) or value.get("number") != number or value.get("state") != "open"
        or type(value.get("draft")) is not bool or not isinstance(base, dict) or not isinstance(head, dict)
        or not isinstance(base.get("sha"), str) or not isinstance(head.get("sha"), str)
        or base.get("ref") != os.environ.get("GITHUB_REF_NAME")
        or not SHA.fullmatch(base["sha"]) or not SHA.fullmatch(head["sha"])
        or not isinstance(base_repository, dict) or not isinstance(head_repository, dict)
        or base_repository.get("full_name") != REPOSITORY or head_repository.get("full_name") != REPOSITORY
    ):
        raise GovernanceError("Pull request is invalid.")
    return value


def canonical_issue(body: object) -> str | None:
    if not isinstance(body, str):
        raise GovernanceError("Pull request body is invalid.")
    issues = closing_issues(body)
    if len(issues) != 1:
        return None
    return next(iter(issues))


def closing_issues(body: str) -> set[str]:
    """Return only closing references that target this repository.

    Short ``#123`` forms are necessarily local.  A fully-qualified URL must
    name ``GITHUB_REPOSITORY``; otherwise an unrelated repository's Issue
    could poison this repository's canonical-closer fence.
    """
    repository = REPOSITORY.casefold()
    issues: set[str] = set()
    for match in CLOSING.finditer(body):
        short, owner, name, full = match.group("short", "owner", "repository", "full")
        if short is not None:
            issues.add(short)
        elif full is not None and f"{owner}/{name}".casefold() == repository:
            issues.add(full)
    return issues


def workflow_path_matches(value: object, expected: str) -> bool:
    """Accept GitHub's documented ``path@ref`` representation safely."""
    if value == expected:
        return True
    if not isinstance(value, str) or not value.startswith(expected + "@"):
        return False
    ref = value[len(expected) + 1:]
    return bool(
        re.fullmatch(r"[A-Za-z0-9._/-]+", ref)
        and ref not in {".", ".."}
        and not ref.startswith("/")
        and "//" not in ref
        and all(part not in {"", ".", ".."} for part in ref.split("/"))
    )


def trusted_dispatcher_source(identifier: int) -> DispatcherSource:
    """Bind dispatch input to one immutable default-branch dispatcher run."""
    if type(identifier) is not int or identifier < 1:
        raise GovernanceError("Dispatcher run ID is invalid.")
    expected_head = os.environ.get("GITHUB_SHA", "")
    if not SHA.fullmatch(expected_head):
        raise GovernanceError("Writer default-branch SHA is invalid.")
    value = api_json(f"repos/{REPOSITORY}/actions/runs/{identifier}", default_token=True)
    repository = value.get("repository") if isinstance(value, dict) else None
    attempt = value.get("run_attempt") if isinstance(value, dict) else None
    status = value.get("status") if isinstance(value, dict) else None
    if not (
        isinstance(value, dict) and value.get("id") == identifier and value.get("name") == DISPATCHER_NAME
        and workflow_path_matches(value.get("path"), DISPATCHER_PATH)
        and value.get("event") in DISPATCHER_EVENTS and value.get("head_sha") == expected_head
        and isinstance(repository, dict) and repository.get("full_name") == REPOSITORY
        and type(value.get("run_number")) is int and value["run_number"] > 0
        and type(attempt) is int and attempt > 0
        and isinstance(status, str) and status in {"requested", "queued", "waiting", "in_progress", "completed"}
    ):
        raise GovernanceError("Dispatcher source is not a trusted default-branch run.")
    return DispatcherSource(identifier, value["event"], attempt)


def rebind_trusted_default_writer() -> None:
    """Refuse a terminal Check Run write after the trusted default moved."""
    expected_head = os.environ.get("GITHUB_SHA", "")
    if not SHA.fullmatch(expected_head):
        raise GovernanceError("Writer default-branch SHA is invalid.")
    repository = api_json(f"repos/{REPOSITORY}", default_token=True)
    branch = repository.get("default_branch") if isinstance(repository, dict) else None
    if not isinstance(branch, str) or not re.fullmatch(r"[A-Za-z0-9._/-]+", branch):
        raise GovernanceError("Repository default branch is invalid.")
    reference = api_json(f"repos/{REPOSITORY}/git/ref/heads/{branch}", default_token=True)
    current_head = reference.get("object", {}).get("sha") if isinstance(reference, dict) and isinstance(reference.get("object"), dict) else None
    if current_head != expected_head:
        raise GovernanceError("Trusted default branch advanced while governance was running.")
    default_blob = api_json(
        f"repos/{REPOSITORY}/contents/{WRITER_WORKFLOW_PATH}?ref={branch}", default_token=True,
    )
    immutable_blob = api_json(
        f"repos/{REPOSITORY}/contents/{WRITER_WORKFLOW_PATH}?ref={expected_head}", default_token=True,
    )
    default_digest = default_blob.get("sha") if isinstance(default_blob, dict) else None
    immutable_digest = immutable_blob.get("sha") if isinstance(immutable_blob, dict) else None
    if not isinstance(default_digest, str) or not SHA.fullmatch(default_digest) or default_digest != immutable_digest:
        raise GovernanceError("Trusted dispatcher workflow changed while governance was running.")


def trusted_workflow_blob(path: str, base: str, head: str, cache: dict[tuple[str, str], str] | None = None) -> None:
    """Require the source workflow bytes to equal base, PR head, and writer."""
    default_ref = os.environ.get("GITHUB_SHA", "")
    if not SHA.fullmatch(default_ref):
        raise GovernanceError("Writer default-branch SHA is invalid.")
    digests: list[str] = []
    for ref in (default_ref, base, head):
        cache_key = (path, ref)
        digest = cache.get(cache_key) if cache is not None else None
        if digest is None:
            blob = api_json(f"repos/{REPOSITORY}/contents/{path}?ref={ref}")
            digest = blob.get("sha") if isinstance(blob, dict) else None
            if not isinstance(digest, str) or not SHA.fullmatch(digest):
                raise GovernanceError("Default-branch workflow blob is invalid.")
            if cache is not None:
                cache[cache_key] = digest
        digests.append(digest)
    if len(set(digests)) != 1:
        raise GovernanceError("Workflow differs from the trusted default branch.")


def check_external_id(head: str) -> str:
    if not SHA.fullmatch(head):
        raise GovernanceError("Check Run head SHA is invalid.")
    return CHECK_EXTERNAL_PREFIX + head.lower()


def check_app_id() -> int:
    value = os.environ.get("KRR_GOVERNANCE_CHECK_APP_ID", "")
    if not NUMBER.fullmatch(value):
        raise GovernanceError("Check Run App ID is invalid.")
    return int(value)


def _valid_check(value: object, head: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise GovernanceError("Check Run response is invalid.")
    app = value.get("app")
    if (
        type(value.get("id")) is not int or value.get("name") != CHECK_NAME
        or value.get("head_sha") != head or value.get("external_id") != check_external_id(head)
        or not isinstance(app, dict) or app.get("id") != check_app_id()
        or not isinstance(value.get("updated_at"), str)
    ):
        raise GovernanceError("Check Run identity is invalid.")
    return value


def checks(head: str, *, default_token: bool = False) -> list[dict[str, Any]]:
    values: list[dict[str, Any]] = []
    query = urlencode({"check_name": CHECK_NAME, "app_id": check_app_id(), "filter": "all", "per_page": 100})
    for page in object_pages(f"repos/{REPOSITORY}/commits/{head}/check-runs?{query}"):
        runs = page.get("check_runs")
        if not isinstance(runs, list) or not all(isinstance(item, dict) for item in runs):
            raise GovernanceError("Check Run pagination is invalid.")
        values.extend(runs)
    return values


def check_run(head: str) -> dict[str, Any] | None:
    matching: list[dict[str, Any]] = []
    for item in checks(head):
        if item.get("name") != CHECK_NAME:
            continue
        app = item.get("app")
        # GitHub may return same-name checks from other Apps even with app_id
        # filtering. They are not governance candidates and cannot DoS ours.
        if not isinstance(app, dict) or app.get("id") != check_app_id():
            continue
        if item.get("head_sha") != head:
            raise GovernanceError("Check Run head mismatch.")
        matching.append(_valid_check(item, head))
    if len(matching) > 1:
        raise GovernanceError("Multiple trusted Check Runs exist for one head.")
    return matching[0] if matching else None


def pace_check_write() -> None:
    """Keep production Check Run mutations below the secondary-rate burst limit."""
    global _last_check_write_at
    if os.environ.get("GITHUB_ACTIONS") != "true":
        return
    if _last_check_write_at is None:
        # A fresh writer may begin immediately after dispatcher mutations.
        # Delay before its first request so that hand-off cannot burst.
        time.sleep(CHECK_WRITE_INTERVAL_SECONDS)
        _last_check_write_at = time.monotonic()
        return
    now = time.monotonic()
    delay = CHECK_WRITE_INTERVAL_SECONDS - (now - _last_check_write_at)
    if delay > 0:
        time.sleep(delay)
        now = time.monotonic()
    _last_check_write_at = now


def write_check(
    head: str,
    *,
    state: str,
    description: str,
    details_url: str,
    existing: dict[str, Any] | None = None,
    expected_fingerprint: tuple[object, ...] | None = None,
) -> dict[str, Any]:
    if state not in {"in_progress", "success", "failure"}:
        raise GovernanceError("Check Run state is invalid.")
    if expected_fingerprint is not None:
        current = check_run(head)
        if check_fingerprint(current) != expected_fingerprint:
            raise NoPostGovernanceError("Check Run changed before terminal write.")
        existing = current
    if existing is None:
        arguments = ["--method", "POST", f"repos/{REPOSITORY}/check-runs", "-f", f"name={CHECK_NAME}", "-f", f"head_sha={head}", "-f", f"external_id={check_external_id(head)}", "-f", "status=in_progress", "-f", f"details_url={details_url}", "-f", f"output[title]={CHECK_NAME}", "-f", f"output[summary]={description}"]
    else:
        identifier = _valid_check(existing, head)["id"]
        arguments = ["--method", "PATCH", f"repos/{REPOSITORY}/check-runs/{identifier}", "-f", f"details_url={details_url}", "-f", f"output[title]={CHECK_NAME}", "-f", f"output[summary]={description}"]
        if state == "in_progress":
            arguments.extend(["-f", "status=in_progress"])
        else:
            arguments.extend(["-f", "status=completed", "-f", f"conclusion={state}"])
    pace_check_write()
    try:
        value = json.loads(command(arguments, check_write=True))
    except json.JSONDecodeError as error:
        raise GovernanceError("Check Run write response is not JSON.") from error
    checked = _valid_check(value, head)
    expected_status = "in_progress" if state == "in_progress" else "completed"
    expected_conclusion = None if state == "in_progress" else state
    if checked.get("status") != expected_status or checked.get("conclusion") != expected_conclusion or checked.get("details_url") != details_url:
        raise GovernanceError("Check Run write state is invalid.")
    reread = check_run(head)
    if reread is None or check_fingerprint(reread) != check_fingerprint(checked):
        raise GovernanceError("Check Run changed after write.")
    return reread


def _same_check_evidence(current: str, desired: str) -> bool:
    """Compare immutable Check Run evidence, ignoring the writer run URL."""
    current_query = parse_qs(urlparse(current).query, keep_blank_values=True)
    desired_query = parse_qs(urlparse(desired).query, keep_blank_values=True)
    required = {
        "source_run_id", "ci_workflow_id", "ci_run_id", "ci_run_number",
        "ci_run_attempt", "ci_status", "ci_conclusion", "release_workflow_id",
        "release_run_id", "release_run_number", "release_run_attempt",
        "release_status", "release_conclusion", "pr_base_sha", "pr_head_sha",
    }
    return {key: current_query.get(key) for key in required} == {key: desired_query.get(key) for key in required}


def check_fingerprint(value: dict[str, Any] | None) -> tuple[object, ...]:
    if value is None:
        return ()
    checked = _valid_check(value, value.get("head_sha", ""))
    return (checked["id"], checked["updated_at"], checked.get("status"), checked.get("conclusion"), checked.get("details_url"), checked["external_id"])


def write_governance_check(
    head: str,
    state: str,
    description: str,
    target_url: str,
    *,
    expected_fingerprint: tuple[object, ...] | None = None,
) -> tuple[object, ...]:
    """Compatibility entry point backed exclusively by one external Check Run."""
    mapped = {"pending": "in_progress", "success": "success", "failure": "failure"}.get(state)
    if mapped is None:
        raise GovernanceError("Governance Check Run state is invalid.")
    existing = check_run(head)
    if expected_fingerprint is not None and check_fingerprint(existing) != expected_fingerprint:
        raise NoPostGovernanceError("Check Run changed before governance write.")
    if existing is None and mapped != "in_progress":
        # Check Runs cannot be created directly in a completed state.
        existing = write_check(
            head,
            state="in_progress",
            description="Trusted governance revalidation is running.",
            details_url=target_url,
            expected_fingerprint=expected_fingerprint,
        )
        expected_fingerprint = check_fingerprint(existing)
    value = write_check(
        head,
        state=mapped,
        description=description,
        details_url=target_url,
        existing=existing,
        expected_fingerprint=expected_fingerprint,
    )
    return check_fingerprint(value)


def check_baseline(head: str) -> tuple[object, ...]:
    return check_fingerprint(check_run(head))


def check_changed_since(head: str, baseline: tuple[object, ...]) -> bool:
    return check_fingerprint(check_run(head)) != baseline


def sensor_terminal_check_count(head: str, sensor_id: int) -> int:
    value = check_run(head)
    if value is None:
        return 0
    details = value.get("details_url")
    source = parse_qs(urlparse(details).query).get("source_run_id") if isinstance(details, str) else None
    return int(value.get("status") == "completed" and source == [str(sensor_id)])


def check_fence(head: str, baseline: tuple[object, ...], sensor_id: int, *, desired_state: str | None = None, desired_target: str | None = None) -> tuple[bool, int, bool]:
    value = check_run(head)
    if value is None:
        return baseline != (), 0, False
    newer = check_fingerprint(value) != baseline
    details = value.get("details_url")
    expected_status = "in_progress" if desired_state == "pending" else "completed"
    expected_conclusion = None if desired_state == "pending" else desired_state
    exact = bool(desired_state and value.get("status") == expected_status and value.get("conclusion") == expected_conclusion and isinstance(details, str) and desired_target and _same_check_evidence(details, desired_target))
    source = parse_qs(urlparse(details).query).get("source_run_id") if isinstance(details, str) else None
    terminal_count = int(value.get("status") == "completed" and source == [str(sensor_id)])
    return newer, terminal_count, exact


def dispatcher_invalidation_url(source: DispatcherSource, carry_pending: int) -> str:
    if carry_pending not in {0, 1}:
        raise GovernanceError("Dispatcher carry marker is invalid.")
    return (
        f"{SERVER_URL}/{REPOSITORY}/actions/runs/{source.identifier}?"
        + urlencode({"dispatcher_run_id": str(source.identifier), "carry_pending": str(carry_pending)})
    )


def observed_invalidations(snapshot: OpenSnapshot, source: DispatcherSource) -> frozenset[int]:
    """Return exactly the carry targets marked by this dispatcher invalidation."""
    expected_fresh = dispatcher_invalidation_url(source, 0)
    expected_carry = dispatcher_invalidation_url(source, 1)
    carry: set[int] = set()
    for pull_request in snapshot.pull_requests:
        number = pull_request.get("number")
        head = pull_request.get("head_sha")
        draft = pull_request.get("isDraft")
        if type(number) is not int or number < 1 or type(draft) is not bool or not isinstance(head, str) or not SHA.fullmatch(head):
            raise GovernanceError("Open pull request head is invalid.")
        value = check_run(head)
        if value is None:
            raise GovernanceError("Dispatcher invalidation Check Run is missing.")
        if value.get("status") != "in_progress" or value.get("conclusion") is not None:
            raise GovernanceError("Dispatcher invalidation Check Run state is invalid.")
        details = value.get("details_url")
        if details not in {expected_fresh, expected_carry}:
            raise GovernanceError("Dispatcher invalidation Check Run evidence is stale or foreign.")
        if details == expected_carry:
            if draft:
                raise GovernanceError("Draft pull request cannot carry a terminal governance decision.")
            carry.add(number)
    return frozenset(carry)


def sensor(number: int, base: str, head: str, evidence: EvidenceSnapshot | None = None) -> int:
    if evidence is None:
        trusted_workflow_blob(".github/workflows/pr-governance-review-events.yml", base, head)
    candidates: list[tuple[int, int, int]] = []
    for event in ("pull_request", "pull_request_review", "pull_request_review_comment"):
        if evidence is None:
            endpoint = (
                f"repos/{REPOSITORY}/actions/workflows/pr-governance-review-events.yml/"
                f"runs?event={event}&per_page=100"
            )
            run_pages = object_pages(endpoint)
        else:
            run_pages = [{"workflow_runs": list(evidence.sensor_runs.get(event, ()))}]
        for page in run_pages:
            runs = page.get("workflow_runs")
            if not isinstance(runs, list):
                raise GovernanceError("Review sensor response is invalid.")
            for run in runs:
                if not isinstance(run, dict):
                    raise GovernanceError("Review sensor run is invalid.")
                pulls = run.get("pull_requests")
                if not isinstance(pulls, list) or len(pulls) != 1 or not isinstance(pulls[0], dict):
                    continue
                current = pulls[0]
                run_base, run_head = current.get("base"), current.get("head")
                repository = run.get("repository")
                if not (
                    run.get("name") == "PR governance review sensor" and run.get("event") == event
                    and workflow_path_matches(run.get("path"), ".github/workflows/pr-governance-review-events.yml")
                    and run.get("head_sha") == head and run.get("run_attempt") == 1
                    and isinstance(repository, dict) and repository.get("full_name") == REPOSITORY
                    and current.get("number") == number and isinstance(run_base, dict) and isinstance(run_head, dict)
                    and run_base.get("sha") == base and run_head.get("sha") == head
                    and isinstance(run_base.get("repo"), dict) and isinstance(run_head.get("repo"), dict)
                    and run_base["repo"].get("full_name") == REPOSITORY
                    and run_head["repo"].get("full_name") == REPOSITORY
                    and type(run.get("id")) is int and type(run.get("run_number")) is int
                ):
                    continue
                candidates.append((run["run_number"], run["id"], run["id"]))
    if not candidates:
        raise GovernanceError("No current trusted review sensor exists.")
    return max(candidates)[2]


@dataclass(frozen=True)
class Generation:
    name: str
    path: str
    workflow_id: int
    identifier: int
    number: int
    attempt: int
    status: str
    conclusion: object


@dataclass(frozen=True)
class EvidenceSnapshot:
    """A complete, run-wide cache of workflow-run pages.

    The arbiter deliberately lists each governed workflow once, rather than
    making every one of 300 PR decisions page the same run history again.
    Per-PR workflow-byte and head/base checks remain fail-closed below.
    """
    sensor_runs: dict[str, tuple[dict[str, Any], ...]]
    workflow_ids: dict[str, int]
    workflow_runs: dict[str, tuple[dict[str, Any], ...]]
    workflow_blobs: dict[tuple[str, str], str] = field(default_factory=dict, compare=False)


@dataclass(frozen=True)
class PendingDecision:
    number: int
    head: str
    base: str
    pending_check_fingerprint: tuple[object, ...]
    state: str
    description: str
    sensor_id: int | None
    generations: tuple[Generation, Generation] | None
    issue: str | None


def evidence_snapshot() -> EvidenceSnapshot:
    sensor_runs: dict[str, tuple[dict[str, Any], ...]] = {}
    for event in ("pull_request", "pull_request_review", "pull_request_review_comment"):
        endpoint = f"repos/{REPOSITORY}/actions/workflows/pr-governance-review-events.yml/runs?event={event}&per_page=100"
        values: list[dict[str, Any]] = []
        for page in object_pages(endpoint):
            runs = page.get("workflow_runs")
            if not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs):
                raise GovernanceError("Review sensor response is invalid.")
            values.extend(runs)
        sensor_runs[event] = tuple(values)
    workflow_ids: dict[str, int] = {}
    workflow_runs: dict[str, tuple[dict[str, Any], ...]] = {}
    for path in (".github/workflows/test-and-build.yml", ".github/workflows/release-preflight.yml"):
        workflow = api_json(f"repos/{REPOSITORY}/actions/workflows/{path.rsplit('/', 1)[-1]}")
        workflow_id = workflow.get("id") if isinstance(workflow, dict) else None
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Default-branch CI workflow ID is invalid.")
        values = []
        for page in object_pages(f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?event=pull_request&per_page=100"):
            runs = page.get("workflow_runs")
            if not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs):
                raise GovernanceError("CI generation response is invalid.")
            values.extend(runs)
        workflow_ids[path] = workflow_id
        workflow_runs[path] = tuple(values)
    return EvidenceSnapshot(sensor_runs, workflow_ids, workflow_runs)


def bounded_runs(endpoint: str) -> tuple[dict[str, Any], ...]:
    """Fully page a single PR/head, never the repository-wide history."""
    values: list[dict[str, Any]] = []
    for page in object_pages(endpoint):
        runs = page.get("workflow_runs")
        if not isinstance(runs, list) or not all(isinstance(run, dict) for run in runs):
            raise GovernanceError("Bounded workflow-run response is invalid.")
        values.extend(runs)
    return tuple(values)


def final_evidence_for_pr(head: str, initial: EvidenceSnapshot) -> EvidenceSnapshot:
    """Re-read only this PR head immediately before its terminal status post."""
    sensor_all = bounded_runs(
        f"repos/{REPOSITORY}/actions/workflows/pr-governance-review-events.yml/runs?head_sha={head}&per_page=100"
    )
    sensor_runs = {
        event: tuple(run for run in sensor_all if run.get("event") == event)
        for event in ("pull_request", "pull_request_review", "pull_request_review_comment")
    }
    workflow_runs: dict[str, tuple[dict[str, Any], ...]] = {}
    for path, workflow_id in initial.workflow_ids.items():
        workflow_runs[path] = bounded_runs(
            f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?event=pull_request&head_sha={head}&per_page=100"
        )
    # Reuse only immutable default workflow IDs; byte guards have their own
    # per-head cache and still check each source workflow before use.
    return EvidenceSnapshot(sensor_runs, initial.workflow_ids, workflow_runs, initial.workflow_blobs)


def generation(number: int, base: str, head: str, name: str, path: str, evidence: EvidenceSnapshot | None = None) -> Generation:
    # A workflow run can originate from a PR-modified YAML file.
    if evidence is None:
        trusted_workflow_blob(path, base, head)
    if evidence is None:
        workflow = api_json(f"repos/{REPOSITORY}/actions/workflows/{path.rsplit('/', 1)[-1]}")
        workflow_id = workflow.get("id") if isinstance(workflow, dict) else None
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Default-branch CI workflow ID is invalid.")
        run_pages = object_pages(f"repos/{REPOSITORY}/actions/workflows/{workflow_id}/runs?event=pull_request&per_page=100")
    else:
        workflow_id = evidence.workflow_ids.get(path)
        if type(workflow_id) is not int or workflow_id < 1:
            raise GovernanceError("Cached CI workflow ID is invalid.")
        run_pages = [{"workflow_runs": list(evidence.workflow_runs.get(path, ()))}]
    matches: list[Generation] = []
    for page in run_pages:
        runs = page.get("workflow_runs")
        if not isinstance(runs, list):
            raise GovernanceError("CI generation response is invalid.")
        for run in runs:
            if not isinstance(run, dict):
                raise GovernanceError("CI run is invalid.")
            pulls = run.get("pull_requests")
            if not isinstance(pulls, list) or len(pulls) != 1 or not isinstance(pulls[0], dict):
                continue
            item = pulls[0]
            run_base, run_head, repository = item.get("base"), item.get("head"), run.get("repository")
            if not (
                run.get("name") == name and workflow_path_matches(run.get("path"), path) and run.get("event") == "pull_request"
                and run.get("head_sha") == head and isinstance(repository, dict)
                and repository.get("full_name") == REPOSITORY and item.get("number") == number
                and isinstance(run_base, dict) and isinstance(run_head, dict)
                and run_base.get("sha") == base and run_head.get("sha") == head
                and isinstance(run_base.get("repo"), dict) and isinstance(run_head.get("repo"), dict)
                and run_base["repo"].get("full_name") == REPOSITORY
                and run_head["repo"].get("full_name") == REPOSITORY
                and run.get("workflow_id") == workflow_id and type(run.get("id")) is int and type(run.get("run_number")) is int
                and type(run.get("run_attempt")) is int and isinstance(run.get("status"), str)
            ):
                continue
            matches.append(Generation(name, path, workflow_id, run["id"], run["run_number"], run["run_attempt"], run["status"], run.get("conclusion")))
    if not matches:
        raise GovernanceError("Current CI generation is missing.")
    return max(matches, key=lambda item: (item.number, item.attempt, item.identifier))


def verdict(value: Generation) -> str:
    if value.status in {"queued", "in_progress", "waiting", "requested"}:
        return "pending"
    if value.status == "completed" and value.conclusion == "success":
        return "success"
    if value.status == "completed":
        return "failure"
    raise GovernanceError("CI generation status is invalid.")


def contract(number: int, base: str, head: str, branch: str, draft: bool, snapshot_path: str) -> str:
    issue = subprocess.run(
        [sys.executable, "scripts/hooks/verify_push_issue.py", "--pr-number", str(number),
         "--pr-base-sha", base, "--pr-head-sha", head, "--pr-branch", branch,
         "--repository", REPOSITORY, "--trusted-default-sha", os.environ.get("GITHUB_SHA", "")], check=False, env=read_environment(),
    )
    if issue.returncode != 0:
        return "failure"
    if draft:
        return "pending"
    ready = subprocess.run(
        [sys.executable, "scripts/review/verify_pr_ready.py", "--pr", str(number),
         "--expected-base-sha", base, "--expected-head-sha", head, "--allow-ready",
         # This writer is producing the trusted Check Run itself.  The
         # verifier still checks its App binding, latch/source, CI, Issue and
         # review evidence, but must not require this output to already be
         # completed/success while it is deliberately in_progress.
         "--exclude-trusted-governance-check", "--open-pull-snapshot", snapshot_path], check=False,
        env=read_environment(),
    )
    return "success" if ready.returncode == 0 else "failure"


def final_closer_is_unique(number: int, issue: str, base: str, head: str, claimants: dict[str, frozenset[int]]) -> bool:
    current = pull(number, default_token=True)
    actual_issue = canonical_issue(current.get("body"))
    if actual_issue != issue or current["base"]["sha"] != base or current["head"]["sha"] != head:
        return False
    # A malformed multi-Issue closer is a claimant for every Issue it names;
    # the one complete snapshot prevents O(N^2) GETs for a 300+ PR run.
    return claimants.get(issue) == frozenset({number})


def target_url(*, source_run_id: int | None = None, generations: tuple[Generation, Generation] | None = None, base: str | None = None, head: str | None = None) -> str:
    url = f"{SERVER_URL}/{REPOSITORY}/actions/runs/{WRITER_RUN_ID}"
    if source_run_id is None and generations is None and base is None and head is None:
        return url
    parts = urlparse(url)
    query: dict[str, str] = {}
    if source_run_id is not None:
        query["source_run_id"] = str(source_run_id)
    if generations is not None:
        for prefix, item in zip(("ci", "release"), generations, strict=True):
            query[f"{prefix}_workflow_id"] = str(item.workflow_id)
            query[f"{prefix}_run_id"] = str(item.identifier)
            query[f"{prefix}_run_number"] = str(item.number)
            query[f"{prefix}_run_attempt"] = str(item.attempt)
            query[f"{prefix}_status"] = item.status
            query[f"{prefix}_conclusion"] = item.conclusion if isinstance(item.conclusion, str) else ""
    if base is not None:
        query["pr_base_sha"] = base
    if head is not None:
        query["pr_head_sha"] = head
    return urlunparse(parts._replace(query=urlencode(query)))


def process(number: int, claimants: dict[str, frozenset[int]], snapshot_path: str, evidence: EvidenceSnapshot | None = None, *, defer_terminal: bool = False) -> PendingDecision | None:
    initial = pull(number)
    head, base, branch, draft = initial["head"]["sha"], initial["base"]["sha"], initial["head"].get("ref"), initial["draft"]
    if not isinstance(branch, str):
        raise GovernanceError("Pull request branch is invalid.")
    # The dispatcher is the only event-path invalidator.  Main always defers
    # terminal publication, so it reuses that pending status (or a scheduled
    # baseline) rather than creating a second pending status for every PR.
    pending = check_baseline(head) if defer_terminal else write_governance_check(head, "pending", "Trusted governance revalidation is running.", target_url())
    try:
        issue = canonical_issue(initial.get("body"))
        result = contract(number, base, head, branch, draft, snapshot_path)
        # A Draft is deliberately non-terminal. It must not require a final
        # review sensor or release-preflight run merely to stay pending.
        if draft:
            if defer_terminal:
                # The dispatcher already invalidated this head.  Do not add
                # an unbudgeted pending mutation while an all-open writer is
                # deliberately conserving its Check Run write allowance.
                return None
            if not check_changed_since(head, pending):
                write_governance_check(head, "pending", "Draft PR governance remains pending.", target_url())
            return
        # verify_push_issue rejects every workflow-file change in the PR
        # range.  Once that contract fails, no untrusted workflow evidence is
        # needed to publish a failure; once it succeeds, the shared
        # default-branch run index is the trust chain and avoids O(N) blob
        # reads for the same immutable workflow paths.
        if result != "success":
            if defer_terminal:
                return PendingDecision(number, head, base, pending, "failure", "Trusted PR governance failed.", None, None, issue)
            if not check_changed_since(head, pending):
                write_governance_check(
                    head, "failure", "Trusted PR governance failed.", target_url(),
                    expected_fingerprint=pending,
                )
            return
        sensor_id = sensor(number, base, head, evidence)
        current_generations = (
            generation(number, base, head, "CI", ".github/workflows/test-and-build.yml", evidence),
            generation(number, base, head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
        )
        ci = "failure" if "failure" in {verdict(item) for item in current_generations} else "pending" if "pending" in {verdict(item) for item in current_generations} else "success"
        if draft or result == "pending" or ci == "pending":
            state, description = "pending", "Trusted governance revalidation is pending."
        elif result != "success" or ci != "success" or issue is None:
            state, description = "failure", "Trusted PR governance failed."
        elif not final_closer_is_unique(number, issue, base, head, claimants):
            state, description = "failure", "Canonical Issue closer set changed."
        else:
            # A second read immediately before success rejects same-head reruns and attempts.
            latest = (
                generation(number, base, head, "CI", ".github/workflows/test-and-build.yml", evidence),
                generation(number, base, head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
            )
            if latest != current_generations:
                state, description = "pending", "CI generation changed during governance revalidation."
            elif not defer_terminal and check_changed_since(head, pending):
                return
            else:
                state, description = "success", "Trusted PR governance passed."
        if defer_terminal:
            return PendingDecision(number, head, base, pending, state, description, sensor_id, current_generations, issue)
        terminal_count = 0
        if state == "success":
            terminal_count = sensor_terminal_check_count(head, sensor_id)
            if terminal_count >= 2:
                raise NoPostGovernanceError("Review latch already has multiple terminal statuses.")
            # Preserve the one existing sensor-bound terminal status.  A new
            # success intentionally omits source_run_id so the latch remains
            # unambiguous.
        if check_changed_since(head, pending):
            return
        write_governance_check(head, state, description, target_url(
            source_run_id=sensor_id if state == "success" and terminal_count == 0 else None,
            generations=current_generations if state == "success" else None,
            base=base if state == "success" else None,
            head=head if state == "success" else None,
        ), expected_fingerprint=pending)
    except NoPostGovernanceError:
        raise
    except GovernanceError:
        if defer_terminal:
            # Main owns the terminal-write reservation.  Publishing a
            # fail-closed result here would let malformed tail PRs bypass the
            # 100/400 write budget and reintroduce a rate-limit burst.
            return PendingDecision(
                number, head, base, pending, "failure",
                "Trusted PR governance failed closed.", None, None, None,
            )
        if not check_changed_since(head, pending):
            write_governance_check(
                head, "failure", "Trusted PR governance failed closed.", target_url(),
                expected_fingerprint=pending,
            )
        raise


def finalize_decision(decision: PendingDecision, claimants: dict[str, frozenset[int]], evidence: EvidenceSnapshot) -> bool:
    """Write one terminal state after a bounded final head-specific refresh."""
    state, description = decision.state, decision.description
    generations = decision.generations
    terminal_count = 0
    try:
        if state == "success":
            if decision.sensor_id is None or generations is None or decision.issue is None:
                raise GovernanceError("Successful governance decision is incomplete.")
            if not final_closer_is_unique(decision.number, decision.issue, decision.base, decision.head, claimants):
                state, description = "failure", "Canonical Issue closer set changed."
            else:
                if sensor(decision.number, decision.base, decision.head, evidence) != decision.sensor_id:
                    state, description = "pending", "Review sensor changed during governance revalidation."
                else:
                    latest = (
                        generation(decision.number, decision.base, decision.head, "CI", ".github/workflows/test-and-build.yml", evidence),
                        generation(decision.number, decision.base, decision.head, "release-preflight", ".github/workflows/release-preflight.yml", evidence),
                    )
                    if latest != generations:
                        state, description = "pending", "CI generation changed during governance revalidation."
        desired = target_url(
            source_run_id=decision.sensor_id if state == "success" else None,
            generations=generations if state == "success" else None,
            base=decision.base if state == "success" else None,
            head=decision.head if state == "success" else None,
        )
        if decision.sensor_id is not None:
            newer, observed_terminal_count, exact_current = check_fence(
                decision.head, decision.pending_check_fingerprint, decision.sensor_id,
                desired_state=state, desired_target=desired,
            )
            if state == "success":
                terminal_count = observed_terminal_count
                if terminal_count >= 2:
                    raise NoPostGovernanceError("Review latch already has multiple terminal statuses.")
        else:
            newer = check_changed_since(decision.head, decision.pending_check_fingerprint)
            exact_current = False
        if newer:
            return False
        # A scheduled all-open pass must not manufacture a fresh status when
        # the currently trusted App status already has identical immutable
        # CI/review/base/head evidence.
        if exact_current:
            return False
        # This is deliberately immediately before the PATCH.  A queued writer
        # must never create a Check Run after default-branch governance bytes
        # changed, even though bootstrap-validation succeeded earlier.
        rebind_trusted_default_writer()
        write_governance_check(decision.head, state, description, target_url(
            source_run_id=decision.sensor_id if state == "success" and terminal_count == 0 else None,
            generations=generations if state == "success" else None,
            base=decision.base if state == "success" else None,
            head=decision.head if state == "success" else None,
        ), expected_fingerprint=decision.pending_check_fingerprint)
        return True
    except NoPostGovernanceError:
        raise
    except GovernanceError:
        rebind_trusted_default_writer()
        if not check_changed_since(decision.head, decision.pending_check_fingerprint):
            write_governance_check(
                decision.head, "failure", "Trusted PR governance failed closed.", target_url(),
                expected_fingerprint=decision.pending_check_fingerprint,
            )
        raise


def decision_write_cost(decision: PendingDecision) -> int:
    """Reserve every Check Run mutation before attempting a terminal decision."""
    if decision.pending_check_fingerprint:
        return 1
    return 1 if decision.state == "pending" else 2


def governance_order(snapshot: OpenSnapshot, carry: frozenset[int]) -> tuple[int, ...]:
    """Prioritize terminal carry, then fresh terminal work, then Drafts."""
    numbers = snapshot.numbers
    if not carry.issubset(numbers):
        raise GovernanceError("Dispatcher carry target is outside the open snapshot.")
    drafts: dict[int, bool] = {}
    for pull_request in snapshot.pull_requests:
        number = pull_request.get("number")
        draft = pull_request.get("isDraft")
        if type(number) is not int or number not in numbers or type(draft) is not bool or number in drafts:
            raise GovernanceError("Open pull request draft state is invalid.")
        drafts[number] = draft
    if set(drafts) != set(numbers):
        raise GovernanceError("Open pull request draft snapshot is incomplete.")
    if any(drafts[number] for number in carry):
        raise GovernanceError("Draft pull request cannot carry a terminal governance decision.")
    return (
        tuple(number for number in numbers if number in carry)
        + tuple(number for number in numbers if not drafts[number] and number not in carry)
        + tuple(number for number in numbers if drafts[number])
    )


def main() -> int:
    if not REPOSITORY or not SERVER_URL or not NUMBER.fullmatch(WRITER_RUN_ID):
        print("Writer runtime identity is invalid.", file=sys.stderr)
        return 1
    dispatcher_run_id = os.environ.get("GOVERNANCE_DISPATCHER_RUN_ID", "")
    if not NUMBER.fullmatch(dispatcher_run_id):
        print("Writer dispatcher run ID is invalid.", file=sys.stderr)
        return 1
    try:
        dispatcher_source = trusted_dispatcher_source(int(dispatcher_run_id))
        snapshot = open_snapshot()
        carry = observed_invalidations(snapshot, dispatcher_source)
        initial_evidence = evidence_snapshot()
    except GovernanceError as error:
        print(str(error), file=sys.stderr)
        return 1
    failures = 0
    # Do not make one malformed/changed PR leave other open PRs stale.
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", suffix=".json") as source:
        json.dump(list(snapshot.pull_requests), source)
        source.flush()
        # Complete each PR before starting the next one.  Retaining every
        # decision and finalizing only after all contracts ran created an
        # avoidable window for Issue/review/CI state to become stale.
        # The all-open invalidator may span multiple hours, so this writer
        # reserves only its own terminal mutations: schedule=400, event=100.
        # The repository-wide writer queue plus 8.1-second pacing keeps its
        # rolling Check Run mutation maximum at 445/hour. Creating a terminal
        # Check Run costs pending POST plus terminal PATCH.
        terminal_write_budget = 400 if dispatcher_source.event == "schedule" else 100
        for number in governance_order(snapshot, carry):
            decision: PendingDecision | None = None
            try:
                decision = process(number, snapshot.claimants, source.name, initial_evidence, defer_terminal=True)
                if decision is not None:
                    cost = decision_write_cost(decision)
                    if cost > terminal_write_budget:
                        continue
                    terminal_write_budget -= cost
                    finalize_decision(decision, snapshot.claimants, final_evidence_for_pr(decision.head, initial_evidence))
            except GovernanceError as error:
                failures += 1
                print(f"PR #{number}: {error}", file=sys.stderr)
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
