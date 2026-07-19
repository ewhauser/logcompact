#!/usr/bin/env bash
set -euo pipefail

readonly DIST_VERSION="0.32.0"
readonly INSTALLER_SHA256="b657cf8c04a8b7bc28f39d220f7e6dd11bbd2bdb072c552262bd9ccf597261b5"
readonly INSTALLER_URL="https://github.com/axodotdev/cargo-dist/releases/download/v${DIST_VERSION}/cargo-dist-installer.sh"

installer="$(mktemp)"
trap 'rm -f "$installer"' EXIT

curl --proto '=https' --tlsv1.2 --location --silent --show-error --fail \
  --output "$installer" "$INSTALLER_URL"
echo "${INSTALLER_SHA256}  ${installer}" | shasum -a 256 --check
sh "$installer"
