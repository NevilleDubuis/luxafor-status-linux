#!/usr/bin/env bash
# Regenerates po/luxafor-status.pot from the sources, and merges it into
# every po/*.po. Run it after changing user-visible messages.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
python3 po/extract.py po/luxafor-status.pot
for po in po/*.po; do
    msgmerge --quiet --update --backup=none --no-wrap "$po" po/luxafor-status.pot
done
