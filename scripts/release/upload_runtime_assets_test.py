#!/usr/bin/env python3
"""Contract tests for immutable Chromium runtime asset publication."""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import tempfile
from pathlib import Path


TAG = "v0.4.0"
PLATFORMS = ["linux64", "mac-arm64", "mac-x64", "win64"]


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        script = Path(__file__).with_name("upload-runtime-assets.sh")
        mock_bin = write_gh_mock(root)
        rejects_missing_assets(root, script, mock_bin)
        rejects_invalid_local_checksum(root, script, mock_bin)
        uploads_new_assets(root, script, mock_bin)
        accepts_identical_published_assets(root, script, mock_bin)
        rejects_changed_published_assets(root, script, mock_bin)
    print("runtime asset upload tests passed")
    return 0


def rejects_missing_assets(root: Path, script: Path, mock_bin: Path) -> None:
    assets = root / "missing-assets"
    assets.mkdir()
    result = run_uploader(script, assets, root / "missing-remote", mock_bin, root, "missing")
    if result.returncode == 0 or "runtime release asset is missing" not in result.stderr:
        raise AssertionError(f"missing assets were not rejected: {result.stderr}")


def rejects_invalid_local_checksum(root: Path, script: Path, mock_bin: Path) -> None:
    assets = write_assets(root / "bad-checksum-assets")
    archive = next(path for path in assets.iterdir() if not path.name.endswith(".sha256"))
    archive.write_bytes(b"modified after checksum")
    result = run_uploader(
        script, assets, root / "bad-checksum-remote", mock_bin, root, "bad-checksum"
    )
    if result.returncode == 0 or "FAILED" not in result.stdout:
        raise AssertionError(f"bad local checksum was not rejected: {result.stdout}")
    if upload_log(root, "bad-checksum"):
        raise AssertionError("bad local checksum reached the upload command")


def uploads_new_assets(root: Path, script: Path, mock_bin: Path) -> None:
    assets = write_assets(root / "new-assets")
    remote = root / "new-remote"
    remote.mkdir()
    result = run_uploader(script, assets, remote, mock_bin, root, "new")
    require_success(result)
    uploaded = set(upload_log(root, "new"))
    if uploaded != {path.name for path in assets.iterdir()}:
        raise AssertionError(f"not every runtime asset was uploaded: {sorted(uploaded)}")


def accepts_identical_published_assets(root: Path, script: Path, mock_bin: Path) -> None:
    assets = write_assets(root / "same-assets")
    remote = root / "same-remote"
    shutil.copytree(assets, remote)
    result = run_uploader(script, assets, remote, mock_bin, root, "same")
    require_success(result)
    if upload_log(root, "same"):
        raise AssertionError("identical published runtime assets were uploaded again")


def rejects_changed_published_assets(root: Path, script: Path, mock_bin: Path) -> None:
    assets = write_assets(root / "changed-assets")
    remote = root / "changed-remote"
    shutil.copytree(assets, remote)
    archive = next(path for path in remote.iterdir() if not path.name.endswith(".sha256"))
    archive.write_bytes(b"different published bytes")
    result = run_uploader(script, assets, remote, mock_bin, root, "changed")
    if result.returncode == 0 or "published runtime asset differs" not in result.stderr:
        raise AssertionError(f"changed published asset was not rejected: {result.stderr}")


def write_assets(directory: Path) -> Path:
    directory.mkdir()
    for platform in PLATFORMS:
        extension = "zip" if platform == "win64" else "tar.gz"
        archive = directory / f"krr-html-browser-runtime-{TAG}-{platform}.{extension}"
        archive.write_bytes(f"runtime-{platform}".encode("utf-8"))
        checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
        (directory / f"{archive.name}.sha256").write_text(
            f"{checksum}  {archive.name}\n", encoding="utf-8"
        )
    return directory


def write_gh_mock(root: Path) -> Path:
    mock_bin = root / "mock-bin"
    mock_bin.mkdir()
    gh = mock_bin / "gh"
    gh.write_text(
        """#!/usr/bin/env bash
set -euo pipefail
if [[ "$1 $2" == "release view" ]]; then
  if [[ -d "${MOCK_REMOTE_DIR}" ]]; then
    for path in "${MOCK_REMOTE_DIR}"/*; do basename "${path}"; done
  fi
elif [[ "$1 $2" == "release upload" ]]; then
  basename "$4" >> "${MOCK_UPLOAD_LOG}"
elif [[ "$1 $2" == "release download" ]]; then
  pattern="$5"
  directory="$7"
  mkdir -p "${directory}"
  cp "${MOCK_REMOTE_DIR}/${pattern}" "${directory}/${pattern}"
else
  echo "unexpected gh command: $*" >&2
  exit 1
fi
""",
        encoding="utf-8",
    )
    gh.chmod(0o755)
    return mock_bin


def run_uploader(
    script: Path,
    assets: Path,
    remote: Path,
    mock_bin: Path,
    root: Path,
    run_name: str,
) -> subprocess.CompletedProcess[str]:
    runner_temp = root / f"runner-{run_name}"
    runner_temp.mkdir()
    upload_path = root / f"uploads-{run_name}.txt"
    env = os.environ.copy()
    env.update(
        {
            "PATH": f"{mock_bin}{os.pathsep}{env['PATH']}",
            "MOCK_REMOTE_DIR": str(remote),
            "MOCK_UPLOAD_LOG": str(upload_path),
            "RUNNER_TEMP": str(runner_temp),
            "GITHUB_RUN_ID": run_name,
            "GITHUB_RUN_ATTEMPT": "1",
        }
    )
    return subprocess.run(
        ["bash", str(script), TAG, str(assets)],
        env=env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def upload_log(root: Path, run_name: str) -> list[str]:
    path = root / f"uploads-{run_name}.txt"
    return path.read_text(encoding="utf-8").splitlines() if path.exists() else []


def require_success(result: subprocess.CompletedProcess[str]) -> None:
    if result.returncode != 0:
        raise AssertionError(
            f"runtime asset uploader failed with {result.returncode}\n"
            f"STDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
        )


if __name__ == "__main__":
    raise SystemExit(main())
