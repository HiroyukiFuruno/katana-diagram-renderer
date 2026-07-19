#!/usr/bin/env bash
set -euo pipefail

error() { printf '[ERROR] %s\n' "$*" >&2; }
success() { printf '[OK] %s\n' "$*"; }

version_from_branch() {
  local branch="$1"
  if [[ "${branch}" =~ ^release/v([0-9]+)\.([0-9]+)\.([0-9]+)(-[A-Za-z0-9._-]+)?$ ]]; then
    printf '%s.%s.%s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

current_branch() {
  if [[ -n "${GITHUB_HEAD_REF:-}" ]]; then
    printf '%s\n' "${GITHUB_HEAD_REF}"
    return
  fi
  git branch --show-current
}

parse_version() {
  local version="$1"
  version="${version#v}"
  if [[ "${version}" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    printf '%s %s %s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

change_version_before_target() {
  local change_name="$1"
  local target_major="$2"
  local target_minor="$3"
  local target_patch="$4"

  if [[ ! "${change_name}" =~ ^v([0-9]+)-([0-9]+)-([0-9]+)- ]]; then
    return 1
  fi

  local major="${BASH_REMATCH[1]}"
  local minor="${BASH_REMATCH[2]}"
  local patch="${BASH_REMATCH[3]}"

  if [[ "${major}" -lt "${target_major}" ]]; then
    return 0
  fi
  if [[ "${major}" -gt "${target_major}" ]]; then
    return 1
  fi
  if [[ "${minor}" -lt "${target_minor}" ]]; then
    return 0
  fi
  if [[ "${minor}" -gt "${target_minor}" ]]; then
    return 1
  fi
  [[ "${patch}" -lt "${target_patch}" ]]
}

change_release_version() {
  local change_name="$1"
  if [[ "${change_name}" =~ ^v([0-9]+)-([0-9]+)-([0-9]+)- ]]; then
    printf '%s.%s.%s\n' "${BASH_REMATCH[1]}" "${BASH_REMATCH[2]}" "${BASH_REMATCH[3]}"
    return 0
  fi
  return 1
}

krr_completed_release() {
  local change_dir="$1"
  local metadata="${change_dir}/release-ownership.toml"
  local in_krr_section=0
  local line

  [[ -f "${metadata}" ]] || return 1

  while IFS= read -r line || [[ -n "${line}" ]]; do
    if [[ "${line}" =~ ^[[:space:]]*\[krr\][[:space:]]*$ ]]; then
      in_krr_section=1
      continue
    fi
    if [[ "${line}" =~ ^[[:space:]]*\[.*\][[:space:]]*$ ]]; then
      in_krr_section=0
      continue
    fi
    if [[ "${in_krr_section}" -eq 1 \
      && "${line}" =~ ^[[:space:]]*completed_release[[:space:]]*=[[:space:]]*\"v?([0-9]+\.[0-9]+\.[0-9]+)\"[[:space:]]*$ ]]; then
      printf '%s\n' "${BASH_REMATCH[1]}"
      return 0
    fi
  done < "${metadata}"
  return 1
}

krr_release_slice_is_complete() {
  local change_dir="$1"
  local change_name="$2"
  local completed_release
  local change_release

  if ! completed_release="$(krr_completed_release "${change_dir}")"; then
    return 1
  fi
  if ! change_release="$(change_release_version "${change_name}")"; then
    return 1
  fi
  [[ "${completed_release}" == "${change_release}" ]]
}

target_version="${1:-}"
if [[ -z "${target_version}" ]]; then
  branch="$(current_branch)"
  if ! target_version="$(version_from_branch "${branch}")"; then
    success "release/v* branch ではないため OpenSpec archive 確認をスキップしました。"
    exit 0
  fi
fi

if ! read -r target_major target_minor target_patch < <(parse_version "${target_version}"); then
  error "invalid release version: ${target_version}"
  exit 1
fi

if [[ ! -d openspec/changes ]]; then
  success "OpenSpec change directory が無いため archive 確認をスキップしました。"
  exit 0
fi

remaining_changes=()
for change_dir in openspec/changes/v*-*-*-*; do
  [[ -d "${change_dir}" ]] || continue
  change_name="$(basename "${change_dir}")"
  if change_version_before_target \
    "${change_name}" \
    "${target_major}" \
    "${target_minor}" \
    "${target_patch}"; then
    if krr_release_slice_is_complete "${change_dir}" "${change_name}"; then
      success "${change_name}: KRR v$(krr_completed_release "${change_dir}") slice は公開済みのため active downstream 作業を許可します。"
      continue
    fi
    remaining_changes+=("${change_name}")
  fi
done

if [[ "${#remaining_changes[@]}" -gt 0 ]]; then
  error "v${target_version#v} より前の OpenSpec change が active のまま残っています。"
  error "release/v* の PR 作成前に archive へ移動してください。"
  for change_name in "${remaining_changes[@]}"; do
    error " - ${change_name}"
  done
  exit 1
fi

success "v${target_version#v} より前の OpenSpec change は archive 済みです。"
