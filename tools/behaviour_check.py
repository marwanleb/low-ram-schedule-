"""Check every claim in docs/parser-behaviour.md against the real parser.

The document is what a tester is handed, so it must not drift from the code.
Every table row in it is a case; this runs them all and reports mismatches.

    python tools/behaviour_check.py            # verify the document
    python tools/behaviour_check.py --emit     # print current results as rows

Rows are written as:

    | `input` | field=value, field=value | note |

where the middle column is what the parser should produce. Recognised fields:
title, due, at, span, byday, repeats, listed, scheduled, est, loc, tags,
priority, pinned, consumed. `title=<same>` means the line is left whole.
"""

import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOC = ROOT / "docs" / "parser-behaviour.md"
TODAY = "2026-08-31"  # a Monday; the Rust example pins the same date


def run(lines):
    proc = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "ms-core", "--example", "parse_lines"],
        input="\n".join(lines), capture_output=True, text=True,
        encoding="utf-8", cwd=ROOT,
    )
    if proc.returncode != 0:
        sys.exit(proc.stderr[-1500:])
    return [json.loads(l) for l in proc.stdout.splitlines() if l.startswith("{")]


def actual(row, field):
    """Read one field out of a parse result, as a comparable string."""
    match field:
        case "title":    return row["title"]
        case "due":      return row["due"] or "none"
        case "at":       return row["at"] or "none"
        case "span":     return row["span"] or "none"
        case "byday":    return str(row["byday"])
        case "repeats":  return str(row["repeats"]).lower()
        case "listed":   return str(row["listed"]).lower()
        case "scheduled":return str(row["scheduled"]).lower()
        case "est":      return str(row["estimate_min"] if row["estimate_min"] is not None else "none")
        case "loc":      return row["location"] or "none"
        case "consumed": return str(row["consumed"])
        case _:          return f"<unknown field {field}>"


def cases_from_doc():
    if not DOC.exists():
        sys.exit(f"{DOC} does not exist yet")
    text = DOC.read_text(encoding="utf-8")
    cases = []
    for line in text.splitlines():
        m = re.match(r"^\|\s*`([^`]+)`\s*\|\s*([^|]+?)\s*\|", line)
        if not m:
            continue
        inp, expect = m.group(1), m.group(2).strip()
        if expect in ("expected", "---", ""):
            continue
        cases.append((inp, expect))
    return cases


def main():
    cases = cases_from_doc()
    if not cases:
        sys.exit("no cases found — are the tables formatted as | `input` | field=value | ... |?")

    results = run([c[0] for c in cases])
    by_input = {r["input"]: r for r in results}

    failures = []
    for inp, expect in cases:
        row = by_input.get(inp)
        if row is None:
            failures.append((inp, expect, "parser produced no output"))
            continue
        for clause in [c.strip() for c in expect.split(",") if c.strip()]:
            if "=" not in clause:
                continue
            field, want = (x.strip() for x in clause.split("=", 1))
            want = want.strip("`")
            got = actual(row, field)
            if want == "<same>":
                want = inp
            if got != want:
                failures.append((inp, f"{field}={want}", f"{field}={got}"))

    print(f"{len(cases)} documented cases, {len(results)} parsed")
    if not failures:
        print("all claims in the document hold")
        return
    print(f"\n{len(failures)} MISMATCHES — the document and the parser disagree:\n")
    for inp, want, got in failures:
        print(f"  {inp!r}\n      documented: {want}\n      actual    : {got}")
    sys.exit(1)


if __name__ == "__main__":
    main()
