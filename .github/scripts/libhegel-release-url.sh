#!/usr/bin/env bash
# Prints the download URL base of the GitHub release for the libhegel version
# pinned in lib/ffi/loader.ml, e.g.
# https://github.com/hegeldev/hegel/releases/download/libhegel/0.47.2
#
# libhegel releases live in the hegeldev/hegel monorepo since 0.47.2, tagged
# libhegel/<version>; earlier releases stay on hegeldev/hegel-rust under
# libhegel-v<version> tags. Keep in sync with release_base in
# lib/ffi/loader.ml and release_source in scripts/update-checksums.py.
set -euo pipefail

loader="$(dirname "$0")/../../lib/ffi/loader.ml"
version=$(awk -F'"' '/^let version =/{print $2}' "$loader")
if [ -z "$version" ]; then
  echo "could not find \`let version\` in $loader" >&2
  exit 1
fi

if [ "$(printf '%s\n' "$version" 0.47.2 | sort -V | head -n1)" = 0.47.2 ]; then
  echo "https://github.com/hegeldev/hegel/releases/download/libhegel/${version}"
else
  echo "https://github.com/hegeldev/hegel-rust/releases/download/libhegel-v${version}"
fi
