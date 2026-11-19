#!/usr/bin/env python3
"""Extracts the translatable messages of the Rust sources into a .pot file:
the first string of every tr("…") and trf("…", …) call, plus the names of the
shipped statuses (share/statuses.conf). Usage: po/extract.py OUT.pot"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CALL = re.compile(r'\btrf?\(\s*"')


def read_literal(text, start):
    """Reads a Rust string literal starting after its opening quote.
    Returns (value, end index)."""
    out, i = [], start
    while i < len(text):
        c = text[i]
        if c == "\\":
            n = text[i + 1]
            if n == "\n":  # line continuation: skip the newline and indentation
                i += 2
                while i < len(text) and text[i] in " \t\n":
                    i += 1
                continue
            out.append({"n": "\n", "t": "\t", '"': '"', "\\": "\\", "'": "'", "0": "\0"}.get(n, n))
            i += 2
            continue
        if c == '"':
            return "".join(out), i
        out.append(c)
        i += 1
    raise ValueError("unterminated string")


def po_quote(s):
    return '"' + s.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n").replace("\t", "\\t") + '"'


def main():
    messages = {}  # msgid -> [references]
    for path in sorted((ROOT / "src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for m in CALL.finditer(text):
            # Skip definitions ("fn tr(") and comments.
            line_start = text.rfind("\n", 0, m.start()) + 1
            line = text[line_start:m.start()]
            if "fn " in line or line.lstrip().startswith("//"):
                continue
            msgid, _end = read_literal(text, m.end())
            lineno = text.count("\n", 0, m.start()) + 1
            messages.setdefault(msgid, []).append(f"{path.relative_to(ROOT)}:{lineno}")
    # Names of the shipped statuses, translated while the user has none of their own.
    for line in (ROOT / "share" / "statuses.conf").read_text(encoding="utf-8").splitlines():
        parts = [p.strip() for p in line.split("|")]
        if not line.strip().startswith("#") and len(parts) >= 3 and parts[2]:
            messages.setdefault(parts[2], []).append("share/statuses.conf")
    out = [
        "# Messages of luxafor-status.",
        "# This file is distributed under the same license as the luxafor-status package (MIT).",
        'msgid ""',
        'msgstr ""',
        '"Project-Id-Version: luxafor-status\\n"',
        '"Report-Msgid-Bugs-To: https://github.com/NevilleDubuis/luxafor-status-linux/issues\\n"',
        '"MIME-Version: 1.0\\n"',
        '"Content-Type: text/plain; charset=UTF-8\\n"',
        '"Content-Transfer-Encoding: 8bit\\n"',
        "",
    ]
    for msgid, refs in messages.items():
        if not msgid:
            continue
        out.append("#: " + " ".join(refs))
        out.append(f"msgid {po_quote(msgid)}")
        out.append('msgstr ""')
        out.append("")
    pathlib.Path(sys.argv[1]).write_text("\n".join(out), encoding="utf-8")
    print(f"{sys.argv[1]}: {len(messages)} messages")


if __name__ == "__main__":
    main()
