#!/usr/bin/env bash
# Prints the path of the in-tree libhegel build this frontend is tested
# against: the release cdylib `cargo build --release -p hegeltest-c` leaves
# under the workspace's target/ (or $CARGO_TARGET_DIR), named for the host.
# Fails if it has not been built; `just ocaml::build-libhegel` builds it.
set -euo pipefail

root="$(cd "$(dirname "$0")/../../.." && pwd)"
target="${CARGO_TARGET_DIR:-$root/target}"
case "$(uname -s)" in
  Darwin) lib=libhegel_c.dylib ;;
  Linux) lib=libhegel_c.so ;;
  MINGW*|MSYS*|CYGWIN*) lib=hegel_c.dll ;;
  *) echo "unsupported host $(uname -s) for libhegel" >&2; exit 1 ;;
esac
path="$target/release/$lib"
if [ ! -f "$path" ]; then
  echo "libhegel not found at $path; run \`just ocaml::build-libhegel\` or set HEGEL_LIBHEGEL_PATH" >&2
  exit 1
fi
echo "$path"
