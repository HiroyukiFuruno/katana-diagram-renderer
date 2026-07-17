#!/usr/bin/env python3
"""Install the pinned Chromium bundle next to the KRR HTML browser helper."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import stat
import sys
import urllib.request
import zipfile
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlparse


@dataclass(frozen=True)
class ChromiumArtifact:
    platform: str
    url: str
    sha256: str
    executable: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--helper-bin", type=Path)
    parser.add_argument("--install-root", type=Path)
    parser.add_argument("--cache-dir", default=Path("tmp/chromium-cache"), type=Path)
    parser.add_argument("--platform")
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--manifest-only", action="store_true")
    parser.add_argument("--fresh", action="store_true")
    return parser.parse_args()


def current_platform_key() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()
    if system == "darwin" and machine in {"arm64", "aarch64"}:
        return "mac-arm64"
    if system == "darwin" and machine in {"x86_64", "amd64"}:
        return "mac-x64"
    if system == "linux" and machine in {"x86_64", "amd64"}:
        return "linux64"
    if system == "windows" and machine in {"x86_64", "amd64"}:
        return "win64"
    raise RuntimeError(f"unsupported Chromium platform: {system}/{machine}")


def load_artifact(manifest_path: Path, platform_key: str | None = None) -> ChromiumArtifact:
    payload = json.loads(manifest_path.read_text(encoding="utf-8"))
    selected_platform = platform_key or current_platform_key()
    for artifact in payload.get("artifacts", []):
        if artifact.get("platform") == selected_platform:
            return validate_artifact(artifact)
    raise RuntimeError(f"Chromium manifest has no artifact for {selected_platform}")


def require_current_platform(platform_key: str) -> None:
    current = current_platform_key()
    if platform_key != current:
        raise RuntimeError(
            f"Chromium platform {platform_key} does not match the runner platform {current}"
        )


def validate_artifact(payload: dict[str, object]) -> ChromiumArtifact:
    artifact = ChromiumArtifact(
        platform=required_string(payload, "platform"),
        url=required_string(payload, "url"),
        sha256=required_string(payload, "sha256"),
        executable=required_string(payload, "executable"),
    )
    if len(artifact.sha256) != 64 or any(it not in "0123456789abcdef" for it in artifact.sha256):
        raise RuntimeError(f"invalid Chromium sha256 for {artifact.platform}")
    if urlparse(artifact.url).scheme not in {"https", "file"}:
        raise RuntimeError(f"unsupported Chromium artifact URL: {artifact.url}")
    if Path(artifact.executable).is_absolute() or ".." in Path(artifact.executable).parts:
        raise RuntimeError(f"unsafe Chromium executable path: {artifact.executable}")
    return artifact


def required_string(payload: dict[str, object], key: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise RuntimeError(f"Chromium artifact is missing {key}")
    return value


def install_root(args: argparse.Namespace) -> Path:
    if args.install_root is not None:
        return args.install_root
    if args.helper_bin is None:
        raise RuntimeError("--helper-bin or --install-root is required")
    return args.helper_bin.parent / "chromium"


def install_chromium(
    artifact: ChromiumArtifact,
    root: Path,
    cache_dir: Path,
    *,
    fresh: bool = False,
) -> Path:
    platform_dir = root / artifact.platform
    executable = platform_dir / artifact.executable
    marker = platform_dir / ".krr-chromium-sha256"
    if (
        not fresh
        and executable.is_file()
        and marker.exists()
        and marker.read_text(encoding="utf-8").strip() == artifact.sha256
    ):
        print(f"Chromium already installed: {executable}")
        return executable
    archive = download_archive(artifact, cache_dir)
    remove_existing_install(platform_dir)
    extract_archive(archive, platform_dir)
    if not executable.is_file():
        raise RuntimeError(f"Chromium executable was not extracted: {executable}")
    make_executable(executable)
    marker.write_text(f"{artifact.sha256}\n", encoding="utf-8")
    print(f"Chromium installed: {executable}")
    return executable


def remove_existing_install(path: Path) -> None:
    if path.is_symlink() or path.is_file():
        path.unlink()
    elif path.is_dir():
        shutil.rmtree(path)


def download_archive(artifact: ChromiumArtifact, cache_dir: Path) -> Path:
    cache_dir.mkdir(parents=True, exist_ok=True)
    archive = cache_dir / f"{artifact.platform}-{artifact.sha256}.zip"
    if archive.is_file() and sha256_file(archive) == artifact.sha256:
        return archive
    temporary = archive.with_suffix(".zip.tmp")
    with urllib.request.urlopen(artifact.url, timeout=60) as response:
        temporary.write_bytes(response.read())
    actual = sha256_file(temporary)
    if actual != artifact.sha256:
        temporary.unlink(missing_ok=True)
        raise RuntimeError(
            f"Chromium checksum mismatch for {artifact.platform}: expected {artifact.sha256}, got {actual}"
        )
    os.replace(temporary, archive)
    return archive


def extract_archive(archive: Path, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    destination_root = destination.resolve()
    with zipfile.ZipFile(archive) as bundle:
        for member in bundle.infolist():
            target = safe_extract_path(destination_root, member.filename)
            if member.is_dir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            target.parent.mkdir(parents=True, exist_ok=True)
            with bundle.open(member) as source, target.open("wb") as output:
                output.write(source.read())
            apply_zip_mode(target, member)


def safe_extract_path(destination_root: Path, member_name: str) -> Path:
    target = (destination_root / member_name).resolve()
    if os.path.commonpath([destination_root, target]) != str(destination_root):
        raise RuntimeError(f"unsafe Chromium archive member: {member_name}")
    return target


def apply_zip_mode(target: Path, member: zipfile.ZipInfo) -> None:
    mode = member.external_attr >> 16
    if mode:
        target.chmod(mode)


def make_executable(path: Path) -> None:
    if os.name == "nt":
        return
    mode = path.stat().st_mode
    path.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def check_installed(artifact: ChromiumArtifact, root: Path) -> Path:
    executable = root / artifact.platform / artifact.executable
    marker = root / artifact.platform / ".krr-chromium-sha256"
    if not executable.is_file():
        raise RuntimeError(f"Chromium executable is not installed: {executable}")
    if not marker.exists() or marker.read_text(encoding="utf-8").strip() != artifact.sha256:
        raise RuntimeError(f"Chromium checksum marker is not installed: {marker}")
    print(f"Chromium install check passed: {executable}")
    return executable


def main() -> int:
    try:
        args = parse_args()
        platform_key = args.platform or current_platform_key()
        require_current_platform(platform_key)
        artifact = load_artifact(args.manifest, platform_key)
        print(f"Chromium manifest check passed: {artifact.platform} {artifact.sha256}")
        if args.manifest_only:
            return 0
        root = install_root(args)
        if args.check_only:
            check_installed(artifact, root)
        else:
            install_chromium(artifact, root, args.cache_dir, fresh=args.fresh)
        return 0
    except Exception as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
