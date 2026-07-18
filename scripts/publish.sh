#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${CARGO_REGISTRY_TOKEN:-}" ]]; then
  echo "CARGO_REGISTRY_TOKEN is required; configure the crates-io environment" >&2
  exit 1
fi

version="$(cargo metadata --format-version 1 --no-deps | python3 -c '
import json
import sys

metadata = json.load(sys.stdin)
print(next(package["version"] for package in metadata["packages"] if package["name"] == "tokencompact-core"))
')"
release_ref="${GITHUB_REF_NAME:-}"
if [[ -z "${release_ref}" ]]; then
  release_ref="$(git tag --points-at HEAD | sed -n '1p')"
fi
if [[ "${release_ref}" != "v${version}" ]]; then
  echo "publish must run from tag v${version}; current ref is ${release_ref:-untagged}" >&2
  exit 1
fi

wait_for_crate() {
  local crate="$1"
  for _ in {1..30}; do
    if cargo info "${crate}@${version}" >/dev/null 2>&1; then
      return 0
    fi
    sleep 10
  done
  echo "timed out waiting for ${crate}@${version} to reach the registry index" >&2
  return 1
}

cargo publish --locked -p tokencompact-core
wait_for_crate tokencompact-core
cargo publish --locked -p tokencompact-builtins
wait_for_crate tokencompact-builtins
cargo publish --locked -p tokencompact
