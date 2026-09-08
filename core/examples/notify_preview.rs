//! What the bot would push, without a token and without waiting ten minutes.
//!
//!     cargo run -p ms-core --example notify_preview
//!     cargo run -p ms-core --example notify_preview -- 60
//!     cargo run -p ms-core --example notify_preview -- 15 2026-09-09T15:22:00Z
//!
//! Reads `SCHEDULE_DB`, or the same default path the app and the bot use.

use chrono::{DateTime, Duration, Utc};
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lead: i64 = args.first().and_then(|s| s.parse().ok()).unwrap_or(10);
    let now: DateTime<Utc> = args
        .get(1)
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let path = std::env::var("SCHEDULE_DB").map(PathBuf::from).unwrap_or_else(|_| {
        let base = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".into());
        PathBuf::from(base).join("com.marwan.schedule").join("schedule.db")
    });

    let db = match ms_core::Db::open(&path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open {}: {e}", path.display());
            std::process::exit(1);
        }
    };

    let zone = ms_core::local_zone();
    println!("{}\n{} \u{2014} next {lead} min from {}\n", path.display(), zone.name(), now.with_timezone(&zone).format("%a %d %b %H:%M"));

    let out = ms_core::due_soon(&db, now, Duration::minutes(lead), zone);
    if out.is_empty() {
        println!("nothing");
    }
    for n in &out {
        let mins = ((n.at - now).num_seconds() as f64 / 60.0).ceil() as i64;
        let ends = n
            .ends
            .map(|e| format!("-{}", e.with_timezone(&zone).format("%H:%M")))
            .unwrap_or_default();
        println!(
            "{:>3} min  {}{}  {}{}   [{}]",
            mins,
            n.at.with_timezone(&zone).format("%H:%M"),
            ends,
            n.title,
            n.location.as_deref().map(|l| format!(" @{l}")).unwrap_or_default(),
            n.key
        );
    }
}
