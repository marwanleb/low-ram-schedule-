# Handoff: Weekly Schedule + To-Do ("Tide Table")

## Overview
A single-view, two-pane personal planner with a running timer. Pane 1 is a calendar-style
weekly schedule (7 day columns × hourly grid). Pane 2 is a to-do list in derived
Today / This week / Someday buckets. Clicking either pane smoothly expands it and minimizes
the other — with no brightness or opacity change. Behind both panes sits an animated
pixel-art marine scene that follows the real time of day and pauses when the window is
hidden.

The app's reason to exist is **recurring commitments + time tracking against estimates**:
classes and blocks that repeat weekly, tasks that may or may not have a time slot, and one
timer running at a time across both panes.

Aesthetic reference: pixel-art, marine, "Dave the Diver"-adjacent, but legibility and
cleanliness take priority over pixel gimmickry. Palette and type derive from the user's
existing "Spend Punchcard" dashboard (`theme_reference.png`).

## About the Design Files
The files in this bundle are **design references created in HTML** — prototypes showing
intended look and behavior, not production code to copy directly. The task is to
**recreate these designs in the target codebase's existing environment** (React, Vue,
SwiftUI, native, etc.) using its established patterns, component library and styling
approach. If no environment exists yet, pick the most appropriate framework and implement
there.

The prototypes are authored as self-contained HTML "design components": markup plus a small
logic class. Read them for exact values and behavior; do not ship them as-is.

## Fidelity
**High-fidelity.** Colors, typography, spacing, motion timings and interaction behavior are
final and intentional. One deliberate placeholder: the pixel display font (see Typography).

---

## THE DATA MODEL (read this first)

There is **one record type**. An earlier draft had separate Event and Task objects; that was
wrong and has been collapsed. An *item* is a thing to do that **may or may not have a time
slot**. The grid tile and the list row are two views of one item — checking it in either
place checks it everywhere, and a scheduled item keeps its estimate, category and timer
history.

```ts
type Category = 'work' | 'life' | 'body' | 'social';   // closed set, drives colour only

interface Item {
  id: string;
  title: string;
  cat: Category;
  tags: string[];        // open-ended, for filtering; no colours
  listed: boolean;        // ONLY controls appearance in the To Do pane
  done: boolean;
  due: ISODate | null;    // 'YYYY-MM-DD'; drives bucketing and the day tag
  start: number | null;   // hour of day, 0.5 steps (9.5 = 9:30); null = no time slot
  dur: number;            // hours, 0.5 steps
  est: number | null;     // estimate in hours
  spent: number;          // banked seconds, excluding a currently-running timer
  tz: 'system' | IanaZone;  // IANA name (e.g. 'America/Chicago') = pinned;
                            // 'system' = follow the machine. NEVER an abbreviation.
  repeat: null | { days: number[];      // 0=Mon … 6=Sun
                   except: ISODate[] }; // "except on these dates", empty by default
  overrides: Record<ISODate, { moved?: ISODate; start?: number; dur?: number }>;
}
```

**`listed` vs having a time.** Classes and work blocks are `listed: false` — they own time
on the week but must not clutter the to-do list. To-dos are `listed: true`. Both appear on
the grid if they have a `start`. Exposed as a "SHOW IN TO DO" toggle in the item detail view.

### Zones are IANA names, never abbreviations

`tz` stores a full IANA zone (`America/Chicago`, `Europe/Paris`, `Asia/Beirut`) or the
literal `'system'`. An earlier draft used `'CT' | 'ET' | 'PT'`, which was wrong twice over:
it covered only US zones, and abbreviations carry no daylight-saving rules.

That second point is not pedantry. The user moves between Austin, France and Lebanon, and
the three regions change DST on different dates, so the offsets between them are not
constant:

```
Austin → Paris,  14 Mar 2026 :  6 hours
Austin → Paris,  14 Jun 2026 :  7 hours
```

Anything derived from a fixed abbreviation is an hour wrong for several weeks each year.
Every conversion must resolve through the zone.

**Display is separate from storage.** `zLabel()` renders the last path segment in caps
(`America/Chicago` → `CHICAGO`) for chips and tiles, where a full path will not fit; the
grid tile's zone dot carries the full name in its `title`. Never persist the short label.

The prototype seeds three zones so the chip row stays demonstrable. Production should treat
`tz` as a free field, offering recently-used zones as chips.

**Occurrence expansion.** For each date in the displayed week:
- If `item.repeat`: skip if the date is in `repeat.except`; if `overrides[date].moved`
  exists, emit the occurrence on *that* date instead; otherwise emit when the weekday index
  is in `repeat.days`. Per-occurrence `start`/`dur` overrides win over the item's.
- Else: emit once if `item.due` is that date and `start !== null`.
An occurrence is `{ item, iso, srcIso, start, dur, rep }` — `srcIso` is the date the rule
generated it from, which is what per-occurrence edits key off.

**Buckets are derived, never stored.** From `due` against the real today:
no `due` → **Someday**; `due <= today` → **Today** (so overdue folds in, it never goes
stale); `due` within the current real week → **This week**; later than that → **Someday**.

**Backend contract.** Suggested: `GET /week?start=<ISO>` → `{ items, warnings }`;
`GET /items`; `POST /items`; `PATCH /items/:id`; `DELETE /items/:id`.
Series vs occurrence edits are both `PATCH`es on the item: a series edit changes
`repeat`/`start`/`dur`; an occurrence edit changes `repeat.except` or `overrides[date]`.
For real persistence `start` should become a datetime in the item's zone; the prototype
keeps an hour number for legibility.

---

## Screens / Views

One screen, plus two overlays (item detail popover, timezone review modal).

### App shell
Full viewport (`100vh`), `display:flex; flex-direction:column`, padding `18px 20px 20px`,
`gap:12px`, `overflow:hidden`, `position:relative`. Children in paint order:
1. Background scene — `position:absolute; inset:0; z-index:0`.
2. `<header>` — `position:relative; z-index:60` (**must** out-stack the panes or the week
   dropdown renders behind them), `flex:none`, `justify-content:space-between`, `gap:20px`.
3. Diagnostics strip — `z-index:20`, `flex:none`, only when warnings exist.
4. `<main>` — `position:relative; z-index:1; flex:1; min-height:0; display:flex; gap:14px`.

Base text `#e9ecf1`; body font Inconsolata.

### Header
- **Week title**: `WEEK OF AUG 24`. Pixel display font `19px`, `letter-spacing:.02em`,
  `#F5B942`, `text-shadow: 0 2px 0 rgba(8,14,26,.55)` (required — the sky behind it is light).
- **Zone chip** beside it: current zone abbreviation (`CT`), pixel font `9px`,
  padding `4px 7px`, `background: rgba(12,19,34,.6)`, `border:1px solid rgba(207,234,246,.28)`,
  `#bfe9f7`. Click reopens the timezone review.
- **Nav cluster**: `position:relative; display:flex; gap:8px` — `<<`, `TODAY ▾`, `>>`.
  Pixel font `11px`, `letter-spacing:.1em`, padding `8px 12px` (`8px 14px` for TODAY),
  `background: rgba(12,19,34,.72)`, `border:2px solid rgba(207,234,246,.3)`, `#e2f1fa`.
  Notched corners:
  `clip-path: polygon(4px 0,100% 0,100% calc(100% - 4px),calc(100% - 4px) 100%,0 100%,0 4px)`.
  Hover `border-color:#7fd0ec; color:#e9ecf1`. TODAY while open:
  `background: rgba(20,44,64,.95)`, `border-color:#7fd0ec`.

### Week picker (anchored to TODAY)
`position:absolute; top:calc(100% + 8px); right:0; z-index:40`, `width:250px`,
`max-height:330px; overflow:auto`, `background: rgba(12,16,24,.97)`,
`border:2px solid #2f4a5e`, `box-shadow: 0 10px 0 rgba(6,10,18,.45)`, `padding:6px`.
Heading `JUMP TO WEEK` (pixel `9px`, `.14em`, `#F5B942`). 33 rows, offsets −4…+28 weeks:
full-width buttons, `padding:7px 9px`, `border-left:3px solid`, date range at `12.5px`
(`#aab3c6`, selected `#e9ecf1`) and a relative label at pixel `8.5px`
(`THIS WEEK`/`NEXT WEEK`/`LAST WEEK`/`+N WKS`/`-N WKS`; `#F5B942` for the real current
week else `#59637a`). Selected row `background:#16283a`, left border `#7fd0ec`; the real
current week's left border is `#F5B942`. Hover `#1b2231`.

### Diagnostics strip (feature 6)
Hidden when there are no warnings. One row per warning: `padding:7px 12px`,
`background: rgba(37,32,20,.9)`, `border:1px solid #4a4334`, `border-left:3px solid #F5B942`.
A `NOTE` label (pixel `8.5px`, `.12em`, `#F5B942`), the message (`12px`, `#d8cfb4`,
`text-wrap:pretty`) and a `×` dismiss (`1px` border `#4a4334`, `#a99e80`; hover `#F5B942`).
Tone: visible, not alarming — no red, no icon. Backend returns these alongside the week;
they replace silently dropping unrenderable data.

### Pane 1 — Schedule
- **Container**: `flex: 1 1 <64%|38%>`, `min-width:0`, column flex,
  `background: rgba(16,20,27,.9)`, `backdrop-filter: blur(3px)`,
  `border:2px solid` (`#2f4a5e` focused / `#1f2530` not),
  `clip-path: polygon(8px 0,100% 0,100% calc(100% - 8px),calc(100% - 8px) 100%,0 100%,0 8px)`,
  `transition: flex-basis .5s cubic-bezier(.45,0,.2,1), border-color .3s`, `cursor:pointer`.
- **Pane header**: `padding:14px 16px 12px`, `border-bottom:2px solid #1f2530`,
  `background: rgba(10,13,20,.72)`. An `8×8px` `#7fd0ec` square, `SCHEDULE`
  (pixel `12px`, `.12em`), and `"17 blocks"` (`12px`, `#6b7280`) = occurrence count.
- **Day header row**: `padding-left:12px`, `padding-right: 12px + scrollbarWidth`
  (measured — see Interactions), `border-bottom:2px solid #1f2530`,
  `background: rgba(10,13,20,.72)`. Each day: `flex:<1|1.5>`, `min-width:0`, column flex,
  `gap:3px`, `padding:9px 2px 8px`, `border-left:1px solid transparent` (matches the grid
  column divider), `border-bottom:3px solid <underline>`,
  `transition: flex-grow .32s cubic-bezier(.4,0,.2,1), background-color .3s`.
  Weekday label pixel `10px` `.08em`; date number `13px`/600.

  | state | background | underline | label | date |
  |---|---|---|---|---|
  | selected (pinned filter) | `#0f3a5e` | `#7fd0ec` | `#bfe9f7` | `#e9ecf1` |
  | today | `rgba(245,185,66,.055)` | — | `#F5B942` | `#FFD98A` |
  | hover-held ≥1s | `rgba(207,234,246,.06)` | — | `#dff0fa` | `#f2f8fc` |
  | weekend | — | — | `#59637a` | `#6b7280` |
  | default | — | — | `#98a0ad` | `#c6ccd6` |

- **Grid scroller**: `flex:1; min-height:0; overflow:auto`, `padding:0 12px 16px`. Inside, a
  flex row: hour gutter (`52px`) + 7 columns.
- **Hour gutter cell**: height = that row's height, `11px` (`9px` collapsed),
  `line-height:1`, `.04em`, `#59637a` (`#39404f` collapsed), right-aligned,
  `padding-right:9px`, `transition: height .34s cubic-bezier(.4,0,.2,1), font-size .34s, color .34s`.
  Labels `8 AM`…`8 PM`, compact `8a`/`8p` when collapsed.
- **Day column**: `flex:<1|1.5>`, `min-width:0`, `position:relative`, height = Σ row heights,
  `border-left:1px solid #1c2029`, background `rgba(245,185,66,.055)` today /
  `rgba(207,234,246,.045)` hover-held / transparent, column flex,
  `transition: height .34s, flex-grow .32s cubic-bezier(.4,0,.2,1), background-color .3s`.
  Contains one spacer per hour row (`border-bottom:1px solid #1a1e27`), then absolutely
  positioned tiles.
- **Event tile** (one per occurrence): `position:absolute; left:2px`,
  `width: calc(100% - 5px)` at rest, `top`/`height` from the row-offset table (fractional
  hours interpolate within a row), `background` = category colour at 13% alpha,
  `border-left:4px solid <category>`, `padding:5px 6px`, `overflow:hidden`, column flex,
  `gap:2px`, `z-index:1`,
  `transition: width .24s cubic-bezier(.4,0,.2,1), box-shadow .24s, background-color .24s,
  top .34s cubic-bezier(.4,0,.2,1), height .34s cubic-bezier(.4,0,.2,1)`.
  - **Row 1**: optional running marker (`6px` `#F5B942` square,
    `animation: ttpulse 1.4s steps(2,end) infinite` — a 2-step pixel blink, not a fade),
    then the title (`flex:1; min-width:0`, `12px`/600, `line-height:1.2`, `#e9ecf1`,
    ellipsized; `#6b7280` + `line-through` when done).
  - **Row 2**: the time (`10.5px`, `.04em`, category colour, ellipsized, `flex:0 1 auto`)
    — format `10a–12p`, `12p–1:30p`.
  - **Density rule (important).** A tile in an unfocused column is ~41px of inner width, so
    at rest it shows *only* title + time, with a `4px` dot for a pinned zone (category
    colour) and a `4px` dot for an estimate (coral if over). The ↻ repeat glyph, the ▶/■
    timer control and the full zone/estimate chips appear **only in the hover-expanded
    state** — except on a running item, whose ■ control and marker stay visible at rest so
    it can always be stopped.
  - **Hover**: `width: max(calc(100% - 5px), measuredTitleWidth + 46 if est + 34 if zone)`,
    `z-index:30`, background = the same tint composited opaque over `rgb(20,23,29)`,
    `box-shadow: 0 0 0 1px rgba(245,185,66,.45), 0 0 16px rgba(245,185,66,.3)` (gold glow,
    deliberately not a black shadow). **Height never changes on hover — width only.**
  - Click opens the item detail popover.
- **Drop ghost** while dragging over the grid: `left:2px; right:2px`, top/height of the
  hovered hour cell, `border:2px dashed #7fd0ec`, `background: rgba(127,208,236,.1)`.

### Pane 2 — To Do
- **Container**: same treatment; `flex: 1 1 <36%|62%>`; focused border `#4a4334`.
- **Pane header**: `8×8px` `#FFCF95` square, `TO DO`, and `"8 open"` — count of listed,
  undone items.
- **Filter bar** (feature: day linkage). Appears when a day is pinned *or* peeked:
  `padding:9px 16px`, `border-bottom:2px solid #1f2530`.
  - Pinned (clicked): `background:#0f3a5e`, label `FILTERED · WED 3` (`#bfe9f7`), plus a
    `CLEAR` button (pixel `9px`, `border:2px solid #2f6c96`, `#bfe9f7`).
  - Peeked (hover-held): `background: rgba(20,32,45,.9)`, label `PEEKING · WED 3`
    (`#8fbcd6`), **no** CLEAR — it evaporates when the pointer leaves.

- **Body**: `flex:1; min-height:0; overflow:auto`, `padding:6px 14px 18px`.
- **Bucket**: column flex, `gap:1px`, `padding-top:14px`. Header row: label (pixel `10px`,
  `.12em` — `TODAY` `#FFCF95`, `THIS WEEK` `#7fd0ec`, `SOMEDAY` `#8b95ab`), a `flex:1`
  rule (`height:2px; background:#1f2530`), then `"3 open"` (`11px`, `#59637a`). Labels stay
  on one line.
- **Task row**: `display:flex; align-items:flex-start; gap:11px`, `padding:9px 8px`,
  `background:#171b22` (`#12151b` done), `border-left:3px solid` category colour
  (`#2a3140` done), `cursor:grab`, `touch-action:none`; hover `#1b2029`. Contents:
  - Checkbox: `16px` button, `border:2px solid #39404f` (border+fill become the category
    colour when done), inner `6px` square (`#0c0e12` when done).
  - Optional running marker (`6px` gold pulsing square).
  - Text: `13.5px`, `line-height:1.35`, `#e9ecf1`; done → `#59637a` + `line-through`.
    `text-wrap:pretty`.
  - Chip group (`flex:none`, `gap:5px`): estimate chip, day chip, timer button.
    - **Estimate chip**: pixel `9px`, `padding:3px 5px`, `background:#10141c`,
      `border:1px solid #39404f`, `#98a0ad`; reads `~2h` / `~30m`. **Once elapsed time
      passes the estimate it reads `OVER`** with `background: rgba(255,138,118,.16)`,
      border and text `#FF8A76` — a colour shift, no alarm.
    - **Day chip**: `TODAY` or `WED`, same chip geometry, `background:#10141c`,
      border+text = category colour.
    - **Timer button**: `17px` square, `1px` category border, `▶` (`■` running, filled with
      the category colour, glyph `#0c0e12`); hover fills.
- **Empty bucket** under a filter: `nothing here`, `12px`, `#4d5669`, `padding:10px 11px`.
- **Compose row** (replaces the add button while open): `padding:9px`, `background:#10141c`,
  `border:2px solid #2f4a5e`, column flex `gap:7px`. A borderless text input
  (`13.5px`, placeholder `what needs doing?`), then a `DAY` chip row —
  `NONE` + `M1 T2 W3 T4 F5 S6 S7` (pixel `9px`, `padding:4px 6px`; selected
  `background: rgba(127,208,236,.16)`, `border:1px solid #7fd0ec`, `#bfe9f7`; else
  `1px #2a3140`, `#6b7280`) — then `CANCEL` (ghost) and `ADD`
  (`background: rgba(245,185,66,.16)`, `border:1px solid #F5B942`, `#FFD98A`; hover fills
  gold with `#0c0e12` text). Enter submits, Escape cancels. The day defaults to the peeked/
  pinned day, else today; `SOMEDAY` defaults to `NONE`.
- **Add row** (when not composing): `+ add task`, left-aligned, `padding:8px 11px`,
  `border:2px dashed #232936`, `#4d5669`, `12px`, `.06em`; hover
  `border-color:#39404f; color:#8b95ab`.
- **Footer — swaps** (feature 2). Idle: the hint
  `drag a task onto the schedule to block time for it` (`padding:10px 16px`,
  `border-top:2px solid #1f2530`, `background: rgba(10,13,20,.72)`, `11px`, `#4d5669`).
  Running: the timer bar — `padding:10px 14px`, `border-top:2px solid #4a4334`,
  `background: rgba(30,26,16,.92)`; an `8px` gold pulsing square, the item title
  (`12.5px`, ellipsized), the elapsed clock (pixel `13px`, `.06em`,
  `font-variant-numeric: tabular-nums`, `#FFD98A` → `#FF8A76` when over) in `H:MM:SS`
  (`1:47:22`), the estimate comparison (pixel `8.5px` — `OF ~2H` / `OVER ~2H` /
  `NO ESTIMATE`), and a `STOP` button (`border:2px solid #F5B942`, `#FFD98A`; hover fills).

### Drag ghost
`position:fixed`, left/top = cursor, `transform: translate(12px,-50%)`,
`pointer-events:none`, `z-index:80`, `padding:8px 11px`, `background:#171b22`,
`border:2px solid` category colour, `box-shadow: 4px 4px 0 rgba(0,0,0,.5)`; an `8px`
category square + the title (`13px`, nowrap).

### Item detail popover (feature 4's edit surface)
Opens on a grid-tile click, or on a task-row click that didn't become a drag. A full-screen
`z-index:90` click-catcher (transparent) closes it; the panel itself swallows clicks.
Panel: `position:absolute` at the clamped pointer position, `width:318px`,
`max-height:78vh; overflow:auto`, `background: rgba(12,16,24,.98)`,
`border:2px solid #2f4a5e`, `box-shadow: 0 10px 0 rgba(6,10,18,.5)`, `padding:14px`,
column flex `gap:12px`. Sections, in order:
1. **Identity** — kind line (pixel `8.5px`, category colour):
   `REPEATING · WORK · CLASS`; title (`15px`/600); when-line (`11.5px`, `#8b95ab`):
   `SEP 3 · 12p–1:30p CT`, or `due SEP 4 · no time set`, or `no date, no time`.
2. **Timer** — a full-width `START TIMER` / `STOP TIMER` button
   (`border:2px solid #F5B942`; idle `rgba(245,185,66,.14)`/`#FFD98A`, running solid gold
   with `#0c0e12`) and the banked total `H:MM:SS` (pixel `10px`, `#98a0ad`).
3. **Estimate** — `−` / value / `+` steppers in 0.5h steps, 0–12h; `22px` square buttons,
   `background:#171b22`, `1px #39404f`; value shows `~2h` or `none`.
4. **Show in To Do** — the `listed` toggle: a `44×22px` track (`rgba(245,185,66,.22)` on /
   `#171b22` off, border `#F5B942`/`#39404f`) with a `16px` knob pushed by
   `justify-content`.
5. **Time zone** — chips for `SYS` plus the recently-used zones (pixel `8.5px`; selected
   `rgba(127,208,236,.16)` + `#7fd0ec`; the row wraps). Chips show a short city label
   (`CHICAGO`, `PARIS`, `BEIRUT`) but the **stored value is the full IANA name**. The
   prototype seeds three zones; production should treat this as a free field with recent
   zones surfaced as chips. See "Zones are IANA names" below.
6. **Repeats weekly** — a `TURN ON`/`TURN OFF` button, then seven `M T W T F S S` weekday
   toggles (`flex:1`; on = `rgba(245,185,66,.18)` + `#F5B942` + `#FFD98A`), then the time
   range: `−`/`+` on start (0.5h steps) plus `−LEN`/`+LEN` on duration (0.5–8h), and the
   **except-on-these-dates** list — heading reads
   `EXCEPT ON THESE DATES — NONE YET` when empty (it is empty by default), otherwise chips
   (`SEP 10 ×`, `background:#171b22`, `1px #39404f`) whose `×` removes the exception.
   Turning repeat on seeds `days` with the occurrence's weekday and gives an untimed item a
   9:00 start.
7. **This occurrence only** — heading `THIS OCCURRENCE ONLY · SEP 3` (or `THIS ITEM` for a
   one-off). `CANCEL THIS ONE` (adds that date to `except`; for a one-off it just clears the
   time slot), and `MOVE` with `‹` `›` shifting that single occurrence by a day via
   `overrides[date].moved` (a one-off moves its `due`). These are visually separate from:
8. **Destructive** — a single `DELETE WHOLE SERIES…` (or `DELETE ITEM…`) button, ghost-styled
   (`1px #2a3140`, `#8b95ab`) that requires a second click:
   `CONFIRM · DELETE WHOLE SERIES` in coral (`rgba(255,138,118,.16)`, `#FF8A76`).
   **Deleting one occurrence and deleting the series are different controls in different
   sections; the per-occurrence action never offers to delete the series.**

### Timezone review modal (feature 5)
Shown once after the system zone changes (the prototype persists the last-seen zone in
`localStorage['tt.zone']`; the header zone chip reopens it on demand).
Backdrop `rgba(6,9,15,.72)`, centred panel `460px`, `max-height:80vh`,
`background: rgba(12,16,24,.99)`, `border:2px solid #2f4a5e`,
`box-shadow: 0 12px 0 rgba(6,10,18,.5)`, `clip-path` with `10px` notches, `padding:20px`.
Eyebrow `TIME ZONE CHANGED` (pixel `9px`, `#F5B942`), headline `You're now on BEIRUT` (short city label of the IANA zone)
(pixel `15px`), explainer (`12.5px`, `#98a0ad`). Then one row per **recurring** item:
`padding:10px`, `background:#171b22`, `border-left:3px solid` (`#F5B942` pinned /
`#7fd0ec` system); title (`13px`) over a sub-line (`11px`, `#6b7280`) reading
`pinned to CT · 12p–1:30p CT` or `system time · 9a–10a wherever you are`; and a one-tap
toggle button showing `PINNED CT` or `SYSTEM`. `DONE` closes.

### Background scene (Sky)
A `<canvas>` filling the shell, drawn at 1/7 scale (`PX = 7` CSS px per art pixel) with
`image-rendering: pixelated`, re-rasterized on resize. All pixel art drawn with 1×1 fills —
**no SVG, no bitmap assets.**
- Sky above the waterline (`waterline: 0.78` of height in the app), ocean below, warm sand
  seabed in the bottom ~20% of the water with an undulating contour, a lit rim and pebbles.
- **Time of day**: zenith / horizon / light colours interpolate across keyframes at hours
  0, 4.5, 6.2, 8, 12, 16.5, 19, 20.6, 22, 24 (exact hexes in `Sky.dc.html`). Gradients are
  rendered with a **Bayer 4×4 ordered dither** between adjacent palette steps — that is what
  makes the fade look pixelated. Sky 8 steps, water 14 (fewer reads as a checkerboard),
  sand 7.
- **Sun** arcs 6:00→20:00 on a sine altitude curve with a dithered radial glow that scales
  with altitude, plus a shimmer column on the water. **Moon** is a 13px pixel disc with
  craters, a shaded limb and a faint halo; clouds are suppressed within 17px of it.
  **Stars** fade in after ~21:30, out by ~06:12, twinkling per-star.
- **Clouds** drift right at 0.16–0.38 px/frame and wrap, tinted toward the light colour at
  dawn/dusk. **Waterline** crest from two summed sines with foam flecks and drifting current
  dashes. **Reef**: swaying mound and branch corals in gold `#F5B942`, violet `#B57BE0`,
  coral `#E8604C`, green `#6FD08C` families. **Fish**: 5–7px bodies with wiggling tails in
  `#FFCF95 #7FD0EC #FF8A76 #C79BF0 #8FD6A8 #F5B942`.
- **Frame loop** ~3.3 fps (`setTimeout(…,300)` + `requestAnimationFrame`) — deliberately low
  so motion reads as pixel animation. The static raster re-bakes every 20 frames (≈6s).
- **Must pause when hidden.** The app lives in the tray: on `visibilitychange` the loop is
  cancelled outright (no timer pending) and resumes with a fresh bake when visible. Do not
  let this run against a game.
- Rebuild the scene **only** when seed or waterline change; other re-renders re-bake the
  palette only, or clouds/fish/coral snap back to their start positions.

---

## Interactions & Behavior

1. **Pane focus.** Clicking anywhere in a pane focuses it: schedule `64%`/to-do `36%`, or
   `38%`/`62%`. `transition: flex-basis .5s cubic-bezier(.45,0,.2,1)`. The only other change
   is border colour — **no opacity, brightness or filter change**, an explicit requirement.
2. **Day peek and pin.** Pointer entering a day header *or* column starts a 1000ms timer;
   on fire that day becomes the wide column (`flex-grow:1.5`) **and the To Do pane filters to
   that day's items** (`PEEKING`). Leaving clears it. Clicking the day pins the filter
   (`FILTERED`, CLEAR to release) and focuses the to-do pane. Exactly one column is wide:
   the peeked day if any, else today.
3. **Drag an item onto the grid.** `pointerdown` on a task row (ignoring the checkbox and
   the timer button) arms a drag; movement past ~5px starts it (below that, `pointerup`
   opens the detail popover instead). `pointermove` moves the ghost and resolves the hovered
   cell; `pointerup` **sets that item's `due` and `start`** — it does not create a second
   record. Dropping outside cancels.
   Cell resolution must read the real column rects (`getBoundingClientRect()` per column) —
   columns are unequal because of the 1.5× day — and the hour comes from the row-offset
   table, not a fixed row height.
4. **Greedy vertical space.** An hour with no occurrence on any day of the week collapses
   from `46px` to `17px`; its gutter label shrinks, dims and switches to the compact form.
   Row heights, tile `top`/`height` and label size animate over `.34s`. A half-hour item
   marks both hours it touches as occupied, and tile geometry interpolates *within* a row for
   fractional starts/durations.
5. **Tile hover.** Width only, to `max(columnWidth, titleWidth + chip allowance)`; tiles whose
   title already fits don't move. Title width is measured with canvas `measureText` at
   `600 12px Inconsolata`, re-measured once `document.fonts.ready` resolves. Reveals the
   ↻ glyph, the timer control and the chips (see the density rule above).
6. **Timer.** One runs at a time, startable from a tile, a task row, or the detail popover.
   Starting a second timer banks the first's elapsed seconds into its `spent` and switches.
   Stopping banks and clears. The footer swaps to the running bar; the running item shows a
   pulsing gold marker in **both** panes. Elapsed = `spent + (now − startedAt)`, ticking at
   1s **only while a timer runs** (no idle re-render).
7. **Estimates.** `elapsed > est` flips the chip to `OVER` and the running clock to coral.
8. **Week navigation.** `<<`/`>>` step by ∓1 week; `TODAY ▾` opens the picker; picking sets
   the offset. Clicking either pane closes the picker.
9. **Scrollbar-aware alignment.** The day header row's right padding is
   `12px + (scroller.offsetWidth − scroller.clientWidth)`, measured on mount and resize.
10. **Not implemented.** Editing a title in place; resizing a tile by dragging its edge;
    reordering within a bucket; monthly/interval recurrence (weekly only); real persistence;
    real backend warnings (seeded in state); actual timezone
    conversion of `start` (the prototype marks zones, it doesn't shift clock times).

## State Management

```
active      : 'schedule' | 'todos'      // which pane is expanded
sel         : 0..6 | null               // pinned day filter
hoverDay    : 0..6 | null               // peeked day (1s hover)
hoverEv     : '<itemId>@<iso>' | null   // hovered occurrence
week        : integer offset            // 0 = current real week
picker      : boolean
drag        : { id, text, cat, x, y } | null
hover       : { day, hour } | null      // drop target
detail      : { id, iso, x, y } | null  // open popover + which occurrence
confirmDel  : itemId | null             // two-step delete
compose     : bucketId | null
composeText : string
composeDay  : 0..6 | null
timer       : { id, startedAt } | null  // ONE timer
now         : epoch ms                  // ticked 1/s only while a timer runs
sbw         : number                    // measured scrollbar width
tzReview    : boolean
warnings    : { id, text }[]            // from the backend
items       : Item[]
```

**Derived per render, never stored**: occurrences for the displayed week, the row
height/offset table, tile geometry, buckets, counts, the week's dates, the 33 picker rows,
elapsed times.

## Design Tokens

**Surfaces**
| token | value |
|---|---|
| page | `#0c0e12` |
| pane | `rgba(16,20,27,.9)` + `backdrop-filter: blur(3px)` |
| pane sub-bar | `rgba(10,13,20,.72)` |
| task row | `#171b22` (done `#12151b`) |
| popover / dropdown | `rgba(12,16,24,.97–.99)` |
| pinned day / filter bar | `#0f3a5e` |
| peek filter bar | `rgba(20,32,45,.9)` |
| diagnostics strip | `rgba(37,32,20,.9)` |
| timer bar | `rgba(30,26,16,.92)` |
| modal scrim | `rgba(6,9,15,.72)` |

**Lines** `#1f2530` (2px pane dividers) · `#1c2029` (column) · `#1a1e27` (hour) ·
`#232936` (dashed add-row) · `#2a3140` (ghost buttons) · `#39404f` (inputs, steppers) ·
`#2f4a5e` (schedule focus / popovers) · `#4a4334` (to-do focus / diagnostics / timer bar) ·
`rgba(207,234,246,.3)` (header buttons)

**Text** `#e9ecf1` primary · `#d8cfb4` on-diagnostics · `#c6ccd6` secondary ·
`#aab3c6` tertiary · `#98a0ad` muted · `#8b95ab` muted-2 · `#6b7280` dim · `#59637a` dimmer ·
`#4d5669` faintest · `#e2f1fa` on-sky · `#bfe9f7` on-blue

**Accents**
| name | hex | used for |
|---|---|---|
| gold (incandescent) | `#F5B942` | today, week title, timer, repeat controls, diagnostics, hover glow |
| gold light | `#FFD98A` | today's date, timer clock, gold button text |
| sand | `#FFCF95` | to-do pane mark, `TODAY` bucket, `life` category |
| cyan | `#7FD0EC` | schedule mark, `THIS WEEK` bucket, selection, `work` |
| coral | `#FF8A76` | `OVER` estimate, destructive confirm, `social` category |
| green | `#A8D98F` | `body` category |
| violet | `#C79BF0` | background art only |

Tiles use their category colour at `.13` alpha; the hover state composites the same colour
opaque over `rgb(20,23,29)`.

**Typography**
- Body / data: **Inconsolata** 400/500/600/700 (Google Fonts).
- Display / labels: **Silkscreen** 400/700 (Google Fonts) — a *stand-in*. The user's own
  dashboard uses a custom pixel face called **"Flamingo"**; substitute it everywhere
  Silkscreen appears if you have it.
- Scale: 19 / 15 / 13.5 / 13 / 12.5 / 12 / 11.5 / 11 / 10.5 / 10 / 9.5 / 9 / 8.5 / 8 px.
  Pixel-font labels are uppercase, `letter-spacing` .06–.16em. Numeric clocks use
  `font-variant-numeric: tabular-nums`.

**Geometry & motion**
- Radius **0 everywhere**; "rounded" corners are notched via `clip-path` polygons — 10px on
  the modal, 8px on panes, 4px on header buttons.
- Row height `46px` normal / `17px` collapsed; hour gutter `52px`; shell padding
  `18px 20px 20px`; pane gap `14px`.
- Easings: `cubic-bezier(.45,0,.2,1)` pane resize (.5s); `cubic-bezier(.4,0,.2,1)` row
  collapse (.34s) and column widen (.32s); `.24s` tile hover; `.3s` colour changes.
- `@keyframes ttpulse` — `opacity 1 → .25 → 1`, `1.4s steps(2,end) infinite`. Stepped, so it
  blinks like a sprite rather than fading.
- Shadows: only the gold hover glow and hard pixel offsets
  `4px 4px 0 rgba(0,0,0,.5)` (drag ghost), `0 10px 0 rgba(6,10,18,.45–.5)` (dropdown,
  popover), `0 12px 0` (modal). No soft ambient shadows.

## Assets
None. No images, icons or SVG — every graphic is a CSS box or procedurally drawn on the
background canvas. Glyphs used are plain text characters: `↻ ▶ ■ ‹ › ▾ × − —`.
Fonts load from Google Fonts. Sample items and warnings are placeholder content.

## Files
| file | what it is |
|---|---|
| `Tide Table.dc.html` | The app: both panes, header, week picker, diagnostics, detail popover, timezone modal, timer, drag/drop. Markup is the template; the logic class holds state, sample data and all derived values. |
| `Sky.dc.html` | The background scene. Props: `seed`, `waterline`, `liveTime`, `hour`, `dayCycle`. |
| `support.js` | Runtime needed to open the two HTML files locally. Not part of the design. |
| `theme_reference.png` | The user's existing dashboard — origin of the palette, notched corners and pixel type. |

Open either HTML file directly in a browser to see it running.
