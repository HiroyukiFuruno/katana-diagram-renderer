#!/usr/bin/env python3
"""Unit checks for the Chromium runtime release packager."""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

import install as chromium_install
import package_runtime as chromium_package


PLATFORMS = {
    "linux64": "chrome-linux64/chrome",
    "mac-arm64": (
        "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
    ),
    "mac-x64": (
        "chrome-mac-x64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
    ),
    "win64": "chrome-win64/chrome.exe",
}


@dataclass(frozen=True)
class Fixture:
    manifest: Path
    helper: Path
    license: Path
    marker: Path


def main() -> int:
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        packages_every_release_platform(root)
        produces_reproducible_archives(root)
        rejects_tampered_install_marker(root)
        rejects_packaged_file_digest_mismatch()
        rejects_invalid_version(root)
        rejects_cross_platform_runner(root)
    print("chromium runtime package tests passed")
    return 0


def packages_every_release_platform(root: Path) -> None:
    for platform_key, executable in PLATFORMS.items():
        fixture = write_fixture(root / f"package-{platform_key}", platform_key, executable)
        archive, checksum = chromium_package.package_runtime(
            "v0.4.0",
            platform_key,
            fixture.manifest,
            fixture.helper,
            fixture.license,
            root / f"output-{platform_key}",
        )
        expected_suffix = ".zip" if platform_key == "win64" else ".tar.gz"
        if not archive.name.endswith(expected_suffix):
            raise AssertionError(f"wrong archive format for {platform_key}: {archive}")
        expected_checksum = chromium_install.sha256_file(archive)
        if checksum.read_text(encoding="utf-8") != f"{expected_checksum}  {archive.name}\n":
            raise AssertionError(f"invalid checksum file for {platform_key}")


def produces_reproducible_archives(root: Path) -> None:
    platform_key = chromium_install.current_platform_key()
    fixture = write_fixture(root / "reproducible", platform_key, PLATFORMS[platform_key])
    output = root / "reproducible-output"
    first, _ = chromium_package.package_runtime(
        "v0.4.0", platform_key, fixture.manifest, fixture.helper, fixture.license, output
    )
    first_sha = chromium_install.sha256_file(first)
    os.utime(fixture.helper, (2_000_000_000, 2_000_000_000))
    executable = fixture.helper.parent / "chromium" / platform_key / PLATFORMS[platform_key]
    os.utime(executable, (2_000_000_001, 2_000_000_001))
    second, _ = chromium_package.package_runtime(
        "v0.4.0", platform_key, fixture.manifest, fixture.helper, fixture.license, output
    )
    if chromium_install.sha256_file(second) != first_sha:
        raise AssertionError("runtime archive changes when source timestamps change")


def rejects_tampered_install_marker(root: Path) -> None:
    platform_key = chromium_install.current_platform_key()
    fixture = write_fixture(root / "tampered", platform_key, PLATFORMS[platform_key])
    fixture.marker.write_text(f"{'0' * 64}\n", encoding="utf-8")
    try:
        chromium_package.package_runtime(
            "v0.4.0",
            platform_key,
            fixture.manifest,
            fixture.helper,
            fixture.license,
            root / "tampered-output",
        )
    except RuntimeError as error:
        if "checksum marker" not in str(error):
            raise AssertionError(f"wrong marker error: {error}") from error
    else:
        raise AssertionError("tampered Chromium install marker was accepted")


def rejects_packaged_file_digest_mismatch() -> None:
    helper = b"expected helper"
    license_content = b"expected license"
    manifest = b"expected manifest"
    contract = chromium_package.PackageContract(
        root_name="runtime",
        platform="linux64",
        helper_name="krr-html-chromium-engine",
        chromium_executable="chromium/linux64/chrome-linux64/chrome",
        payload={
            "helper_sha256": chromium_package.sha256_bytes(helper),
            "krr_license_sha256": chromium_package.sha256_bytes(license_content),
            "chromium": {
                "manifest_sha256": chromium_package.sha256_bytes(manifest),
                "source_archive_sha256": "0" * 64,
            },
        },
    )
    members = {
        "krr-html-chromium-engine": b"changed helper",
        "KRR-LICENSE": license_content,
        "chromium/manifest.json": manifest,
    }
    try:
        chromium_package.verify_packaged_file_digests(members.__getitem__, contract)
    except RuntimeError as error:
        if "krr-html-chromium-engine" not in str(error):
            raise AssertionError(f"wrong packaged digest error: {error}") from error
    else:
        raise AssertionError("changed packaged helper digest was accepted")


def rejects_invalid_version(root: Path) -> None:
    platform_key = chromium_install.current_platform_key()
    fixture = write_fixture(root / "invalid-version", platform_key, PLATFORMS[platform_key])
    try:
        chromium_package.package_runtime(
            "../../v0.4.0",
            platform_key,
            fixture.manifest,
            fixture.helper,
            fixture.license,
            root / "invalid-version-output",
        )
    except RuntimeError as error:
        if "invalid runtime release version" not in str(error):
            raise AssertionError(f"wrong invalid-version error: {error}") from error
    else:
        raise AssertionError("unsafe runtime release version was accepted")


def rejects_cross_platform_runner(root: Path) -> None:
    current = chromium_install.current_platform_key()
    other = next(platform_key for platform_key in PLATFORMS if platform_key != current)
    command = [
        sys.executable,
        str(Path(__file__).with_name("package_runtime.py")),
        "--version",
        "v0.4.0",
        "--platform",
        other,
        "--manifest",
        str(root / "missing-manifest.json"),
        "--helper-bin",
        str(root / "missing-helper"),
        "--krr-license",
        str(root / "missing-license"),
        "--output-dir",
        str(root / "cross-platform-output"),
    ]
    result = subprocess.run(command, text=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode == 0 or "does not match the runner platform" not in result.stderr:
        raise AssertionError(f"cross-platform runner mismatch was not rejected: {result.stderr}")


def write_fixture(root: Path, platform_key: str, executable: str) -> Fixture:
    helper_dir = root / "helper"
    helper_dir.mkdir(parents=True)
    helper = helper_dir / chromium_package.helper_filename(platform_key)
    helper.write_bytes(b"test helper")
    if platform_key != "win64":
        helper.chmod(0o755)

    platform_root = helper_dir / "chromium" / platform_key
    browser = platform_root / executable
    browser.parent.mkdir(parents=True)
    browser.write_bytes(b"test chromium")
    if platform_key != "win64":
        browser.chmod(0o755)
    about = platform_root / Path(executable).parts[0] / "ABOUT"
    about.write_text("Chromium license metadata\n", encoding="utf-8")

    source_sha = platform_key.encode("utf-8").hex().ljust(64, "0")[:64]
    marker = platform_root / ".krr-chromium-sha256"
    marker.write_text(f"{source_sha}\n", encoding="utf-8")
    manifest = root / "manifest.json"
    manifest.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "engine": "chrome-for-testing",
                "version": "150.0.0.0",
                "license": "BSD-3-Clause",
                "artifacts": [
                    {
                        "platform": platform_key,
                        "url": f"https://example.invalid/{platform_key}.zip",
                        "sha256": source_sha,
                        "executable": executable,
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    license_path = root / "LICENSE"
    license_path.write_text("KRR license\n", encoding="utf-8")
    return Fixture(manifest=manifest, helper=helper, license=license_path, marker=marker)


if __name__ == "__main__":
    raise SystemExit(main())
