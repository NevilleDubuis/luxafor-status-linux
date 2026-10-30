#!/usr/bin/env bash
# Compiles po/*.po into OUTDIR/<lang>/LC_MESSAGES/luxafor-status.mo.
# Usage: po/build.sh OUTDIR
set -euo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${1:?usage: po/build.sh OUTDIR}"
for po in "$SRC"/*.po; do
    lang="$(basename "$po" .po)"
    mkdir -p "$OUT/$lang/LC_MESSAGES"
    msgfmt --check -o "$OUT/$lang/LC_MESSAGES/luxafor-status.mo" "$po"
done
