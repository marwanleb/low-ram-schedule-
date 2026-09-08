# AGENT.md — driving `sched`

This is the whole interface for an agent populating Marwan's schedule. The app
never fetches anything by itself; nothing appears unless you put it there.

## The one rule that matters

**Every imported record needs a stable `external_id` that you derive from the
source, never generate randomly.** It is what makes a second run a no-op instead
of a second copy of everything.

    canvas:assignment:88213     good — stable across re-scans
    canvas:STAT240-pset-4          good — stable if the source names it that way
    a fresh uuid each run       WRONG — this is how the previous app ended up
                                posting every deadline twice

Re-running an identical import changes nothing. A record whose due date changed
updates in place. Items Marwan typed himself have no `external_id` and are never
touched by an import.

## Check before you add

    sched list --json
    sched export --json

Prefer `sched import` over repeated `sched add` for anything derived from an
external source: `add` has no key, so calling it twice really does create two
items. `add` is for one-off human-style capture.

## Import file

A JSON array. `external_id` and `title` are required; everything else optional.

```json
[
  {
    "external_id": "gcal:7f3a9c2e",
    "title": "STAT240 problem set 4",
    "due_at": "2026-09-04T23:59:00-05:00",
    "estimate_min": 120,
    "tags": ["math"]
  },
  {
    "external_id": "gcal:phys201",
    "title": "PHYS201",
    "tags": ["class"],
    "location": "Hall 2.106",
    "repeat": {
      "byday": ["mon", "wed"],
      "start_time": "10:30",
      "end_time": "12:00",
      "tz": "America/Chicago"
    }
  }
]
```

Then:

    sched import schedule.json

**`repeat` is how a timetable goes in.** A weekly class is one record with a
repeat rule, not fifteen dated copies — one row that renders every week and can
be edited in one place. An item with a `repeat` is treated as a commitment: it
occupies the week and stays out of the to-do list.

- `byday` — any of `mon tue wed thu fri sat sun`, at least one.
- `start_time` / `end_time` — 24-hour `HH:MM`, wall clock.
- `tz` — an IANA name (`America/Chicago`, `Europe/Paris`, `Asia/Beirut`). It
  fixes the class to that city's clock, so it still reads correctly from
  another country. Omit it and the time follows whatever machine is showing it,
  which is what you want for something like a wake-up alarm. **Never an
  abbreviation** — `CT` is rejected.
- Dropping `repeat` on a later import removes the rule, so a cancelled class
  stops appearing.

`due_at` is for one-off deadlines and wants a full RFC3339 timestamp including
the offset. `tags` are free-form; the four that also set a display colour are
`work`, `life`, `body`, `social`.

The batch applies completely or not at all. A record missing `external_id`, or
carrying an unknown time zone, is refused and **nothing** is written — a
half-applied import is worse than a failed one, because it looks like it worked.

## Commands

    sched add "<text>"          capture one line, same syntax as the app
    sched list [--today]        open items
    sched week [--on DATE]      the week's schedule
    sched done "<title start>"  tick off; refuses if the prefix is ambiguous
    sched except <id> <date>    cancel one occurrence of a repeat
    sched rm <id>               delete
    sched import <file.json>    bulk upsert (see above)
    sched export                every item as JSON
    sched stats [--tag T]       how much longer things take than estimated
    sched help                  syntax reference

Add `--json` to any read command for machine-readable output. `--db <path>`
targets a specific store; the default is the app's own.

Exit codes: `0` success, `1` refused (bad input, nothing written), `2` not found
or ambiguous.

## Capture syntax

`sched add` uses exactly the same parser as the app and the Telegram bot, so
anything below also works when typed by hand.

```
Type what you want to remember. Plain text always works:

> renew parking

Add details on the end. Anything the parser does not recognise stays
in the title, so ordinary sentences are safe.

  when        fri · friday · tues · mondays · tomorrow · today
              2 sep · 2 September 2026 · Sep 2
              2026-09-15 · 2-9-2026 · 2/9 · 2.9.26   (day first)
  time        5pm · 5 pm · 5p · 17:00 · noon · midnight
  a block     9:00-10:15 · 10:30-12:00p   (scheduled; kept out of the list)
  kind        due fri      → a task with a deadline
              on fri 9am   → an hour on the week, still tickable
  repeating   every        (say it plainly; one weekday is enough)
              mondays      (the plural says it too)
              daily        (every day of the week)
  estimate    ~2h · ~90m · ~1.5h
  tag         #math        (#work #life #body #social also set the colour)
  place       @Hall 2.106   (runs to the end of the line, spaces and all)
  travel      #floating    (follows this machine; otherwise it stays fixed
                            to the zone you created it in)
  important   !! in front

> math hw fri 5pm ~2h #math
> trash every tue 20:00
> PHYS201 every monday wednesday 10:30-12:00p @Hall 2.106
> gym every mon wed fri ~1h #body
> wake up daily 08:00 #floating
> pick up a friend from the airport 2 Sep 2026 8:00 pm
> dentist 14 october 9:30 am
> lab report due wed by 11:59 pm
> standup on friday 9am
> gym mon/wed/fri 6am
> !! renew parking tomorrow

Commands:

  help          this text
  list          everything still open
  list today    due today or overdue
  list week     due in the next seven days
  week          this week's schedule
  done: <text>  tick off the first task matching <text>

A line with no date is read back to you before it is filed, so something
sent in a hurry does not quietly become a task with no when.

Ten minutes before anything on the week starts, or anything in the list
falls due, this chat gets a reminder. Nothing to switch on — it pushes to
whoever last spoke to it.
```

## What not to do

- Do not invent deadlines. Only add what you actually found.
- Do not use `add` in a loop over scraped data; use `import`.
- Do not retry a failed `import` by switching to `add` — fix the record and
  re-run the import, which is safe.
