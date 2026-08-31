"""Sweep the capture parser across the space of date and time formats.

The parser's own test suite is written from the same assumptions as the parser,
so it can only confirm what was already thought of. This enumerates formats
mechanically instead — every strftime rendering of one known date and time,
plus the common human variants — and checks each against `dateutil`, a mature
parser written by other people.

A disagreement is a candidate gap, not a verdict: dateutil accepts plenty that
this parser should keep out of a title (a bare number is not a date). Read the
report, do not apply it.

    python tools/parser_sweep.py
"""

import json
import subprocess
import sys
from datetime import date, datetime

from dateutil import parser as du

# The corpus renders this one moment, so any correct parse is checkable.
TARGET_DATE = date(2026, 9, 2)          # a Wednesday
TARGET_TIME = (20, 0)                    # 20:00
TODAY = date(2026, 8, 31)                # matches the Rust example


def date_forms():
    """Every way this date might reasonably be written."""
    d = TARGET_DATE
    out = set()

    # Mechanical: separators x orders x year widths.
    for sep in ("/", "-", "."):
        for order in ("dmy", "mdy", "ymd"):
            for year in ("%Y", "%y"):
                parts = {
                    "dmy": ["%d", "%m", year],
                    "mdy": ["%m", "%d", year],
                    "ymd": [year, "%m", "%d"],
                }[order]
                out.add(d.strftime(sep.join(parts)))
            # And without a year at all.
            if order != "ymd":
                pair = ["%d", "%m"] if order == "dmy" else ["%m", "%d"]
                out.add(d.strftime(sep.join(pair)))

    # Unpadded, which is how people type.
    out.update({f"{d.day}/{d.month}", f"{d.day}/{d.month}/{d.year}",
                f"{d.day}-{d.month}-{d.year}"})

    # Month names, long and short, either order, with and without a year.
    for name in (d.strftime("%b"), d.strftime("%B")):
        for nm in (name, name.lower(), name.upper()):
            out.update({
                f"{d.day} {nm}", f"{nm} {d.day}", f"{d.day} {nm} {d.year}",
                f"{nm} {d.day} {d.year}", f"{nm} {d.day}, {d.year}",
            })

    # Ordinals.
    out.update({f"{d.day}nd {d.strftime('%B')}", f"{d.strftime('%B')} {d.day}nd",
                f"the {d.day}nd"})

    # Relative and named.
    out.update({"today", "tomorrow", "wed", "wednesday", "next wednesday",
                "next week", "in 2 days", "in two days", "this friday",
                "end of the month", "eom"})
    return sorted(out)


def time_forms():
    h24, m = TARGET_TIME
    h12 = h24 % 12 or 12
    out = {
        f"{h24}:{m:02d}", f"{h24}{m:02d}", f"{h12}pm", f"{h12} pm",
        f"{h12}:{m:02d}pm", f"{h12}:{m:02d} pm", f"{h12}p", f"{h12} p.m.",
        f"{h12}PM", f"{h12}:{m:02d}PM", f"{h24}h", f"{h12}.{m:02d}pm",
        f"{h24}:{m:02d}:00", f"at {h12}pm", f"@{h12}pm",
    }
    out.update({"noon", "midnight", "midday", "half past 8", "8 o'clock"})
    return sorted(out)


def run_parser(lines):
    """Feed the corpus through the real parser."""
    proc = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "ms-core", "--example", "parse_lines"],
        input="\n".join(lines),
        capture_output=True,
        text=True,
        encoding="utf-8",
    )
    if proc.returncode != 0:
        print(proc.stderr[-2000:], file=sys.stderr)
        sys.exit("the parser example failed to run")
    return [json.loads(l) for l in proc.stdout.splitlines() if l.startswith("{")]


def reference_understands(fragment):
    """Would a mature parser make a date or time of this on its own?"""
    try:
        du.parse(fragment, default=datetime(TODAY.year, TODAY.month, TODAY.day),
                 dayfirst=True, fuzzy=False)
        return True
    except (ValueError, OverflowError, TypeError):
        return False


def main():
    dates, times = date_forms(), time_forms()

    # "x <fragment>" so a failure shows up as the fragment surviving in the title.
    corpus = [f"x {f}" for f in dates + times]
    corpus += [f"x {d} {t}" for d in dates[:14] for t in times[:8]]

    results = run_parser(corpus)

    missed_date, missed_time, missed_combo = [], [], []
    for r in results:
        frag = r["input"][2:]
        got_something = r["consumed"] > 0
        if got_something:
            continue
        ref = reference_understands(frag)
        row = (frag, ref)
        if frag in dates:
            missed_date.append(row)
        elif frag in times:
            missed_time.append(row)
        else:
            missed_combo.append(row)

    def report(title, rows):
        print(f"\n{title}  ({len(rows)})")
        print("-" * 62)
        for frag, ref in sorted(rows, key=lambda x: (not x[1], x[0])):
            mark = "dateutil parses it" if ref else "(dateutil also declines)"
            print(f"  {frag:<34} {mark}")

    print(f"corpus: {len(corpus)} lines "
          f"({len(dates)} date forms, {len(times)} time forms, "
          f"{len(corpus) - len(dates) - len(times)} combinations)")
    report("DATE FORMS NOT RECOGNISED", missed_date)
    report("TIME FORMS NOT RECOGNISED", missed_time)
    print(f"\ncombination failures: {len(missed_combo)}"
          f" (shown only if a component was itself understood)")
    for frag, ref in sorted(missed_combo)[:15]:
        print(f"  {frag}")

    # Wrong answers matter more than missing ones.
    print("\nWRONG DATES (parsed, but not 2026-09-02)")
    print("-" * 62)
    wrong = [r for r in results
             if r["due"] and r["input"][2:] in dates and r["due"] != "2026-09-02"]
    for r in sorted(wrong, key=lambda r: r["input"]):
        print(f"  {r['input'][2:]:<34} -> {r['due']}")
    if not wrong:
        print("  none")


if __name__ == "__main__":
    main()
