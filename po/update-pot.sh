#!/usr/bin/env bash
# Regenerates po/luxafor-status.pot from the sources, and merges it into
# every po/*.po. Run it after changing user-visible messages.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
xgettext --from-code=UTF-8 --language=Python --keyword=_ --add-comments=TRANSLATORS \
    --package-name=luxafor-status --msgid-bugs-address="https://github.com/NevilleDubuis/luxafor-status-linux/issues" \
    -o po/python.pot bin/luxafor-panel bin/luxafor-tray
xgettext --from-code=UTF-8 --language=Shell --keyword=t --keyword=tf --add-comments=TRANSLATORS \
    -o po/shell.pot bin/luxafor-toggle bin/luxafor-light bin/luxafor-callwatch
# Default status labels (share/statuses.conf), translated when the user has none of their own.
{
    printf 'msgid ""\nmsgstr ""\n"Content-Type: text/plain; charset=UTF-8\\n"\n\n'
    for label in "Available" "Busy" "On a call"; do
        printf '#: share/statuses.conf\nmsgid "%s"\nmsgstr ""\n\n' "$label"
    done
} > po/defaults.pot
sed -i "s/charset=CHARSET/charset=UTF-8/" po/python.pot po/shell.pot
msgcat --use-first po/python.pot po/shell.pot po/defaults.pot -o po/luxafor-status.pot
rm -f po/python.pot po/shell.pot po/defaults.pot
for po in po/*.po; do
    msgmerge --quiet --update --backup=none "$po" po/luxafor-status.pot
done
echo "po/luxafor-status.pot: $(grep -c '^msgid ' po/luxafor-status.pot) messages"
