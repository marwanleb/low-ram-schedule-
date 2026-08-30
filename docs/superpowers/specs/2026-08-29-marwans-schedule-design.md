# Marwan's Schedule — Design

Status: draft for review
Date: 2026-08-29
Supersedes: `dash-app` (archived, not forked)

## 1. Purpose

Paper is the incumbent and it wins at almost everything. This app exists only
because paper cannot do three things:

1. **Recurring events don't get rewritten.** Define MATH210 once; it appears every
   week without further effort.
2. **Per-item timing.** Estimate versus actual, accumulated over weeks, until the
   schedule stops being fiction.
3. **The list is in your pocket.** One place, replacing Reminders plus the
   post-it on the laptop.

A feature that does not serve one of those three does not go in. This is the
whole scope test, and it is deliberately narrow: the predecessor failed by
growing into a desktop replacement.

### Non-goals

Explicitly out, and each was in the predecessor: file browser, terminal, music
control, system monitor, app-launcher macros, email integration, an in-app
agent runtime, any always-on background service, any listening socket.

### Nothing populates itself

The app has no ingest of its own — no poller, no watcher, no listening socket.
Items appear because you typed them, sent them over Telegram, or ran the `sched`
CLI (§8). An agent can drive that CLI, but only when you tell it to.

The principle behind this is that writing a deadline down is what makes you know
it; an auto-filled list is a list you have not read. Deliberate bulk import is
fine — it is *your* action. What is excluded is anything that fills the list while
you are not looking.

This reverses the predecessor, which polled Canvas on a timer and had an agent
POST tasks into it unattended.

---

## 2. Frontend contract (frozen first)

The frontend is being built independently. This section is the coordination
boundary and should be treated as stable; changes here need to be agreed, not
assumed.

All calls are Tauri commands. All times crossing the boundary are **RFC3339 with
offset**. Durations are integer minutes. `null` means absent, never `0` or `""`.

### Types

```jsonc
// Item — the single object the whole app is built on. There is no type field:
// "task" and "commitment" are views over this, not kinds of it. See §3.1.
{
  "id": "itm_7f3a",
  "title": "STAT240 problem set",
  "tags": ["math"],
  "category": "work",                       // work|life|body|social; null = none
  "listed": true,                           // renders in the To Do pane; §3.1
  "due_at": "2026-09-04T23:59:00-05:00",   // null = no deadline
  "estimate_min": 120,                      // null = unestimated
  "recurs": true,                           // has a recurrence rule
  "done_on": ["2026-09-01"],                // occurrence dates completed; see §3.2
  "done": false,                            // convenience: completed for the
                                            // occurrence in the current query context
  "created_at": "2026-08-29T14:02:11-05:00",
  "source": "self",            // "self" | "import"
  "external_id": null          // set only by `sched import`; §8
}

// Placement — an item occupying real time. Generated or explicit.
{
  "id": "plc_2026-09-01_itm_7f3a",  // stable, derived; see §4.3
  "item_id": "itm_7f3a",
  "starts_at": "2026-09-01T13:00:00-05:00",
  "ends_at":   "2026-09-01T15:00:00-05:00",
  "origin": "recurrence",      // "recurrence" | "oneoff" | "moved"
  "moved_from": null,          // date string when origin="moved", else null
  "pinned_tz": "America/Chicago",  // null = systime (follows the machine)
  "foreign": true              // true when pinned_tz differs from viewing_tz:
                               // render a zone marker. Never hide it; §5.3.
}
// Note: origin="recurrence" placements are generated on read and have no row in
// the placements table, which is why that table's CHECK allows only the two
// persisted origins. The frontend sees all three and should not assume a
// placement is editable purely because it was returned.

// Recurrence — the rule attached to an item. null when the item does not recur.
{
  "item_id": "itm_7f3a",
  "byday": ["mon", "wed"],
  "start_time": "09:00",       // wall clock, NOT an instant; see §5
  "end_time": "10:15",
  "tz": "America/Chicago",     // pinned to this zone; null = SYSTIME, follows
                               // you as you travel. See §5.3 — this is the whole
                               // travel model and the default is NOT null.
  "from_date": "2026-08-25",
  "until_date": null,          // null = open-ended
  "except_on": []              // dates, empty by default, user-editable
}

// Session — a real interval of work.
{
  "id": "ses_91b2",
  "item_id": "itm_7f3a",       // never null
  "placement_id": "plc_...",   // null when started from the list
  "started_at": "2026-09-01T13:04:00-05:00",
  "ended_at": null,            // null = running
  "discarded": false
}

// Week — what the week view renders. Cannot fail; see §5.
{
  "anchor": "2026-08-31",      // Monday of the week
  "viewing_tz": "Asia/Beirut", // the OS zone at render time, read fresh; §5.3
  "days": [
    { "date": "2026-08-31", "placements": [ /* Placement */ ] }
    // ... 7 entries, always exactly 7, always in order
  ],
  "diagnostics": [             // non-fatal problems; render as a banner
    { "level": "warn", "item_id": "itm_x", "message": "Recurrence skipped: until_date precedes from_date" }
  ]
}
```

### Commands

| Command | Args | Returns |
|---|---|---|
| `get_week` | `anchor: "YYYY-MM-DD"` | `Week` |
| `get_items` | `filter: { done?, listed?, recurs?, tag?, due_before? }` | `Item[]` |
| `set_listed` | `id, listed: bool` | `Item` |
| `add_from_text` | `text: string` | `ParseResult` |
| `set_done` | `id, done: bool, on_date?` | `Item` |
| `delete_item` | `id` | `void` |
| `set_recurrence` | `item_id, rule \| null` | `Item` |
| `set_recurrence_tz` | `item_id, tz \| null` | `Recurrence` |
| `get_tz_review` | — | `TzReview \| null` |
| `add_except_on` | `item_id, date` | `Recurrence` |
| `remove_except_on` | `item_id, date` | `Recurrence` |
| `move_placement` | `placement_id, starts_at, ends_at` | `Placement` |
| `timer_start` | `item_id, placement_id?` | `Session` |
| `timer_stop` | `session_id` | `Session` |
| `get_active_session` | — | `Session \| null` |
| `get_stats` | `tag?` | `Stats` |

```jsonc
// ParseResult — add_from_text always succeeds; see §6.
{
  "item": { /* Item */ },
  "consumed": ["fri", "5pm", "~2h"],   // tokens the parser recognized
  "title": "math hw"                   // what remained
}

// Stats — the payoff, as one sentence-ready number. Not a plot.
{
  "tag": "math",               // null when the request had no tag filter
  "n": 14,                     // completed, non-discarded, both values present
  "pct_over": 30,              // "30% longer than you estimate". Negative = faster.
  "phrase": "30% longer",      // pre-formatted; null when n < 5
  "confident": true            // n >= 5; below that pct_over is present but noisy
}

// TzReview — emitted once when the OS zone changes; see §5.3. null when the
// current zone has already been reviewed.
{
  "from_tz": "America/Chicago",
  "to_tz": "Asia/Beirut",
  "pinned":   [ /* Item */ ],  // stay on their origin zone unless you say otherwise
  "systime": [ /* Item */ ]    // already following the machine clock
}
```

### Events (Rust → frontend)

| Event | Payload | When |
|---|---|---|
| `week:changed` | `{ anchor }` | any write that could alter a rendered week |
| `timer:tick` | `{ session_id, elapsed_min }` | every 30s while a session runs |
| `capture:received` | `Item` | Telegram capture landed |
| `items:changed` | `{}` | an outside writer (the §8 CLI) touched the DB — detected by comparing SQLite's `data_version` pragma when the overlay becomes visible, not by watching the filesystem |
| `tz:changed` | `TzReview` | OS timezone differs from last render; §5.3 |

`timer:tick` is 30s, not 1s. The frontend renders the running clock locally from
`started_at`; the tick only corrects drift. There is no per-second IPC.

---

## 3. Data model

```sql
CREATE TABLE items (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL,
  tags          TEXT NOT NULL DEFAULT '',       -- comma-separated, open set
  category      TEXT,                           -- closed set, drives colour; §3.3
  listed        INTEGER NOT NULL DEFAULT 1,     -- 1 = renders in the To Do pane; §3.1
  due_at        TEXT,                           -- UTC instant
  due_tz        TEXT,                           -- zone it was set in, for display
  estimate_min  INTEGER,
  created_at    TEXT NOT NULL,
  source        TEXT NOT NULL DEFAULT 'self'
                CHECK (source IN ('self','import')),
  external_id   TEXT                            -- caller-owned upsert key; §8
);

CREATE TABLE recurrence (
  item_id     TEXT PRIMARY KEY REFERENCES items(id) ON DELETE CASCADE,
  byday       TEXT NOT NULL,          -- "mon,wed"
  start_time  TEXT NOT NULL,          -- "09:00" wall clock
  end_time    TEXT NOT NULL,          -- "10:15"
  tz          TEXT,                   -- IANA zone = pinned; NULL = systime. See §5.3.
  from_date   TEXT NOT NULL,
  until_date  TEXT,                   -- null = open-ended
  except_on   TEXT NOT NULL DEFAULT ''  -- comma-separated dates, empty default
);

CREATE TABLE completions (
  item_id   TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  on_date   TEXT NOT NULL DEFAULT '',  -- '' = the item itself; else occurrence date
  done_at   TEXT NOT NULL,             -- UTC instant
  PRIMARY KEY (item_id, on_date)
);

CREATE TABLE placements (
  id          TEXT PRIMARY KEY,
  item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  starts_at   TEXT NOT NULL,
  ends_at     TEXT NOT NULL,
  origin      TEXT NOT NULL CHECK (origin IN ('oneoff','moved')),
  moved_from  TEXT
);

CREATE TABLE sessions (
  id            TEXT PRIMARY KEY,
  item_id       TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
  placement_id  TEXT,                  -- intentionally NOT a foreign key; see below
  started_at    TEXT NOT NULL,
  ended_at      TEXT,
  discarded     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_sessions_item ON sessions(item_id);
CREATE INDEX idx_placements_item ON placements(item_id);
```

### Why `sessions.placement_id` is not a foreign key

A placement can be destroyed while a session references it — edit a recurrence
rule mid-work and the generated placement you started against ceases to exist.
The session must survive that. `item_id` is the durable link and carries all the
estimate-versus-actual value; `placement_id` is best-effort provenance for the
"did I use the block I scheduled" question. A dangling `placement_id` is a
normal, expected state, not corruption.

This is a deliberate denormalization and the reason is exactly the crash class
raised in review.

### 3.1 There is no item type

An earlier draft had `kind IN ('task','commitment')`. It is gone. The field was
only deciding two things — whether to draw a checkbox, and whether recurrence was
allowed — and both turn out to be wrong as type-level distinctions:

- A task can recur. "Take out the trash, Tuesdays" is a repeating task.
- A commitment can be completed. Attending a lecture is a thing you did or
  didn't do, and tracking that is useful.

So "task" and "commitment" are **queries, not kinds**:

- *the list* → items where `listed = 1`, not yet done for the occurrence
- *the week* → items with placements in range, regardless of `listed`

Same table, same object, two lenses.

#### `listed` gates one view and nothing else

A first draft derived list membership from "has no placement today," which fails
on the first Tuesday: MATH210 has no Tuesday placement, so every class would flood
Tuesday's to-do list. Membership is not derivable — "attend MATH210" is not a
to-do, "take out the trash" is, and both recur weekly at a fixed time.

So `items.listed` is a boolean, and it is deliberately **not** a revival of
`kind`. The difference matters:

| | `kind` (removed) | `listed` (kept) |
|---|---|---|
| gated | whether it can recur; whether it can complete | which pane renders it |
| effect on the object | changed its capabilities | none |

Both listed and unlisted items recur, complete, get timed, and take placements
identically. Per-occurrence completion still lives in `completions` (§3.2), so
the bug that removing `kind` fixed does not return.

**Default:** the parser sets `listed = 0` when the input carries an explicit time
*range*, and `1` otherwise — typing `9:00-10:15` means you are scheduling, not
listing. One toggle overrides it.

### 3.2 Completion is per-occurrence

Dropping `kind` surfaced a bug the earlier draft had: `done_at` lived on the item,
so completing a *recurring* task would have marked it done forever rather than
done for this week.

Completions therefore live in their own table keyed by `(item_id, on_date)`:

| Case | Row |
|---|---|
| one-off task done | `(itm, '')` |
| recurring task done this week | `(itm, '2026-09-01')` |
| lecture attended | `(itm, '2026-09-01')` |

One mechanism covers all three, and "did I actually attend class this semester"
becomes queryable for free.

`on_date` defaults to `''` rather than `NULL` deliberately: SQLite treats NULLs
as distinct in a composite primary key, so a nullable column would silently
permit duplicate completion rows for the same item.

### 3.3 `category` versus `tags`

Two fields that look redundant and are not.

- **`category`** — closed set (`work` | `life` | `body` | `social`), at most one,
  nullable. Exists solely so the UI has a reliable colour to draw: the design
  language keys every tile border, checkbox fill, and day chip off it. A closed
  set is required because an open one has no colour to assign.
- **`tags`** — open set, many per item. Used for filtering and for grouping the
  estimate ratio (§5.6). `#math`, `#math210`, `#errand`.

The parser fills `category` from the first `#tag` matching the closed set, and
leaves it null otherwise; the tag itself is kept either way. So `#body` both
tags and colours, while `#math` only tags.

### Recurrence is weekly-only

`byday` + times, no monthly or yearly rules. A student schedule is weekly, and
monthly recurrence drags in the entire month-end problem family (the 31st in
February, "last Tuesday", leap years). Not building it removes those bugs rather
than handling them.

---

## 4. Recurrence expansion

### 4.1 Instances are computed, never stored

The week view expands rules on read. No row is ever written for "MATH210 on every
Monday until December."

Materialized recurrence is where calendar apps accumulate their worst bugs:
regeneration races, orphans after a rule edit, unbounded growth. Computing on
read costs microseconds for one week and makes a rule edit instantly correct
everywhere with no migration. The only persisted rows are the exceptions you
created deliberately.

### 4.2 Expansion algorithm

For each of the 7 days in the requested week:

1. Select recurrence rules where `from_date <= day` and
   (`until_date` is null or `day <= until_date`).
2. Skip the rule if the day's weekday is not in `byday`.
3. Skip the rule if the date appears in `except_on`.
4. Resolve `start_time`/`end_time` against the day **in the rule's own zone**
   (`recurrence.tz`), or in the current OS zone when `tz` is NULL. Never via a
   fixed offset — see §5.3.
5. Append explicit `placements` whose `starts_at` falls on the day.
6. Sort by `starts_at`, stable by `item_id`.

Overlap is legal. Two items at Monday 09:00 is a real thing that happens and is
rendered side by side; it is never an error and never causes one to be dropped.

### 4.3 Placement IDs are derived, not stored

Generated placements have id `plc_{date}_{item_id}`. This is deterministic, so
the frontend can hold a reference across a refetch, and a session started
against it keeps a meaningful `placement_id` even though no row exists.

### 4.4 Cancel versus move

Two mechanisms, cleanly separated:

- **Cancel** → add the date to `except_on`. The instance stops being generated.
- **Move** → add the date to `except_on` *and* insert a `placements` row with
  `origin='moved'` and `moved_from` set to the original date.

A move is therefore a suppression plus an addition. The two mechanisms cannot
contradict each other because they operate in different directions.

---

## 5. Error handling

Requirement from review: parameterized scheduling must degrade, never crash.

### 5.1 The core guarantee

**`get_week` is a total function.** For any database state, including a corrupt
or contradictory one, it returns a `Week` with exactly 7 days. Rules that cannot
be resolved are skipped and reported in `diagnostics`; they never abort the
response.

Implementation rules for the expansion path:

- No `unwrap()`, `expect()`, or indexing that can panic anywhere in expansion.
- Every rule is resolved inside a function returning `Result`; an `Err` becomes
  a diagnostic and the loop continues to the next rule.
- A partial week with a visible warning beats both a crash and a silently
  incomplete week. Silent omission is the failure mode to avoid — the user must
  be told something was dropped.

### 5.2 Catalogue

| Condition | Handling |
|---|---|
| `until_date` < `from_date` | Rejected at write time with a message. If already stored, rule is skipped + diagnostic. |
| `end_time` <= `start_time` | Rejected at write. Stored instances skipped + diagnostic. |
| `except_on` date outside the rule's range | Silent no-op. Not an error. |
| `except_on` date the rule never generates | Silent no-op. Not an error. |
| Malformed date in `except_on` | That entry ignored + diagnostic; the rest of the rule still expands. |
| Two rules overlapping in time | Legal. Rendered side by side. |
| Placement orphaned by a rule edit | Stands alone as a one-off. Not cleaned up, not an error. |
| Session referencing a vanished placement | Survives on `item_id`. `placement_id` renders as "block deleted". |
| Empty `byday` | Rule generates nothing + diagnostic. |
| `from_date` far in the past | Expansion is per-week, so cost is constant regardless. |
| `recurrence.tz` is an unknown IANA name | Rule treated as systime + diagnostic. Never panics on lookup failure. |
| OS reports a zone `chrono-tz` doesn't know | Fall back to UTC for rendering + diagnostic banner. The week still renders. |
| Pinned item lands on a DST gap **in its own zone** while you're in another | Resolved in the item's zone per §5.3, then converted. Same three-variant handling. |
| Zone changes while a session is running | No effect. Sessions are UTC instants; elapsed time is unaffected by travel. |

### 5.3 Travel between timezones: pinned vs systime

The user moves regularly between **America/Chicago**, **Europe/Paris**, and
**Asia/Beirut**. This is routine, not an edge case, and it is the reason
`recurrence.tz` exists.

#### Two behaviours, both required

| | Stored | On travel |
|---|---|---|
| **Pinned** (`tz` set) | wall clock + IANA zone | instant fixed; displayed local time moves |
| **Systime** (`tz` NULL) | wall clock only | wall clock fixed; instant moves |

The canonical pair, in the user's own framing:

```
wake up   08:00  systime            → 08:00 in Austin, 08:00 in Beirut
class     12:00  America/Chicago    → 12:00 in Austin, 20:00 seen from Beirut
```

Neither behaviour can express the other. Put class on systime and you attend at
the wrong hour; put wake-up on a pinned zone and it drifts across your day the
moment you land. This is why iCalendar carries `TZID` on some times and omits it
on others — the distinction is irreducible, and every calendar that tried to
collapse it has had to add it back.

The choice is per-recurrence, set once when the item is created, and never typed:
the zone comes from the system clock at creation time, and switching an item to
systime is a toggle.

#### The current zone is an input, never state

`get_week` reads the OS timezone **fresh on every call** and never caches or
persists it. Consequences:

- Landing in Beirut and letting the laptop update its clock is the entire
  migration. No sync, no user action, no stored "home" location to go stale.
- `Week.viewing_tz` reports what was used, so the frontend never has to guess.
- Nothing in the database changes when you travel. Travel is a *render*
  parameter.

#### Offsets between these three cities are not constant

America/Chicago and Europe/Paris change DST on different dates, so Austin↔Paris
is 7 hours for most of the year and 6 hours during two shoulder windows. Asia/Beirut
follows a third schedule and has changed it at short political notice within
recent memory.

Therefore: **all conversions resolve through the IANA zone via `chrono-tz`.**
Fixed-offset arithmetic anywhere in the codebase is a bug that will surface
twice a year. This corrects §9 of the earlier draft, which stated `chrono-tz`
was unnecessary — it was, while everything was machine-local; it no longer is.

Keeping `chrono-tz` current matters more than for a typical app, since two of the
three zones have volatile rules. Treat a tzdata bump as a routine dependency
update rather than something to defer.

#### Default is pinned, and the choice is surfaced

New recurrences default to **pinned to the zone the item was created in**. The
costs are asymmetric: a class left on systime makes you miss class, while a
wrongly-pinned wake-up makes you go to the gym at an odd hour. Default to the side
whose failure is cheap.

To keep that from being a silent wrong guess, a zone change emits `tz:changed`
carrying a `TzReview` — one screen listing which recurring items are pinned and
which follow systime, each toggleable in one tap:

```
You're now in Asia/Beirut (was America/Chicago)

  pinned — still on Austin time
    MATH210      09:00 CT  → shows 17:00      [use systime]
    work shift 14:00 CT  → shows 22:00      [use systime]

  systime — following this machine
    gym        18:00                        [anchor to Beirut]
```

Reviewed once per zone change, then dismissed. This is the only travel-related
thing the user is ever asked, and it is optional — dismissing it leaves
everything pinned, which is correct for the common case.

#### Foreign placements are marked, never hidden

A placement whose `pinned_tz` differs from `viewing_tz` sets `foreign: true`
and renders with a zone marker. It is **not** filtered out. Hiding it would be
silent omission, which §5 already identifies as the worst available failure mode
— the user decides whether a 17:00-in-Beirut lecture is one they are attending.

### 5.4 DST and ambiguous local times

Recurring times are stored as **wall clock**, not UTC. "Class at 09:00" means
09:00 in March and 09:00 in November; storing UTC would shift it by an hour
across the transition.

Resolution goes through the item's own zone (`recurrence.tz`) for pinned items,
or the current OS zone for systime ones — never through a fixed offset, per
§5.3.

#### A block is placed as a pair, not as two independent ends

`from_local_datetime` returns `LocalResult`, which has three variants and all
three occur in real schedules: one instant, two (the repeated hour), or none
(the skipped hour). Unwrapping it is a twice-yearly panic.

But handling each end *separately* is not enough either, and this is subtle.
On the autumn fall-back night, `01:30–02:30` has two readings:

```
first  01:30 (-05:00) → 02:30 (-06:00)   two hours
second 01:30 (-06:00) → 02:30 (-06:00)   one hour
```

Two earlier attempts each got half of it right:

| approach | length | what the tile reads |
|---|---|---|
| resolve both ends literally | 2h ✗ | `01:30–02:30` ✓ |
| resolve start, add written duration | 1h ✓ | `01:30–01:30` ✗ |

**So the pair is chosen together.** Enumerate every instant each end maps to,
and take the combination whose elapsed time equals the written duration; among
equals, the earliest start. That yields `01:30–02:30` lasting one hour — the
times as written and the length as written.

When no consistent pairing exists — the skipped hour, where the written time
does not occur at all — the start moves forward to the first instant that does
exist and the written duration is preserved from there. A `02:00–02:30` block on
the spring-forward Sunday becomes `03:00–03:30` rather than collapsing to zero.

Every clock-change placement records a diagnostic, so a block that moved says so
rather than quietly shifting under the user.

Sessions store **UTC instants**, because a session is a real moment rather than a
wall-clock intention. This split is deliberate and is the standard resolution.

### 5.5 Orphaned running sessions

A crash or a forgotten stop leaves `ended_at` null indefinitely, which would
poison the actual-versus-estimate data — the one number the app exists to
produce.

On startup, any session open longer than **12 hours** is marked
`discarded = 1` and surfaced for the user to correct or delete. It is never
silently included in stats, and never silently deleted. Only one session may run
at a time; `timer_start` while another runs stops the previous one first and
reports that it did.

### 5.6 Deriving the estimate ratio

The output is one number in a sentence — *"STAT240 problem sets take 30% longer than
you estimate"* — not a chart. A plot of this is noise; the percentage is the
entire finding.

**Median of per-item ratios, not ratio of totals.** For each item with both an
estimate and completed non-discarded sessions, compute `actual / estimate`, then
take the median across items.

The alternative — summing all actuals and dividing by summed estimates — lets a
single large item dominate the result. One eight-hour project would swamp twenty
half-hour tasks and the number would stop describing your typical day. The median
is also robust to the one session you forgot to stop before the 12-hour cutoff
caught it.

`pct_over = round((median_ratio - 1) * 100)`. Negative means faster than
estimated. `phrase` is null below **n = 5**, because a ratio from three data
points reads as authoritative while being mostly noise — the number is withheld
rather than qualified.

---

## 6. Capture parser

Deterministic, offline, no LLM. Instant, free, and unit-testable.

### The rule that makes it usable

**Scan from the right. Stop at the first token not recognized. Everything to the
left is the title.**

```
math hw fri 5pm ~2h                → "math hw"          Fri 17:00, est 120
renew parking                      → "renew parking"
meet Sarah about the March report  → "meet Sarah about the March report"
MATH210 mon wed 9:00-10:15 @Hall 1.204 → "MATH210"  recurring Mon/Wed, room Hall 1.204
```

The third line is why the rule exists. "March" is date-shaped, but the scan hits
"report" first and halts, so the title survives intact. A scan-anywhere parser
mangles it. This rule is unambiguous, cheap, and its failure mode is always
"treated it as plain text" rather than "guessed wrong."

### Grammar

Recognized only as trailing tokens, in any order:

| Token | Meaning |
|---|---|
| `mon` … `sun`, multiple allowed | weekday(s) |
| `every` (or `weekly`) | **states** that it repeats; see below |
| `today`, `tomorrow` | relative date |
| `YYYY-MM-DD`, `DD/MM` | absolute date |
| `5pm`, `17:00` | time of day |
| `9:00-10:15` | time range → a placement |
| `~2h`, `~90m` | estimate |
| `#tag` | tag |
| `@text` | location, appended to title metadata |
| `!!` (leading) | priority |

### Recurrence is stated, not inferred

An earlier draft read recurrence off the number of weekdays typed: one meant a
due date, two or more meant a repeat. That is a guess wearing the costume of a
rule, and it cannot express a weekly item that falls on one day — `trash tue
20:00` was unsayable.

`every` says it outright:

```
math hw fri              → due Friday, once
trash every tue 20:00    → repeats weekly
gym every mon wed fri    → repeats weekly
```

Several weekdays with no `every` are still treated as a repeat, since nothing
else is coherent, but that is a convenience rather than the mechanism.

`every` is only read where trailing tokens are already being scanned, so
"water the plants every day" stays prose — `day` halts the scan before `every`
is reached.

### Guarantees

- **`add_from_text` never returns an error.** Unparseable input becomes an item
  whose title is the entire raw string. Capture must never be lost to a syntax
  mistake.
- Recognized tokens are removed from the title; `ParseResult.consumed` reports
  what was taken so the UI can show what it understood.
- Bare text is always valid input. The syntax is opt-in; nothing must be learned
  to use the app.

### Test cases (must exist before implementation)

Both directions matter: tokens correctly consumed, and English correctly left
alone. Cases include the March example, `"buy milk for tomorrow's breakfast"`
(must not parse `tomorrow`), `"pay rent 1/9"`, `"gym mon wed fri ~1h"`,
`"!! renew parking"`, and empty/whitespace input.

---

## 7. Telegram capture

Reuses the long-poll transport salvaged from `telegram.rs`. The LLM parsing
inside it is deleted and replaced by §6.

| Message | Effect |
|---|---|
| any text | parsed as capture, item created, confirmation echoed |
| `list` | today's items |
| `week` | the week summary |
| `done: <title prefix>` | marks matching item done; ambiguous prefix asks which |
| `push: <title> <date>` | moves due date |

Why Telegram over a synced-folder or CalDAV approach: the app is already
installed, it is genuinely bidirectional, push notifications come free, it works
off your network, and it needs no conflict-resolution design. Undelivered
messages queue for 24h, so captures made while the PC is off drain on next
launch.

Accepted tradeoff: task text transits Telegram's servers. Judged acceptable for
this content.

---

## 8. `sched` — the command-line interface

Nothing populates the database on its own. There is no watcher, no polling, no
background ingest, and no listening socket. The only non-interactive way in is a
CLI the user or an agent runs deliberately.

This replaces an earlier design in which Claude Cowork wrote a Canvas snapshot to
a watched folder. The CLI subsumes it and is better on every axis: testable,
scriptable, usable by hand, and it removes the `notify` dependency entirely.

### Why a CLI rather than an API or a watched file

- An agent already knows how to run commands. It does not need a bespoke protocol.
- The same parser (§6) backs both the CLI and the overlay, so there is exactly one
  grammar and one test suite. A second ingest path is how the predecessor drifted.
- It is inspectable. `sched list --json` shows what the agent actually did.
- It works with the app closed.

### Commands

```
sched add "<text>"              parse per §6 and insert; prints the item as JSON
sched add "<text>" --tz <IANA>  pin the recurrence to a zone
sched list [--week|--today] [--json]
sched done "<title-prefix>" [--on <date>]
sched except <id> <date>        add an except_on entry
sched rm <id>
sched import <file.json>        bulk upsert; see below
sched export [--json]
sched stats [--tag <tag>]
```

`--json` on every read command. Human-readable output otherwise. Exit non-zero on
failure with a message on stderr, so a script can branch on it.

### `import` is upsert-keyed and idempotent

This is the load-bearing part. Bulk population is where an agent does damage,
because agents retry, and a retried `add` is a duplicate.

```jsonc
[
  {
    "external_id": "canvas:assignment:88213",   // REQUIRED, stable, caller-owned
    "title": "STAT240 problem set",
    "due_at": "2026-09-04T23:59:00-05:00",
    "estimate_min": 120,
    "tags": ["math"]
  }
]
```

`items` gains a nullable `external_id` with a `UNIQUE` index. `import` performs
`INSERT … ON CONFLICT(external_id) DO UPDATE`. Consequences:

- Running the same import twice changes nothing the second time.
- A re-scan with a corrected due date updates in place rather than adding a rival
  row.
- Items you typed yourself have `external_id = NULL` and are never touched by an
  import.

The predecessor's duplicate-task bug was `uuid::Uuid::new_v4()` plus a bare
`INSERT` on every agent call. A required, caller-owned, unique `external_id` is
the structural fix, and it is enforced by the schema rather than by asking the
agent to be careful.

`import` refuses a record without an `external_id` — it reports the index of the
offending record and imports nothing. Partial imports are worse than none, so the
whole file applies in one transaction or not at all.

### Concurrency with the running app

SQLite in **WAL mode**. The CLI writes while the overlay is open; the overlay
re-reads on show, which it does regardless. No watcher, no IPC, no daemon.

Worst case is a few seconds of staleness in an already-open window, which the next
summon clears. That is an acceptable trade for deleting an entire subsystem.

### `AGENT.md`

Ships beside the binary and is the file the user points an agent at. Contents:

1. The §6 grammar with worked examples.
2. Every command and its flags.
3. The `import` JSON shape, with `external_id` marked required and its purpose
   explained — *stable across re-scans, derived from the source's own identifier,
   never randomly generated.*
4. An explicit **check before you add** instruction: run `sched list --json`
   first, and prefer `import` over repeated `add` for anything derived from an
   external source.
5. A statement that the app never populates itself, so nothing appears unless the
   agent put it there.

### Reminders to scan are ordinary items

There is no staleness setting and no badge logic. "Go run a scan" is a recurring
item like any other:

```
update schedule sat 10:00
```

It appears on the week and nudges through Telegram exactly as anything else
scheduled does. This dogfoods the recurrence engine rather than building a
parallel notification path beside it, and the cadence is edited by editing the
item.

Seeded on first run; deletable like any item.

---

## 9. Shell and runtime

Tauri 2, Rust backend, tray-resident with a global hotkey overlay.

- Hidden by default; the Rust process alone is a few MB.
- **Open question, to be measured not guessed:** whether hiding the window is
  sufficient or the webview must be destroyed on hide and recreated on show.
  Build with plain hide first, measure the working set with a game running,
  switch only if the measurement demands it.
- No listening sockets. This also retires the predecessor's
  `dash_server.rs`, which bound `0.0.0.0:18791` unauthenticated despite a
  comment claiming loopback.

New dependencies beyond the salvaged set: `tauri-plugin-global-shortcut`
(hotkey), `clap` (the §8 CLI), and **`chrono-tz`** for IANA zone resolution.
`chrono` is already present but only handles the machine's own zone; §5.3
requires resolving times in zones the machine is not currently in.

No filesystem watcher. Replacing the Canvas snapshot ingest with the CLI removed
`notify` and `notify-debouncer-mini` entirely — the CLI writes to SQLite in WAL
mode and the overlay re-reads on show, so nothing needs to observe the disk.

Detecting the OS zone change that drives `tz:changed` is a cheap poll (once a
minute, comparing the resolved zone name) rather than an event subscription —
Windows offers a broadcast for this, but a string comparison on a one-minute
timer is simpler, has no unsafe FFI, and a minute of latency after landing is
irrelevant.

---

## 10. Salvage

New repository. Not a fork: ~1,700 of 2,389 Rust lines and 6 of 9 frontend
modules would be deleted, and a fork drags a 17,615-file vendored agent monorepo
through the history permanently. Starting fresh makes the operation *choosing*
rather than *deleting*.

| Take | Leave |
|---|---|
| `db.rs` schema and migration patterns | `openclaw/`, `docker/` |
| `tasks.rs` CRUD shape | `canvas.rs`, `gmail.rs`, `dash_server.rs` |
| `telegram.rs` long-poll transport | the LLM parsing inside it |
| `window.rs` frameless + drag region | `terminal.rs`, `music.rs`, `files.rs` |
| `styles/tokens.css` | `system.rs`, `macros.rs` |
| `tauri.conf.json` as a base | the 9-tile dashboard layout |

`dash-app` is archived in place, not modified.

---

## 11. Build order

1. Schema + migrations + `get_week` expansion, with the §5.2 catalogue as tests.
2. Parser (§6), tests first.
3. Timer + sessions + stats (§5.6).
4. `sched` CLI + `AGENT.md` (§8) — thin once 1–3 exist, since it is the same
   parser and the same store behind a different front door.
5. Tray/overlay shell and the frontend contract wired to real data.
6. Telegram transport.

Steps 1–3 are the product. 4 makes it scriptable. 5–6 are reach.

---

## 12. Open questions

None blocking implementation.

### Resolved in review

- ~~Staleness threshold for the audit nudge~~ → not a threshold; a recurring item
  (§8).
- ~~Whether commitments should be timeable~~ → moot; there is no item type (§3.1).
- ~~Whether `chrono-tz` is needed~~ → yes, required by §5.3.
- ~~Overlay hotkey~~ → `Ctrl+Alt+S`, configurable. Avoids `Alt+Space`, claimed by
  both the Windows window menu and PowerToys Run, and unlikely to collide with
  game bindings.
- ~~Whether the parser should accept a zone token (`@austin`)~~ → **no.** The zone
  is read from the system clock at creation time and never typed. No grammar
  addition.
