# Field entry, movable blocks, and a bot that asks — Design

Four changes with one thread running through them: **making a capture should not
require remembering the grammar**, and a block on the week should be a thing you
can move rather than a thing you re-type.

## The thread: one composer, two callers

The app form and the Telegram ladder both need to turn a set of fields into a
grammar line. Built twice — once in JavaScript, once in Rust — they are two
implementations of one rule set, and they drift. That is the failure this app
was rebuilt to avoid.

So there is one:

```rust
ms_core::compose::line(&Fields) -> String
```

the inverse of `parse`. Both surfaces fill `Fields` and call it. Nothing writes
to the store except through `add_from_text`, exactly as before.

This buys a property worth testing:

    parse(line(f)) == f

for any `f`. A round-trip test catches drift between the two directions without
anyone having to notice it — the differential-testing trick already used against
`dateutil`, pointed inward instead.

### Fields

    title        String
    priority     bool               → "!!" in front
    repeat       Vec<weekday>       → "every mon wed"
    kind         Task | Block       → "due" | "on"
    date         Option<NaiveDate>  → "dd/mm/yyyy"
    at           Option<NaiveTime>  → "HH:MM"
    span_end     Option<NaiveTime>  → makes it "HH:MM-HH:MM"
    estimate_min Option<u32>        → "~2h" / "~90m"
    tag          Option<String>     → "#math"
    place        Option<String>     → "@Hall 2.106"

Emitted in that order. `place` goes last because `@` runs to the end of the line
or until a recognised token; last is the position with no ambiguity to reason
about.

Estimates have one canonical spelling per value — whole hours as `~2h`,
everything else as `~90m` — so the round-trip is a function, not a relation.

## 1. The form

Expands from the existing capture row behind a `⌄` toggle. Not a new surface:
the input you already use stays the default and the form is the thing you open
when you cannot remember whether it is `~2h` or `2h~`.

The composed line renders live beneath the fields, in the same styling the
add-echo already uses. You watch the sentence assemble. That is the point — the
form teaches the grammar as a side effect, and you need it less over time.

Submitting sends the composed line to `cmd_add`. The form has no privileged
path into the store.

## 2. Moving and resizing a one-off

- Dragging the body moves it: `cmd_move_placement`, which already exists in Rust
  and has never been called from the frontend.
- Dragging the bottom edge resizes it. Snap to 15 minutes, minimum 15.
- The detail popover gains the same shorter/longer stepper that recurring rules
  already have, as the precise fallback — a 30-minute block is about 18px tall
  and its edge is not a reliable target on a trackpad.

## 3. Moving one occurrence of a repeat

A recurring occurrence is not a row. It is generated on read with a synthetic
id, so there is nothing to update. Moving one means two writes:

1. an exception on that date, so the series stops emitting it, and
2. a real placement at the new time, `origin = 'moved'`, `moved_from` = the
   original date.

Both or neither: `core::move_occurrence()` does them in one transaction. A
half-move leaves an occurrence that has vanished from the week without arriving
anywhere else, which is worse than a failed move.

The schema already anticipated this. `expand.rs` reads `origin='moved'` and
`moved_from`, and `model::Origin::Moved` exists. Nothing has ever written them.
`add_placement` hardcodes `'oneoff'` and needs to stop.

**Restoring a moved occurrence deletes the moved placement.** Otherwise
un-cancelling the date brings back the 10:30 class while the 14:00 copy is still
sitting there, and the same class appears twice.

Series-wide edits stay in the popover. Dragging never changes a rule.

## 4. The bot asks for what is missing

`Pending` becomes a small state machine:

    Filling { draft: Fields, step: Date | Time | Length }

It asks only for what the line did not supply, in that order. `-` or `skip`
advances; `cancel` abandons the draft. A complete line asks nothing, so the
ladder is the price of being terse rather than a tax on every capture.

This replaces `Confirm` and `AwaitingDate` — the date step subsumes the
undated-capture check, and asking "when?" is a better question than "did you
mean to leave this undated? (y/n)".

`handle_with_state` is already a pure function of message and state, so the
whole ladder is testable without a token or a network.

## Testing

- **Round-trip** — `parse(line(f)) == f`, over fixed cases and seeded random
  `Fields`. This is the load-bearing one.
- **`move_occurrence`** — the series survives, the exception exists, the moved
  placement is where it was dropped, and next week is untouched.
- **Restore** — un-excepting a moved date removes the moved placement.
- **Resize** — refuses an end at or before the start; honours the minimum.
- **Ladder** — driven as `(message, state) -> (reply, state)`, including skips,
  cancel, and a complete line asking nothing.

## Not in scope

Undo. Multi-select. Dragging a block back into the to-do list. Resizing a
recurring block by drag — that stays in the popover, where it already works.
