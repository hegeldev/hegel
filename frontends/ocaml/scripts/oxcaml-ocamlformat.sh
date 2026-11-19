#!/usr/bin/env bash
# Print the path of the OxCaml ocamlformat that formats the cppo templates and
# the OxCaml-only files: $HEGEL_OXCAML_OCAMLFORMAT, else the ocamlformat of the
# first opam switch named like an OxCaml one that has it. The binary runs on
# its own, so it need not stay in a switch. Exits 1, saying so on stderr, when
# there is none, and 2 when $HEGEL_OXCAML_OCAMLFORMAT is not an executable.
set -euo pipefail
if [ -n "${HEGEL_OXCAML_OCAMLFORMAT:-}" ]; then
  if [ ! -x "$HEGEL_OXCAML_OCAMLFORMAT" ]; then
    echo "HEGEL_OXCAML_OCAMLFORMAT=$HEGEL_OXCAML_OCAMLFORMAT is not an executable" >&2
    exit 2
  fi
  echo "$HEGEL_OXCAML_OCAMLFORMAT"
  exit 0
fi
for switch in $(opam switch list --short 2>/dev/null | grep -E '\+ox|oxcaml' || true); do
  bin="$(opam var --switch="$switch" bin 2>/dev/null)/ocamlformat"
  if [ -x "$bin" ]; then
    echo "$bin"
    exit 0
  fi
done
echo "No OxCaml ocamlformat (set HEGEL_OXCAML_OCAMLFORMAT): skipping the *.ml.in, *.mli.in and .ocamlformat-ignore files" >&2
exit 1
