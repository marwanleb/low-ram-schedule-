# Marwan's Schedule

A weekly planner that exists to do the three things paper cannot: keep recurring
commitments without rewriting them, time work against your estimate, and be in
your pocket.

Design: [`docs/superpowers/specs/2026-08-29-marwans-schedule-design.md`](docs/superpowers/specs/2026-08-29-marwans-schedule-design.md)
Agent instructions: [`AGENT.md`](AGENT.md)

## Layout

```
core/          all logic: schema, week expansion, parser, timer, stats, import
cli/           `sched` — the command line, and what an agent drives
bot/           `msbot` — Telegram capture (a separate process you opt into)
app/           the Tauri window: src-tauri (commands) + src (frontend)
```

Everything goes through `core`. The window, the CLI and the bot are three front
doors onto one store and one grammar, so they cannot drift apart.

## Build

```sh
cargo build --release
```

Binaries land in `target/release/`: `marwans-schedule.exe`, `sched.exe`,
`msbot.exe`.

## Run

```sh
cargo run -p marwans-schedule      # the window
sched add "math hw fri ~2h #math"  # capture from a terminal
sched help                         # syntax
msbot                              # Telegram capture, needs a token
```

All three share one database at `%APPDATA%\com.marwan.schedule\schedule.db`
(SQLite, WAL). Override with `SCHEDULE_DB`, or `sched --db <path>`.

## Living in the tray

The window opens maximised and borderless — windowed fullscreen, not exclusive,
so alt-tab and overlays behave normally.

- **Ctrl+Alt+S** summons it, and puts it away again if it is already in front.
- **Esc** hides it. So does closing it: the window goes to the tray rather than
  quitting, which is the point of a thing that is always one key away.
- The tray icon toggles on left click and has Open / Quit on right click.

The background canvas stops drawing the moment the window hides — Rust emits
`window:hidden` explicitly, because a hidden native window does not reliably
fire `visibilitychange` and a loop running behind a game is the exact problem
this app was rebuilt to avoid.

### Launch at login

The app enables it for itself on first run, and starts hidden with `--hidden`
so it sits in the tray instead of appearing over whatever you were doing.

To turn it off: Settings → Apps → Startup, or delete the
`marwans-schedule` entry under
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

The bot is separate, because it is a console program: a
`msbot.vbs` in the Startup folder launches it with no console window. Delete
that file to stop it.

## The Telegram bot

It needs a token from [@BotFather](https://t.me/BotFather), supplied either way:

```sh
set TELEGRAM_TOKEN=...
```

or a line in `%APPDATA%\com.marwan.schedule\bot.toml`:

```toml
token = "..."
```

That file is gitignored. Keep the token out of the repository.

The bot is a **separate process on purpose**. The app runs no background
services; this is one you start when you want it. Messages sent while it is down
are delivered when it comes back — Telegram queues them for 24 hours.

## Syntax

Plain text always works. Anything the parser does not recognise stays in the
title, so ordinary sentences are safe.

```
renew parking
math hw fri 5pm ~2h #math
trash every tue 20:00
MATH210 every mon wed 9:00-10:15 @Hall 1.204
vitamins daily 08:00
!! renew parking tomorrow
```

Run `sched help` for the full reference; the app and the bot print the same text
from the same function, and a test asserts every example in it actually parses.

## Tests

```sh
cargo test
```

70 tests. The ones worth knowing about:

- `fuzz_totality` — 4,000 seeded cases of malformed data against every tzdata
  zone, asserting `get_week` never panics.
- `defects` — regressions for bugs found after the fact: DST duration, calendar
  bounds, zone aliases, silent store failures.
- `expansion` — recurrence, exceptions, and the spec's error catalogue.
- `parser` — including the cases that must **not** parse, which matter more than
  the ones that do.
