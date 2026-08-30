//! Telegram capture for Marwan's Schedule.
//!
//! Deliberately a separate process, not something the app starts. The app has
//! no background services; this is one you opt into by running it. It talks to
//! the same SQLite store in WAL mode, so it works whether or not the window is
//! open, and messages sent while it is down are delivered by Telegram when it
//! comes back (they queue for 24h).
//!
//! Parsing is the same `interpret()` the app and the CLI use — there is no
//! second grammar and no LLM in the loop.

use chrono::{Local, Utc};
use ms_core::{
    add_from_text, find_by_prefix, get_items, get_week, help_text, interpret, set_done, stats,
    Command, Db, Filter,
};
use std::path::PathBuf;

const API: &str = "https://api.telegram.org/bot";
/// Long-poll timeout. Telegram holds the connection open until something
/// arrives, so this costs one idle socket rather than repeated requests.
const POLL_SECS: u64 = 50;

fn token() -> Option<String> {
    if let Ok(t) = std::env::var("TELEGRAM_TOKEN") {
        if !t.trim().is_empty() {
            return Some(t.trim().to_string());
        }
    }
    // bot.toml sits beside the store and is gitignored.
    let path = config_dir().join("bot.toml");
    let text = std::fs::read_to_string(path).ok()?;
    for line in text.lines() {
        if let Some(v) = line.trim().strip_prefix("token") {
            let v = v.trim_start_matches([' ', '=']).trim().trim_matches('"');
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn config_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("com.marwan.schedule")
}

fn db_path() -> PathBuf {
    std::env::var("SCHEDULE_DB")
        .map(PathBuf::from)
        .unwrap_or_else(|_| config_dir().join("schedule.db"))
}

fn local_zone() -> chrono_tz::Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(chrono_tz::UTC)
}

fn send(token: &str, chat_id: i64, text: &str) {
    // Telegram caps a message at 4096 characters.
    let body = if text.chars().count() > 4000 {
        text.chars().take(4000).collect::<String>()
    } else {
        text.to_string()
    };
    let _ = ureq::post(&format!("{API}{token}/sendMessage"))
        .send_json(ureq::json!({ "chat_id": chat_id, "text": body }));
}

fn fmt_mins(m: u32) -> String {
    if m % 60 == 0 {
        format!("{}h", m / 60)
    } else if m > 60 {
        format!("{}h{}m", m / 60, m % 60)
    } else {
        format!("{m}m")
    }
}

fn handle(db: &Db, text: &str) -> String {
    let today = Local::now().date_naive();
    let now = Utc::now();

    match interpret(text, today) {
        Command::Help => help_text(),

        Command::List => {
            let items = get_items(
                db,
                &Filter {
                    listed: Some(true),
                    done: Some(false),
                    on: Some(today),
                    ..Default::default()
                },
            );
            if items.is_empty() {
                return "nothing open".into();
            }
            let mut out = String::from("open:\n");
            for i in &items {
                let due = i
                    .due_at
                    .map(|d| d.with_timezone(&Local).format("  (%a %d %b)").to_string())
                    .unwrap_or_default();
                let est = i
                    .estimate_min
                    .map(|m| format!("  ~{}", fmt_mins(m)))
                    .unwrap_or_default();
                out.push_str(&format!("· {}{}{}\n", i.title, due, est));
            }
            out
        }

        Command::Week => {
            let week = get_week(db, today, local_zone());
            let mut out = format!("week of {}\n", week.anchor);
            for day in &week.days {
                out.push_str(&format!("\n{}\n", day.date.format("%a %d %b")));
                if day.placements.is_empty() {
                    out.push_str("  —\n");
                }
                for p in &day.placements {
                    let title = ms_core::store::fetch(db, &p.item_id)
                        .map(|i| i.title)
                        .unwrap_or_else(|| p.item_id.clone());
                    out.push_str(&format!(
                        "  {}-{}  {}\n",
                        p.starts_at.format("%H:%M"),
                        p.ends_at.format("%H:%M"),
                        title
                    ));
                }
            }
            for d in &week.diagnostics {
                out.push_str(&format!("\nnote: {}\n", d.message));
            }
            out
        }

        Command::Done(prefix) => {
            let matches = find_by_prefix(db, &prefix);
            match matches.len() {
                0 => format!("nothing matches {prefix:?}"),
                1 => {
                    let item = &matches[0];
                    // A repeat is ticked off for today's occurrence only.
                    let on = item.recurs.then_some(today);
                    match set_done(db, &item.id, true, on, now) {
                        Ok(_) => format!("done: {}", item.title),
                        Err(e) => format!("could not update: {e}"),
                    }
                }
                _ => {
                    let mut out = format!("{prefix:?} matches {}:\n", matches.len());
                    for m in matches.iter().take(10) {
                        out.push_str(&format!("· {}\n", m.title));
                    }
                    out.push_str("\nbe more specific");
                    out
                }
            }
        }

        Command::Capture(p) => {
            if p.title.trim().is_empty() {
                return "nothing to add".into();
            }
            match add_from_text(db, text, today, now) {
                Ok(item) => {
                    let mut out = format!("added: {}", item.title);
                    if let Some(d) = item.due_at {
                        out.push_str(&format!("\ndue {}", d.with_timezone(&Local).format("%a %d %b %H:%M")));
                    }
                    if item.recurs {
                        out.push_str(&format!("\nrepeats {}", p.byday.join(", ")));
                    }
                    if let Some(e) = item.estimate_min {
                        out.push_str(&format!("\nestimate {}", fmt_mins(e)));
                    }
                    if p.consumed.is_empty() {
                        out.push_str("\n\n(no syntax recognised — saved as plain text. send `help` for the syntax)");
                    }
                    let s = stats(db, None);
                    if let Some(phrase) = s.phrase {
                        out.push_str(&format!("\n\nfyi: things take {phrase} than you estimate"));
                    }
                    out
                }
                Err(e) => format!("could not add: {e}"),
            }
        }
    }
}

fn main() {
    let Some(token) = token() else {
        eprintln!(
            "msbot: no token.\n\
             Set TELEGRAM_TOKEN, or put `token = \"...\"` in {}",
            config_dir().join("bot.toml").display()
        );
        std::process::exit(1);
    };

    let path = db_path();
    let db = match Db::open(&path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("msbot: could not open {}: {e}", path.display());
            std::process::exit(1);
        }
    };
    eprintln!("msbot: polling, store at {}", path.display());

    let mut offset: i64 = 0;
    loop {
        let url = format!("{API}{token}/getUpdates?timeout={POLL_SECS}&offset={offset}");
        let resp = ureq::get(&url)
            .timeout(std::time::Duration::from_secs(POLL_SECS + 15))
            .call();

        let body: serde_json::Value = match resp {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("msbot: bad response: {e}");
                    std::thread::sleep(std::time::Duration::from_secs(5));
                    continue;
                }
            },
            Err(e) => {
                // A dropped long-poll is normal; back off briefly and retry
                // rather than exiting, so the bot survives a flaky connection.
                eprintln!("msbot: poll failed ({e}); retrying");
                std::thread::sleep(std::time::Duration::from_secs(5));
                continue;
            }
        };

        let Some(updates) = body.get("result").and_then(|r| r.as_array()) else {
            continue;
        };

        for u in updates {
            if let Some(id) = u.get("update_id").and_then(|v| v.as_i64()) {
                offset = offset.max(id + 1);
            }
            let Some(msg) = u.get("message") else { continue };
            let Some(chat_id) = msg.pointer("/chat/id").and_then(|v| v.as_i64()) else {
                continue;
            };
            let Some(text) = msg.get("text").and_then(|v| v.as_str()) else {
                send(&token, chat_id, "I only understand text.");
                continue;
            };

            let reply = handle(&db, text);
            send(&token, chat_id, &reply);
        }
    }
}
