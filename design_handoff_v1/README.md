# Handoff: Weekly Schedule + To-Do ("Tide Table")

## Overview
A single-view, two-pane personal planner. Pane 1 is a calendar-style weekly schedule
(7 day columns × hourly time grid). Pane 2 is a to-do list bucketed into
Today / This week / Someday. Clicking either pane smoothly expands it and minimizes the
other — with no brightness or opacity change. Behind both panes sits an animated pixel-art
marine scene (sky + ocean) that follows the real time of day.

Aesthetic reference: pixel-art, marine, "Dave the Diver"-adjacent, but legibility and
cleanliness take priority over pixel gimmickry. Palette and type were derived from the
user's existing "Spend Punchcard" dashboard.

## About the Design Files
The files in this bundle are **design references created in HTML** — prototypes showing
intended look and behavior, not production code to copy directly. The task is to
**recreate these designs in the target codebase's existing environment** (React, Vue,
SwiftUI, native, etc.) using its established patterns, component library, and styling
approach. If no environment exists yet, pick the most appropriate framework for the
project and implement there.

The prototypes are authored as self-contained HTML "design components": markup plus a
small logic class. Read them for exact values and behavior; do not ship them as-is.

## Fidelity
**High-fidelity.** Colors, typography, spacing, motion timings, and interaction behavior
are final and intentional. Recreate pixel-faithfully with the codebase's own primitives.
The one deliberate placeholder: the pixel display font (see Typography).

---

## Screens / Views

There is exactly one screen.

### App shell
- **Purpose**: See the week and the task list at once; shift emphasis between them.
- **Layout**: Full viewport (`100vh`), `display:flex; flex-direction:column`,
  padding `18px 20px 20px`, `gap:14px`, `overflow:hidden`, `position:relative`.
  Children, in paint order:
  1. Background scene, `position:absolute; inset:0; z-index:0`.
  2. `<header>`, `position:relative; z-index:60` (must out-stack the panes so the week
     dropdown renders above them), `flex:none`, `display:flex`, `align-items:flex-end`,
     `justify-content:space-between`, `gap:20px`.
  3. `<main>`, `position:relative; z-index:1; flex:1; min-height:0; display:flex; gap:14px`.
- **Base text color** `#e9ecf1`; body font Inconsolata.

### Header
- **Week title**: text `WEEK OF AUG 24` (month abbrev + date of that week's Monday).
  Pixel display font, `19px`, `letter-spacing:.02em`, color `#F5B942` (gold),
  `text-shadow: 0 2px 0 rgba(8,14,26,.55)` — the shadow is required because the top of the
  background is a light sky band.
- **Nav cluster**: `position:relative; display:flex; align-items:center; gap:8px`.
  Three buttons: `<<`, `TODAY ▾`, `>>`.
  - Button: pixel font `11px`, `letter-spacing:.1em`, padding `8px 12px` (`8px 14px` for
    TODAY), background `rgba(12,19,34,.72)`, border `2px solid rgba(207,234,246,.3)`,
    color `#e2f1fa`, cursor pointer.
  - Notched pixel corners via
    `clip-path: polygon(4px 0, 100% 0, 100% calc(100% - 4px), calc(100% - 4px) 100%, 0 100%, 0 4px)`.
  - Hover: `border-color:#7fd0ec; color:#e9ecf1`.
  - TODAY while its dropdown is open: background `rgba(20,44,64,.95)`, border `#7fd0ec`.

### Week picker dropdown (anchored to TODAY)
- `position:absolute; top:calc(100% + 8px); right:0; z-index:40`, width `250px`,
  `max-height:330px; overflow:auto`, background `rgba(12,16,24,.97)`,
  border `2px solid #2f4a5e`, `box-shadow: 0 10px 0 rgba(6,10,18,.45)`, padding `6px`,
  `display:flex; flex-direction:column; gap:1px`.
- Heading `JUMP TO WEEK`: pixel font `9px`, `letter-spacing:.14em`, color `#F5B942`,
  padding `7px 8px 8px`.
- Rows: 33 entries, offsets `-4` … `+28` weeks from the current week.
  Row is a full-width button, `display:flex; align-items:baseline;
  justify-content:space-between; gap:10px`, padding `7px 9px`, `border-left:3px solid`.
  - Left label: date range, e.g. `AUG 24 – AUG 30`, `12.5px`, color `#aab3c6`
    (selected `#e9ecf1`).
  - Right label: pixel font `8.5px`, `letter-spacing:.08em`, `white-space:nowrap` —
    `THIS WEEK` / `NEXT WEEK` / `LAST WEEK` / `+N WKS` / `-N WKS`.
    Color `#F5B942` for the current real week, else `#59637a`.
  - Selected row: background `#16283a`, left border `#7fd0ec`. The real current week gets
    left border `#F5B942` when not selected. Others: transparent.
  - Hover: background `#1b2231`.

### Pane 1 — Schedule
- **Container**: `flex: 1 1 <64% | 38%>`, `min-width:0`, `display:flex; flex-direction:column`,
  background `rgba(16,20,27,.9)`, `backdrop-filter: blur(3px)`,
  border `2px solid` (`#2f4a5e` when focused, `#1f2530` when not),
  `clip-path: polygon(8px 0, 100% 0, 100% calc(100% - 8px), calc(100% - 8px) 100%, 0 100%, 0 8px)`,
  `transition: flex-basis .5s cubic-bezier(.45,0,.2,1), border-color .3s`, cursor pointer.
- **Pane header**: padding `14px 16px 12px`, `border-bottom:2px solid #1f2530`,
  background `rgba(10,13,20,.72)`. Contents: an `8×8px` `#7fd0ec` square, the word
  `SCHEDULE` (pixel font `12px`, `letter-spacing:.12em`, `#e9ecf1`), and a count
  `"17 blocks"` (`12px`, `#6b7280`).
- **Day header row**: `flex:none; display:flex`, `padding-left:12px`,
  `padding-right: 12px + scrollbarWidth` (measured from the grid scroller so the columns
  line up — see Interactions), `border-bottom:2px solid #1f2530`,
  background `rgba(10,13,20,.72)`.
  Each day is a button: `flex: <1 | 1.5>`, `min-width:0`, column flex, `gap:3px`,
  padding `9px 2px 8px`, `border-left:1px solid transparent` (matches the grid column's
  1px divider), `border-bottom:3px solid <underline>`,
  `transition: flex-grow .32s cubic-bezier(.4,0,.2,1), background-color .3s`.
  - Weekday label: pixel font `10px`, `letter-spacing:.08em`.
  - Date number: `13px`, weight 600.
  - Colors:
    | state | background | underline | label | date |
    |---|---|---|---|---|
    | selected (filter active) | `#0f3a5e` | `#7fd0ec` | `#bfe9f7` | `#e9ecf1` |
    | today | `rgba(245,185,66,.055)` | none | `#F5B942` | `#FFD98A` |
    | hover-held (≥1s) | `rgba(207,234,246,.06)` | none | `#dff0fa` | `#f2f8fc` |
    | weekend | transparent | none | `#59637a` | `#6b7280` |
    | default | transparent | none | `#98a0ad` | `#c6ccd6` |
- **Grid scroller**: `flex:1; min-height:0; overflow:auto`, padding `0 12px 16px`.
  Inside, a flex row: hour gutter (`52px`, `flex:none`) + 7 day columns.
- **Hour gutter cell**: height = that row's height, `font-size:11px` (`9px` when the row is
  collapsed), `line-height:1`, `letter-spacing:.04em`, color `#59637a` (`#39404f` collapsed),
  `text-align:right`, `padding-right:9px`,
  `transition: height .34s cubic-bezier(.4,0,.2,1), font-size .34s, color .34s`.
  Label `8 AM` … `8 PM` when the row is normal, compact `8a`/`8p` when collapsed.
- **Day column**: `flex: <1 | 1.5>`, `min-width:0`, `position:relative`, height = sum of row
  heights, `border-left:1px solid #1c2029`, background `rgba(245,185,66,.055)` for today /
  `rgba(207,234,246,.045)` for the hover-held day / transparent,
  `display:flex; flex-direction:column`,
  `transition: height .34s cubic-bezier(.4,0,.2,1), flex-grow .32s cubic-bezier(.4,0,.2,1), background-color .3s`.
  It contains one spacer div per hour row (`height: rowHeight; border-bottom:1px solid #1a1e27`,
  same height transition), then absolutely-positioned event tiles.
- **Event tile**: `position:absolute; left:2px`, width `calc(100% - 5px)` at rest,
  `top`/`height` derived from the row offsets (see State), background = category color at
  13% alpha over the pane, `border-left:4px solid <categoryColor>`, padding `5px 6px`,
  `overflow:hidden`, column flex, `gap:2px`, `z-index:1`, no shadow.
  `transition: width .24s cubic-bezier(.4,0,.2,1), box-shadow .24s, background-color .24s,
  top .34s cubic-bezier(.4,0,.2,1), height .34s cubic-bezier(.4,0,.2,1)`.
  - Title span: `flex:1; min-height:0`, `12px`, weight 600, `line-height:1.2`,
    `letter-spacing:.02em`, `#e9ecf1`, `overflow:hidden; text-overflow:ellipsis; white-space:nowrap`.
  - Time span: `flex:none`, `10.5px`, `letter-spacing:.04em`, color = category color,
    `white-space:nowrap`. Format `10a–12p`.
  - Hovered: `width: max(calc(100% - 5px), <measuredTitleWidth>)`, `z-index:30`,
    background = the same tint composited to fully opaque over `rgb(20,23,29)`,
    `box-shadow: 0 0 0 1px rgba(245,185,66,.45), 0 0 16px rgba(245,185,66,.3)` (gold glow —
    explicitly not a black shadow), title `text-overflow:clip`.
    **Height never changes on hover; only width.**
- **Drop ghost** (while dragging a task over the grid): `position:absolute; left:2px; right:2px`,
  top/height of the hovered hour cell, `border:2px dashed #7fd0ec`,
  background `rgba(127,208,236,.1)`.

### Pane 2 — To Do
- **Container**: identical treatment to pane 1; `flex: 1 1 <36% | 62%>`; focused border
  `#4a4334`.
- **Pane header**: `8×8px` `#FFCF95` square, `TO DO` (pixel font `12px`, `.12em`, `#e9ecf1`),
  count `"8 open"` (`12px`, `#6b7280`).
- **Filter bar** (only when a day is selected): padding `9px 16px`, background `#0f3a5e`,
  `border-bottom:2px solid #1f2530`, `display:flex; justify-content:space-between`.
  Label `FILTERED · WED` (pixel font `10px`, `.1em`, `#bfe9f7`); `CLEAR` button
  (pixel font `9px`, `.1em`, padding `5px 9px`, transparent bg,
  `border:2px solid #2f6c96`, `#bfe9f7`).
- **Body**: `flex:1; min-height:0; overflow:auto`, padding `6px 14px 18px`.
- **Bucket** (three of them): column flex, `gap:1px`, `padding-top:14px`.
  - Bucket header: `display:flex; align-items:center; gap:9px`, padding `0 2px 8px`.
    Label pixel font `10px`, `letter-spacing:.12em` — `TODAY` `#FFCF95`,
    `THIS WEEK` `#7fd0ec`, `SOMEDAY` `#8b95ab`. Then a `flex:1` rule
    (`height:2px; background:#1f2530`), then `"3 open"` (`11px`, `#59637a`).
    Labels must stay on one line.
  - Task row: `display:flex; align-items:flex-start; gap:11px`, padding `9px 8px`,
    background `#171b22` (`#12151b` when done), `border-left:3px solid` category color
    (`#2a3140` when done), `cursor:grab`, `touch-action:none`;
    hover background `#1b2029`.
    - Checkbox: `16px` square button, `margin-top:1px`, `border:2px solid #39404f`
      (border and fill become the category color when done), centered `6px` inner square
      (`#0c0e12` when done, transparent otherwise).
    - Text: `13.5px`, `line-height:1.35`, `letter-spacing:.01em`, `#e9ecf1`;
      when done `#59637a` + `line-through`. `text-wrap:pretty`.
    - Day tag (only if the task has a day): pixel font `9px`, `letter-spacing:.06em`,
      padding `3px 5px`, background `#10141c`, `border:1px solid` category color,
      text = category color, e.g. `WED`.
  - Empty bucket (under an active filter): `"nothing here"`, `12px`, `#4d5669`,
    padding `10px 11px`.
  - Add row: `+ add task`, left-aligned, padding `8px 11px`, transparent,
    `border:2px dashed #232936`, `#4d5669`, `12px`, `letter-spacing:.06em`;
    hover `border-color:#39404f; color:#8b95ab`. **Affordance only — not wired.**
- **Footer hint**: padding `10px 16px`, `border-top:2px solid #1f2530`,
  background `rgba(10,13,20,.72)`, `11px`, `letter-spacing:.05em`, `#4d5669`,
  copy: `drag a task onto the schedule to block time for it`.

### Drag ghost (follows the cursor)
`position:fixed`, left/top = cursor, `transform: translate(12px,-50%)`,
`pointer-events:none`, `z-index:50`, `display:flex; align-items:center; gap:9px`,
padding `8px 11px`, background `#171b22`, `border:2px solid` category color,
`box-shadow: 4px 4px 0 rgba(0,0,0,.5)`; an `8px` category-color square + the task text
(`13px`, `#e9ecf1`, nowrap).

### Background scene (Sky)
A `<canvas>` filling the shell, drawn at 1/7 scale (`PX = 7` CSS px per art pixel) with
`image-rendering: pixelated`, re-rasterized on resize. Everything is pixel art drawn with
1×1 fills — **no SVG, no photographic assets.**
- **Composition**: sky above the waterline (`waterline = 0.78` of height in the app),
  ocean below, sand seabed in the bottom ~20% of the water.
- **Time of day**: zenith color, horizon color, and light color interpolate across
  keyframes at hours 0 / 4.5 / 6.2 / 8 / 12 / 16.5 / 19 / 20.6 / 22 / 24 (see `DAY` in
  `Sky.dc.html` for the exact hexes). The gradient is rendered as a **Bayer 4×4 ordered
  dither** between adjacent palette steps — that is what makes the fade look pixelated.
  Sky uses 8 steps; water uses 14 closely-spaced steps (fewer steps read as a checkerboard);
  sand uses 7.
- **Sun**: arcs 6:00→20:00 across the sky on a sine altitude curve, with a dithered radial
  glow whose radius and intensity scale with altitude, plus a shimmer column on the water
  beneath it.
- **Moon**: 13px-wide pixel disc with craters, a one-pixel shaded limb, and a faint dithered
  halo. Clouds are suppressed within 17px of it so they never bite a chunk out.
- **Stars**: fade in after ~21:30 and out by ~06:12, twinkling on a per-star sine.
- **Clouds**: 3–5 overlapping pixel lobes each, drifting right at 0.16–0.38 px/frame and
  wrapping; tinted toward the light color at dawn/dusk.
- **Waterline**: two summed sines make a crest line with foam flecks; plus slow drifting
  "current" dashes below.
- **Reef**: seabed contour from three summed sines; mound and branch corals sway on a sine,
  in gold `#F5B942`, violet `#B57BE0`, coral-red `#E8604C`, and green `#6FD08C` families
  (each with hi/mid/lo shades).
- **Fish**: 5–7px pixel bodies with wiggling tails, in `#FFCF95 #7FD0EC #FF8A76 #C79BF0
  #8FD6A8 #F5B942`, drifting horizontally with a bobbing sine and wrapping at the edges.
- **Frame loop**: ~3.3 fps (`setTimeout(…, 300)` + `requestAnimationFrame`) — deliberately
  low so motion reads as pixel animation rather than smooth CSS. The static
  sky/water/sand raster is re-baked every 20 frames (≈6s), which is often enough to follow
  the clock.

---

## Interactions & Behavior

1. **Pane focus (the headline interaction)**
   Clicking anywhere in a pane focuses it: schedule `64%`/to-do `36%`, or `38%`/`62%`
   when the to-do pane is focused. Animated with
   `transition: flex-basis .5s cubic-bezier(.45,0,.2,1)`. The only other change is the
   pane's border color. **No opacity, brightness, or filter change on either pane** — this
   was an explicit requirement.

2. **Day selection filters the to-do list**
   Clicking a day header toggles `sel`. When set, every bucket shows only tasks whose
   `day === sel`; buckets with no match show "nothing here". The filter bar appears; CLEAR
   resets. Selecting a day also focuses the to-do pane.

3. **Hover-held day widening**
   Pointer entering a day header *or* its grid column starts a 1000 ms timer; on fire, that
   day becomes the wide column (`flex-grow: 1.5`) with the cool wash. Leaving clears the
   timer and the state. Exactly one column is wide at a time: the hover-held day if there is
   one, otherwise today. Width eases via `transition: flex-grow .32s cubic-bezier(.4,0,.2,1)`.

4. **Drag a task onto the schedule**
   `pointerdown` on a task row (ignored if the target is the checkbox button) starts a
   pointer-capture drag: `pointermove` updates the floating ghost and resolves the hovered
   cell; `pointerup` commits. Dropping creates a 1-hour event at that day/hour with the
   task's title uppercased and its category color, and stamps `day` onto the task so its
   `WED`-style tag appears. Dropping outside the grid cancels.
   **Cell resolution must read the real column rects** (`getBoundingClientRect()` per column
   element) rather than dividing the width by 7 — columns are unequal because of the
   1.5× wide day. The hour comes from the row offset table, not a fixed row height.

5. **Greedy vertical space**
   Any hour with no event on *any* day of the week collapses from `46px` to `17px`, and its
   gutter label shrinks and dims and switches to the compact `8a` form. Row heights, event
   `top`/`height`, and label size all animate over `.34s`. Dropping a task into a collapsed
   hour re-expands that row.

6. **Event tile hover**
   Grows horizontally only, to `max(columnWidth, measuredTitleWidth)` — tiles whose title
   already fits do not move. Title width is measured with a canvas
   `measureText` at `600 12px Inconsolata` plus 28px of chrome, re-measured once
   `document.fonts.ready` resolves. Gold glow, opaque background, `z-index:30`, 0.24s ease.

7. **Week navigation**
   `<<`/`>>` step `week` by ∓1. `TODAY ▾` toggles the picker; picking a row sets `week` to
   that offset and closes. Clicking either pane also closes it. The header must out-stack
   the panes or the dropdown renders behind them.

8. **Checkbox toggle**
   Flips `done`; updates the bucket count and the pane's open count. `stopPropagation` so it
   doesn't also fire the row's drag or the pane focus.

9. **Scrollbar-aware alignment**
   The day header row's right padding is `12px + (scroller.offsetWidth - scroller.clientWidth)`,
   measured on mount and on resize, so day headers stay aligned with the grid columns.

10. **Not implemented (intentional stubs)**
    `+ add task`; creating/editing/deleting events other than by drop; moving or resizing
    existing events; recurrence; persistence; multi-week event data (the sample events are
    the same for every week).

## State Management

```
active     : 'schedule' | 'todos'        // which pane is expanded
sel        : 0..6 | null                 // selected weekday → filters to-dos
week       : integer offset              // 0 = the week of the reference Monday
hoverDay   : 0..6 | null                 // hover-held wide day
hoverEv    : eventId | null              // hovered tile
picker     : boolean                     // week dropdown open
drag       : { id, text, cat, x, y } | null
hover      : { day, hour } | null        // drop target cell
sbw        : number                      // measured scrollbar width
events     : Event[]
tasks      : Task[]
```

```
Event = { id: string, day: 0..6, start: hour int, dur: hours int,
          title: string, cat: Category }
Task  = { id: string, b: 'today'|'week'|'someday', text: string,
          cat: Category, day: 0..6 | null, done: boolean }
Category = 'work' | 'life' | 'body' | 'social'
```

**Derived per render** (do not store): the row-height/offset table (from which hours are
occupied), each event's top/height, the filtered buckets, open counts, the week's dates,
and the 33 picker rows.

**Backend contract.** These two shapes are what the API should serve and accept.
Suggested endpoints: `GET /week?start=<ISO date>` → `{ events }`;
`POST /events`, `PATCH /events/:id`, `DELETE /events/:id`;
`GET /tasks`, `POST /tasks`, `PATCH /tasks/:id`.
Note that `day` is a weekday index in the prototype — real persistence should use absolute
dates (and `start` should become a datetime) with the index derived per displayed week.
The reference Monday in the prototype is hardcoded as `new Date(2026, 7, 24)`;
replace it with the real current week's Monday.

## Design Tokens

**Surfaces**
| token | value |
|---|---|
| page / deepest | `#0c0e12` |
| pane | `rgba(16,20,27,.9)` + `backdrop-filter: blur(3px)` |
| pane sub-bar (header/footer/day row) | `rgba(10,13,20,.72)` |
| task row | `#171b22` (done `#12151b`) |
| dropdown | `rgba(12,16,24,.97)` |
| selected day / filter bar | `#0f3a5e` |
| dropdown selected row | `#16283a` |

**Lines**
`#1f2530` (2px pane dividers) · `#1c2029` (column divider) · `#1a1e27` (hour line) ·
`#232936` (dashed add-row) · `#39404f` (checkbox) · `#2f4a5e` (schedule focus border) ·
`#4a4334` (to-do focus border) · `rgba(207,234,246,.3)` (header buttons)

**Text**
`#e9ecf1` primary · `#c6ccd6` secondary · `#aab3c6` tertiary · `#98a0ad` muted ·
`#6b7280` dim · `#59637a` dimmer · `#4d5669` faintest · `#e2f1fa` on-sky button ·
`#bfe9f7` on-blue

**Accents**
| name | hex | used for |
|---|---|---|
| gold (incandescent) | `#F5B942` | today, week title, glow, dropdown heading |
| gold light | `#FFD98A` | today's date number |
| sand | `#FFCF95` | to-do pane mark, `TODAY` bucket, `life` category |
| cyan | `#7FD0EC` | schedule pane mark, `THIS WEEK` bucket, selection, `work` category |
| coral | `#FF8A76` | `social` category |
| green | `#A8D98F` | `body` category |
| violet | `#C79BF0` | fish / coral art only |

Event tiles use their category color at `.13` alpha; the hover state uses the same color
composited opaque over `rgb(20,23,29)`.

**Typography**
- Body / data: **Inconsolata** (400/500/600/700), Google Fonts.
- Display / labels: **Silkscreen** (400/700), Google Fonts — a *stand-in*. The user's own
  dashboard uses a custom pixel face called **"Flamingo"** that wasn't available here;
  if that font exists in your assets, substitute it everywhere Silkscreen appears.
- Scale in use: 19 / 13.5 / 13 / 12.5 / 12 / 11 / 10.5 / 10 / 9 / 8.5 px.
  Pixel-font labels are uppercase with `letter-spacing` .06–.16em.

**Geometry & motion**
- Radius: **0 everywhere.** "Rounded" corners are notched instead, via `clip-path`
  polygons — 8px notches on panes, 4px on buttons.
- Row height: `46px` normal, `17px` collapsed. Hour gutter: `52px`.
- Shell padding `18px 20px 20px`; pane gap `14px`.
- Easings: `cubic-bezier(.45,0,.2,1)` for pane resize (.5s);
  `cubic-bezier(.4,0,.2,1)` for row collapse and column widen (.34s / .32s);
  `.24s` for tile hover.
- Shadows: only two — the gold hover glow, and the hard pixel offsets
  `4px 4px 0 rgba(0,0,0,.5)` (drag ghost) and `0 10px 0 rgba(6,10,18,.45)` (dropdown).
  No soft ambient shadows.

## Assets
None. No image files, no icon set, no SVG. Every graphic is either a CSS box or drawn
procedurally on the background canvas. Fonts load from Google Fonts
(`Silkscreen`, `Inconsolata`). Sample events and tasks in the prototype are placeholder
content — replace with real data.

## Files
| file | what it is |
|---|---|
| `Tide Table.dc.html` | The app: both panes, header, week picker, drag/drop, all layout and tokens. Markup is the template; the logic class at the bottom holds state, sample data, and derived values. |
| `Sky.dc.html` | The background scene: day-cycle palette, sun/moon, clouds, waves, reef, fish. Self-contained canvas renderer. Props: `seed`, `waterline`, `liveTime`, `hour`, `dayCycle`. |
| `theme_reference.png` | The user's existing dashboard — origin of the palette, the notched-corner language, and the pixel type. |

Open either HTML file directly in a browser to see it running.
