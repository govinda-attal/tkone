#!/usr/bin/env python3
"""Build and check the tkone schedule format Internet-Draft.

Sources, all in this crate:

  ietf/tkone-schedule.abnf    the grammar (single source)
  GRAMMAR.md                  grammar prose, validity and evaluation rules,
                              conformance examples (checked against the
                              Rust parser by src/grammar_tests.rs)
  ietf/draft.template.md      the IETF framing: front matter, abstract,
                              introduction, security and IANA sections

Outputs:

  GRAMMAR.md                  its ABNF blocks, refreshed from the .abnf file
  ietf/<docname>.md           the draft, in kramdown-rfc Markdown

Usage:

  python3 ietf/build.py             regenerate the outputs
  python3 ietf/build.py --check     fail if an output is out of date (for CI)
  python3 ietf/build.py --validate  check the conformance examples against the
                                    ABNF (needs `pip install abnf`)
  python3 ietf/build.py --render    also render the draft to .xml, .txt and
                                    .html (needs kramdown-rfc and xml2rfc; see
                                    ietf/README.md)

The options can be combined.
"""

import argparse
import difflib
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CRATE = HERE.parent
ABNF = HERE / "tkone-schedule.abnf"
GRAMMAR = CRATE / "GRAMMAR.md"
TEMPLATE = HERE / "draft.template.md"

ABNF_BLOCK = re.compile(r"<!-- abnf:(\w+) -->\n```abnf\n.*?```\n<!-- /abnf -->", re.S)
NOT_IN_DRAFT = re.compile(r"<!-- not-in-draft -->.*?<!-- /not-in-draft -->", re.S)
RFC_LINK = re.compile(r"\[RFC (\d+)\]\(https://www\.rfc-editor\.org/rfc/rfc\d+\)")
LINK = re.compile(r"\[([^\]]+)\]\(([^)]+)\)")
HEADING = re.compile(r"^(#{2,4}) (.+)$", re.M)


# ---------------------------------------------------------------------------
# ABNF
# ---------------------------------------------------------------------------

def abnf_sections() -> dict[str, str]:
    """The `; @section NAME` sections of the ABNF file, and `all` of them."""
    sections: dict[str, list[str]] = {}
    current = None
    for line in ABNF.read_text().splitlines():
        m = re.match(r"; @section (\w+)$", line)
        if m:
            current = m.group(1)
            sections[current] = []
        elif current:
            sections[current].append(line)
    out = {name: "\n".join(lines).strip("\n") for name, lines in sections.items()}
    out["all"] = "\n\n".join(out[name] for name in sections)
    return out


def refresh_grammar(grammar: str, abnf: dict[str, str]) -> str:
    """GRAMMAR.md with each `<!-- abnf:NAME -->` block replaced by that section."""

    def block(m: re.Match) -> str:
        name = m.group(1)
        if name not in abnf:
            sys.exit(f"GRAMMAR.md refers to ABNF section `{name}`, which {ABNF.name} does not have")
        return f"<!-- abnf:{name} -->\n```abnf\n{abnf[name]}\n```\n<!-- /abnf -->"

    return ABNF_BLOCK.sub(block, grammar)


# ---------------------------------------------------------------------------
# Draft
# ---------------------------------------------------------------------------

def slug(text: str) -> str:
    """A heading's anchor as GitHub writes it: `3.1 The start` -> `31-the-start`."""
    text = re.sub(r"[^\w\s-]", "", text.lower())
    return re.sub(r"\s", "-", text.strip())


def draft_body(grammar: str) -> str:
    """GRAMMAR.md sections 1-4, reshaped for kramdown-rfc."""
    body = grammar[grammar.index("## 1. Grammar"):]
    body = NOT_IN_DRAFT.sub("", body)
    body = re.sub(r"<!-- /?abnf(:\w+)? -->\n", "", body)
    # Horizontal rules: kramdown-rfc reads a line starting with `---` as a
    # document part separator (`--- back`), and the RFC tools add their own
    # spacing between sections.
    body = re.sub(r"^---\n", "", body, flags=re.M)
    body = body.replace("```abnf\n", "~~~ abnf\n").replace("```text\n", "~~~ text\n")
    body = re.sub(r"^```$", "~~~", body, flags=re.M)

    # Headings: drop GRAMMAR's numbering (the RFC tools number sections), move
    # each up a level, and give each an explicit, unique anchor.
    anchors: dict[str, str] = {}
    used: set[str] = set()
    parent = ""

    def heading(m: re.Match) -> str:
        nonlocal parent
        level, title = len(m.group(1)), m.group(2)
        bare = re.sub(r"^\d+(\.\d+)*\.?\s+", "", title)
        anchor = slug(bare)
        if level == 2:
            parent = anchor
        elif anchor in used:
            anchor = f"{parent}-{anchor}"
        used.add(anchor)
        anchors[slug(title)] = anchor
        return f"{'#' * (level - 1)} {bare} {{#{anchor}}}"

    body = HEADING.sub(heading, body)

    # Links: RFCs become citations, in-document links become section
    # references, and links to repository files keep only their text.
    body = RFC_LINK.sub(lambda m: f"{{{{RFC{m.group(1)}}}}}", body)

    def link(m: re.Match) -> str:
        text, target = m.group(1), m.group(2)
        if target.startswith("#") and target[1:] in anchors:
            return f"{{{{{anchors[target[1:]]}}}}}"
        return text

    body = LINK.sub(link, body)
    # Plain "RFC nnnn" mentions become citations, except inside code blocks.
    parts = re.split(r"(~~~ \w+\n.*?~~~)", body, flags=re.S)
    for i in range(0, len(parts), 2):
        parts[i] = re.sub(r"\bRFC (3339|9557|5234|7405|5545)\b", r"{{RFC\1}}", parts[i])
    body = "".join(parts)
    return body.strip() + "\n"


def build_draft(grammar: str, abnf: dict[str, str]) -> tuple[str, str]:
    """The draft's file name and text."""
    template = TEMPLATE.read_text()
    docname = re.search(r"^docname:\s*(\S+)", template, re.M).group(1)
    text = template.replace("<!-- include: GRAMMAR.md -->", draft_body(grammar))
    text = text.replace("<!-- include: abnf -->", f"~~~ abnf\n{abnf['all']}\n~~~")
    # Say so at the top of the generated file, inside the front matter.
    text = text.replace(
        "---\n",
        "---\n# Generated by ietf/build.py from ietf/draft.template.md, GRAMMAR.md and\n"
        "# ietf/tkone-schedule.abnf. Edit those, then run `python3 ietf/build.py`.\n",
        1,
    )
    return f"{docname}.md", text


# ---------------------------------------------------------------------------
# Checks
# ---------------------------------------------------------------------------

def table_specs(grammar: str, heading: str) -> list[str]:
    """The `code` spans in the first column of the table under `heading`."""
    start = grammar.index(f"\n{heading}\n")
    section = grammar[start + 1:]
    end = re.search(r"\n#{1,3} ", section[len(heading):])
    section = section[: len(heading) + end.start()] if end else section
    specs = []
    rows = [line for line in section.splitlines() if line.startswith("|")][2:]
    for row in rows:
        first = re.split(r"(?<!\\)\|", row.strip().strip("|"))[0]
        specs += [s.replace("\\|", "|") for s in re.findall(r"`([^`]+)`", first)]
    return specs


def validate(grammar: str) -> bool:
    """Check GRAMMAR.md's accepted and (syntactically) rejected examples against the ABNF."""
    try:
        from abnf.parser import Rule
    except ImportError:
        print("--validate needs the `abnf` package: pip install abnf", file=sys.stderr)
        return False

    class Schedule(Rule):
        pass

    Schedule.load_grammar(ABNF.read_text())

    def matches(text: str) -> bool:
        try:
            Schedule("schedule").parse_all(text)
            return True
        except Exception:
            return False

    ok = True
    accepted = table_specs(grammar, "### Accepted")
    rejected = table_specs(grammar, "### Rejected")
    for spec in accepted:
        if not matches(spec):
            print(f"ABNF rejects `{spec}`, which GRAMMAR.md lists as accepted", file=sys.stderr)
            ok = False
    for spec in rejected:
        if matches(spec):
            print(f"ABNF accepts `{spec}`, which GRAMMAR.md lists as rejected", file=sys.stderr)
            ok = False
    if ok:
        print(f"ABNF agrees with GRAMMAR.md: {len(accepted)} accepted, {len(rejected)} rejected examples")
    return ok


def render(draft: Path) -> bool:
    """Render the draft to .xml (kramdown-rfc), then .txt and .html (xml2rfc)."""
    missing = [tool for tool in ("kramdown-rfc", "xml2rfc") if not shutil.which(tool)]
    if missing:
        print(f"--render needs {' and '.join(missing)}; see ietf/README.md", file=sys.stderr)
        return False
    xml = draft.with_suffix(".xml")
    with xml.open("w") as out:
        if subprocess.run(["kramdown-rfc", draft.name], cwd=draft.parent, stdout=out).returncode != 0:
            return False
    return subprocess.run(["xml2rfc", "--text", "--html", xml.name], cwd=draft.parent).returncode == 0


# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="fail if an output is out of date; write nothing")
    parser.add_argument("--validate", action="store_true", help="check the conformance examples against the ABNF")
    parser.add_argument("--render", action="store_true", help="render the draft to .xml, .txt and .html")
    args = parser.parse_args()

    abnf = abnf_sections()
    grammar = refresh_grammar(GRAMMAR.read_text(), abnf)
    name, draft = build_draft(grammar, abnf)
    outputs = {GRAMMAR: grammar, HERE / name: draft}

    ok = True
    for path, text in outputs.items():
        current = path.read_text() if path.exists() else ""
        if current == text:
            continue
        if args.check:
            ok = False
            rel = path.relative_to(CRATE)
            diff = difflib.unified_diff(current.splitlines(), text.splitlines(), f"{rel} (on disk)", f"{rel} (expected)", lineterm="", n=1)
            print(f"{rel} is out of date; run `python3 ietf/build.py`:", file=sys.stderr)
            print("\n".join(list(diff)[:40]), file=sys.stderr)
        else:
            path.write_text(text)
            print(f"wrote {path.relative_to(CRATE)}")

    if args.validate:
        ok = validate(grammar) and ok
    if args.render and not args.check:
        ok = render(HERE / name) and ok
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
