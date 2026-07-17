#!/usr/bin/env python3
"""Create and verify a KRR helper plus Chromium release archive."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
import posixpath
import re
import shutil
import stat
import sys
import tarfile
import tempfile
import zipfile
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path, PurePosixPath

import install as chromium_install


VERSION_PATTERN = re.compile(r"^v(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)$")
RUNTIME_NAME = "krr-html-browser-runtime"


@dataclass(frozen=True)
class PackageContract:
    root_name: str
    platform: str
    helper_name: str
    chromium_executable: str
    payload: dict[str, object]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform")
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--helper-bin", required=True, type=Path)
    parser.add_argument("--krr-license", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    return parser.parse_args()


def package_runtime(
    version: str,
    platform_key: str,
    manifest_path: Path,
    helper_bin: Path,
    license_path: Path,
    output_dir: Path,
) -> tuple[Path, Path]:
    validate_version(version)
    artifact = chromium_install.load_artifact(manifest_path, platform_key)
    manifest = load_manifest_metadata(manifest_path)
    expected_helper = helper_filename(platform_key)
    if helper_bin.name != expected_helper or not helper_bin.is_file():
        raise RuntimeError(f"release helper is missing: {helper_bin}")
    if platform_key != "win64" and not os.access(helper_bin, os.X_OK):
        raise RuntimeError(f"release helper is not executable: {helper_bin}")
    if not license_path.is_file():
        raise RuntimeError(f"KRR license is missing: {license_path}")

    chromium_root = helper_bin.parent / "chromium"
    chromium_install.check_installed(artifact, chromium_root)
    platform_root = chromium_root / platform_key
    about = platform_root / PurePosixPath(artifact.executable).parts[0] / "ABOUT"
    if not about.is_file():
        raise RuntimeError(f"Chromium ABOUT file is missing: {about}")

    root_name = f"{RUNTIME_NAME}-{version}-{platform_key}"
    payload = runtime_manifest_payload(
        version,
        platform_key,
        expected_helper,
        artifact,
        manifest,
        manifest_path,
        helper_bin,
        license_path,
    )
    contract = PackageContract(
        root_name=root_name,
        platform=platform_key,
        helper_name=expected_helper,
        chromium_executable=f"chromium/{platform_key}/{artifact.executable}",
        payload=payload,
    )
    output_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="krr-chromium-package-", dir=output_dir) as temp:
        package_root = Path(temp) / root_name
        stage_runtime(
            package_root, platform_root, manifest_path, helper_bin, license_path, payload
        )
        archive_path = output_dir / f"{root_name}{archive_suffix(platform_key)}"
        temporary_archive = Path(temp) / archive_path.name
        write_archive(temporary_archive, package_root, platform_key)
        verify_archive(temporary_archive, contract)
        os.replace(temporary_archive, archive_path)

    checksum = chromium_install.sha256_file(archive_path)
    checksum_path = archive_path.with_name(f"{archive_path.name}.sha256")
    checksum_path.write_text(f"{checksum}  {archive_path.name}\n", encoding="utf-8")
    print(f"Chromium runtime package verified: {archive_path}")
    print(f"Chromium runtime checksum: {checksum_path}")
    return archive_path, checksum_path


def validate_version(version: str) -> None:
    if not VERSION_PATTERN.fullmatch(version):
        raise RuntimeError(f"invalid runtime release version: {version}")


def load_manifest_metadata(path: Path) -> dict[str, object]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("schema_version") != 1:
        raise RuntimeError("unsupported Chromium manifest schema")
    for field in ["engine", "version", "license"]:
        if not isinstance(payload.get(field), str) or not payload[field]:
            raise RuntimeError(f"Chromium manifest is missing {field}")
    return payload


def helper_filename(platform_key: str) -> str:
    return "krr-html-chromium-engine.exe" if platform_key == "win64" else "krr-html-chromium-engine"


def runtime_manifest_payload(
    version: str,
    platform_key: str,
    helper_name: str,
    artifact: chromium_install.ChromiumArtifact,
    manifest: dict[str, object],
    manifest_path: Path,
    helper_bin: Path,
    license_path: Path,
) -> dict[str, object]:
    return {
        "schema_version": 1,
        "runtime": RUNTIME_NAME,
        "version": version,
        "platform": platform_key,
        "helper": helper_name,
        "helper_sha256": chromium_install.sha256_file(helper_bin),
        "krr_license": "KRR-LICENSE",
        "krr_license_sha256": chromium_install.sha256_file(license_path),
        "chromium": {
            "engine": manifest["engine"],
            "version": manifest["version"],
            "license": manifest["license"],
            "manifest": "chromium/manifest.json",
            "manifest_sha256": chromium_install.sha256_file(manifest_path),
            "source_archive_sha256": artifact.sha256,
            "executable": f"chromium/{platform_key}/{artifact.executable}",
        },
    }


def stage_runtime(
    package_root: Path,
    platform_root: Path,
    manifest_path: Path,
    helper_bin: Path,
    license_path: Path,
    payload: dict[str, object],
) -> None:
    package_root.mkdir()
    shutil.copy2(helper_bin, package_root / helper_bin.name)
    chromium = package_root / "chromium"
    chromium.mkdir()
    shutil.copytree(platform_root, chromium / platform_root.name, symlinks=True)
    shutil.copy2(manifest_path, chromium / "manifest.json")
    shutil.copy2(license_path, package_root / "KRR-LICENSE")
    (package_root / "runtime-manifest.json").write_text(
        json.dumps(payload, ensure_ascii=True, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def archive_suffix(platform_key: str) -> str:
    return ".zip" if platform_key == "win64" else ".tar.gz"


def write_archive(archive_path: Path, package_root: Path, platform_key: str) -> None:
    if platform_key == "win64":
        write_zip_archive(archive_path, package_root)
    else:
        write_tar_archive(archive_path, package_root)


def write_tar_archive(archive_path: Path, package_root: Path) -> None:
    with archive_path.open("wb") as raw:
        with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as bundle:
                bundle.add(
                    package_root,
                    arcname=package_root.name,
                    recursive=True,
                    filter=normalize_tar_info,
                )


def normalize_tar_info(info: tarfile.TarInfo) -> tarfile.TarInfo:
    info.uid = 0
    info.gid = 0
    info.uname = ""
    info.gname = ""
    info.mtime = 0
    info.pax_headers = {}
    return info


def write_zip_archive(archive_path: Path, package_root: Path) -> None:
    paths = [package_root, *sorted(package_root.rglob("*"))]
    with zipfile.ZipFile(archive_path, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
        for path in paths:
            relative = path.relative_to(package_root.parent).as_posix()
            mode = path.lstat().st_mode
            if path.is_dir() and not path.is_symlink():
                relative = f"{relative}/"
                content = b""
            elif path.is_symlink():
                content = os.readlink(path).encode("utf-8")
            else:
                content = path.read_bytes()
            info = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = mode << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            bundle.writestr(info, content)


def verify_archive(archive_path: Path, contract: PackageContract) -> None:
    if archive_path.name.endswith(".tar.gz"):
        verify_tar_archive(archive_path, contract)
    elif archive_path.suffix == ".zip":
        verify_zip_archive(archive_path, contract)
    else:
        raise RuntimeError(f"unsupported runtime archive: {archive_path}")


def verify_tar_archive(archive_path: Path, contract: PackageContract) -> None:
    with tarfile.open(archive_path, "r:gz") as bundle:
        members = {member.name.rstrip("/"): member for member in bundle.getmembers()}
        for member in members.values():
            validate_archive_member(contract.root_name, member.name, member.linkname)
        verify_required_members(set(members), contract)
        verify_payload(read_tar_member(bundle, members, contract, "runtime-manifest.json"), contract)
        verify_marker(read_tar_member(bundle, members, contract, marker_path(contract)), contract)
        verify_packaged_file_digests(
            lambda relative: read_tar_member(bundle, members, contract, relative), contract
        )
        verify_executable_mode(members[member_path(contract, contract.helper_name)], contract)
        verify_executable_mode(members[member_path(contract, contract.chromium_executable)], contract)


def read_tar_member(
    bundle: tarfile.TarFile,
    members: dict[str, tarfile.TarInfo],
    contract: PackageContract,
    relative: str,
) -> bytes:
    member = members[member_path(contract, relative)]
    extracted = bundle.extractfile(member)
    if extracted is None:
        raise RuntimeError(f"runtime archive member is not a file: {member.name}")
    return extracted.read()


def verify_zip_archive(archive_path: Path, contract: PackageContract) -> None:
    with zipfile.ZipFile(archive_path) as bundle:
        infos = {info.filename.rstrip("/"): info for info in bundle.infolist()}
        for info in infos.values():
            validate_archive_member(contract.root_name, info.filename, "")
        verify_required_members(set(infos), contract)
        verify_payload(bundle.read(member_path(contract, "runtime-manifest.json")), contract)
        verify_marker(bundle.read(member_path(contract, marker_path(contract))), contract)
        verify_packaged_file_digests(
            lambda relative: bundle.read(member_path(contract, relative)), contract
        )


def validate_archive_member(root_name: str, name: str, link_name: str) -> None:
    path = PurePosixPath(name)
    if path.is_absolute() or ".." in path.parts or not path.parts or path.parts[0] != root_name:
        raise RuntimeError(f"unsafe runtime archive member: {name}")
    if link_name:
        target = posixpath.normpath(posixpath.join(posixpath.dirname(name), link_name))
        target_path = PurePosixPath(target)
        if target_path.is_absolute() or not target_path.parts or target_path.parts[0] != root_name:
            raise RuntimeError(f"unsafe runtime archive link: {name} -> {link_name}")


def verify_required_members(members: set[str], contract: PackageContract) -> None:
    chromium_top = PurePosixPath(contract.chromium_executable).parts[2]
    required = [
        contract.helper_name,
        contract.chromium_executable,
        marker_path(contract),
        f"chromium/{contract.platform}/{chromium_top}/ABOUT",
        "chromium/manifest.json",
        "KRR-LICENSE",
        "runtime-manifest.json",
    ]
    missing = [relative for relative in required if member_path(contract, relative) not in members]
    if missing:
        raise RuntimeError(f"runtime archive is missing required members: {', '.join(missing)}")


def member_path(contract: PackageContract, relative: str) -> str:
    return f"{contract.root_name}/{relative}"


def marker_path(contract: PackageContract) -> str:
    return f"chromium/{contract.platform}/.krr-chromium-sha256"


def verify_payload(content: bytes, contract: PackageContract) -> None:
    payload = json.loads(content.decode("utf-8"))
    if payload != contract.payload:
        raise RuntimeError("runtime archive manifest does not match the package contract")


def verify_marker(content: bytes, contract: PackageContract) -> None:
    chromium = contract.payload["chromium"]
    if not isinstance(chromium, dict):
        raise RuntimeError("invalid runtime Chromium manifest payload")
    if content.decode("utf-8").strip() != chromium["source_archive_sha256"]:
        raise RuntimeError("runtime archive Chromium checksum marker does not match")


def verify_packaged_file_digests(
    read_member: Callable[[str], bytes],
    contract: PackageContract,
) -> None:
    chromium = contract.payload["chromium"]
    if not isinstance(chromium, dict):
        raise RuntimeError("invalid runtime Chromium manifest payload")
    expected = [
        (contract.helper_name, contract.payload["helper_sha256"]),
        ("KRR-LICENSE", contract.payload["krr_license_sha256"]),
        ("chromium/manifest.json", chromium["manifest_sha256"]),
    ]
    for relative, digest in expected:
        if sha256_bytes(read_member(relative)) != digest:
            raise RuntimeError(f"runtime archive digest does not match: {relative}")


def sha256_bytes(content: bytes) -> str:
    return hashlib.sha256(content).hexdigest()


def verify_executable_mode(member: tarfile.TarInfo, contract: PackageContract) -> None:
    if contract.platform != "win64" and member.mode & stat.S_IXUSR == 0:
        raise RuntimeError(f"runtime executable mode is missing: {member.name}")


def main() -> int:
    try:
        args = parse_args()
        platform_key = args.platform or chromium_install.current_platform_key()
        chromium_install.require_current_platform(platform_key)
        package_runtime(
            args.version,
            platform_key,
            args.manifest,
            args.helper_bin,
            args.krr_license,
            args.output_dir,
        )
        return 0
    except Exception as error:
        print(error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
