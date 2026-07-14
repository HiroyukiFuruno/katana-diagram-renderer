#!/usr/bin/env python3
"""Unit checks for the Chromium install helper."""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

import install as chromium_install


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        install_and_check(root)
        rejects_checksum_mismatch(root)
        rejects_unsafe_archive_member(root)
    print("chromium install helper tests passed")
    return 0


def install_and_check(root: Path) -> None:
    archive = root / "chrome.zip"
    executable = "chrome-test/chrome"
    write_zip(archive, executable)
    manifest = write_manifest(root, archive, executable)
    helper_bin = root / "helper" / "krr-html-chromium-engine"
    helper_bin.parent.mkdir(parents=True)

    run_install(manifest, helper_bin, root / "cache")
    installed = helper_bin.parent / "chromium" / chromium_install.current_platform_key() / executable
    if not installed.is_file():
        raise AssertionError(f"Chromium executable was not installed: {installed}")
    run_check(manifest, helper_bin, root / "cache")
    run_manifest_check(manifest)


def rejects_checksum_mismatch(root: Path) -> None:
    archive = root / "mismatched-chrome.zip"
    executable = "chrome-test/chrome"
    write_zip(archive, executable)
    manifest = write_manifest(root, archive, executable, sha256="0" * 64)
    helper_bin = root / "checksum-helper" / "krr-html-chromium-engine"
    helper_bin.parent.mkdir(parents=True)

    error = run_helper_failure(
        "--manifest",
        str(manifest),
        "--helper-bin",
        str(helper_bin),
        "--cache-dir",
        str(root / "checksum-cache"),
    )

    if "checksum mismatch" not in error:
        raise AssertionError(f"checksum mismatch was not reported: {error}")
    if (helper_bin.parent / "chromium").exists():
        raise AssertionError("checksum mismatch created a Chromium install")


def rejects_unsafe_archive_member(root: Path) -> None:
    archive = root / "unsafe-chrome.zip"
    executable = "chrome-test/chrome"
    write_unsafe_zip(archive, executable)
    manifest = write_manifest(root, archive, executable)
    helper_bin = root / "unsafe-helper" / "krr-html-chromium-engine"
    helper_bin.parent.mkdir(parents=True)

    error = run_helper_failure(
        "--manifest",
        str(manifest),
        "--helper-bin",
        str(helper_bin),
        "--cache-dir",
        str(root / "unsafe-cache"),
    )

    if "unsafe Chromium archive member" not in error:
        raise AssertionError(f"unsafe archive member was not rejected: {error}")
    if (root / "escaped").exists():
        raise AssertionError("unsafe archive member escaped the install root")


def write_zip(path: Path, executable: str) -> None:
    info = zipfile.ZipInfo(executable)
    info.external_attr = 0o755 << 16
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(info, b"test chrome")


def write_unsafe_zip(path: Path, executable: str) -> None:
    with zipfile.ZipFile(path, "w") as archive:
        archive.writestr(executable, b"test chrome")
        archive.writestr("../escaped", b"unsafe")


def write_manifest(root: Path, archive: Path, executable: str, sha256: str | None = None) -> Path:
    manifest = root / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "engine": "chrome-for-testing",
                "version": "test",
                "license": "BSD-3-Clause",
                "artifacts": [
                    {
                        "platform": chromium_install.current_platform_key(),
                        "url": archive.resolve().as_uri(),
                        "sha256": sha256 or sha256_file(archive),
                        "executable": executable,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    return manifest


def run_install(manifest: Path, helper_bin: Path, cache_dir: Path) -> None:
    run_helper(
        "--manifest",
        str(manifest),
        "--helper-bin",
        str(helper_bin),
        "--cache-dir",
        str(cache_dir),
    )


def run_check(manifest: Path, helper_bin: Path, cache_dir: Path) -> None:
    run_helper(
        "--manifest",
        str(manifest),
        "--helper-bin",
        str(helper_bin),
        "--cache-dir",
        str(cache_dir),
        "--check-only",
    )


def run_manifest_check(manifest: Path) -> None:
    run_helper("--manifest", str(manifest), "--manifest-only")


def run_helper(*args: str) -> None:
    command = [sys.executable, str(Path(__file__).with_name("install.py")), *args]
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise AssertionError(
            f"Chromium helper failed with {result.returncode}\nSTDOUT:\n{result.stdout}\nSTDERR:\n{result.stderr}"
        )


def run_helper_failure(*args: str) -> str:
    command = [sys.executable, str(Path(__file__).with_name("install.py")), *args]
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode == 0:
        raise AssertionError(f"Chromium helper unexpectedly succeeded\nSTDOUT:\n{result.stdout}")
    return result.stderr


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    digest.update(path.read_bytes())
    return digest.hexdigest()


if __name__ == "__main__":
    raise SystemExit(main())
