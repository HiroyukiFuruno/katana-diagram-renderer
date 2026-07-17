#!/usr/bin/env bash
set -euo pipefail

tag="${1:?release tag is required}"
assets_dir="${2:?runtime asset directory is required}"

platforms=(linux64 mac-arm64 mac-x64 win64)
expected=()
for platform in "${platforms[@]}"; do
  extension="tar.gz"
  if [[ "${platform}" == "win64" ]]; then
    extension="zip"
  fi
  archive="krr-html-browser-runtime-${tag}-${platform}.${extension}"
  expected+=("${archive}" "${archive}.sha256")
done

for name in "${expected[@]}"; do
  if [[ ! -f "${assets_dir}/${name}" ]]; then
    echo "runtime release asset is missing: ${name}" >&2
    exit 1
  fi
done

shopt -s nullglob
actual=("${assets_dir}"/*)
if [[ "${#actual[@]}" -ne "${#expected[@]}" ]]; then
  echo "runtime release asset directory must contain exactly ${#expected[@]} files" >&2
  exit 1
fi
for path in "${actual[@]}"; do
  name="$(basename "${path}")"
  found=false
  for allowed in "${expected[@]}"; do
    if [[ "${name}" == "${allowed}" ]]; then
      found=true
      break
    fi
  done
  if [[ "${found}" != "true" ]]; then
    echo "unexpected runtime release asset: ${name}" >&2
    exit 1
  fi
done

(
  cd "${assets_dir}"
  sha256sum --check ./*.sha256
)

remote_assets="$(gh release view "${tag}" --json assets --jq '.assets[].name')"
download_dir="${RUNNER_TEMP:-tmp}/krr-runtime-assets-${GITHUB_RUN_ID:-local}-${GITHUB_RUN_ATTEMPT:-0}"
mkdir -p "${download_dir}"

for path in "${actual[@]}"; do
  name="$(basename "${path}")"
  if grep -Fqx "${name}" <<<"${remote_assets}"; then
    gh release download "${tag}" --pattern "${name}" --dir "${download_dir}"
    if ! cmp -s "${path}" "${download_dir}/${name}"; then
      echo "published runtime asset differs from the verified local asset: ${name}" >&2
      exit 1
    fi
    echo "Runtime release asset already matches: ${name}"
  else
    gh release upload "${tag}" "${path}"
  fi
done
