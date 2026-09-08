//! What is about to happen, and what has already been said about it.
//!
//! The bot is the only always-running process, so it is what pushes — but
//! deciding *what* to push is store logic, and it lives here where it can be
//! tested without a network or a token.
//!
//! Two rules keep this from becoming noise:
//!
//! 1. A notice is keyed by occurrence **and instant**, so moving a block
//!    announces it again while a restart does not.
//! 2. Anything already begun, or already ticked off, is not announced.

use crate::db::Db;
use crate::expand::get_week;
use crate::store::{get_items, Filter, Item};
use chrono::{DateTime, Datelike, Duration, Utc};
use chrono_tz::Tz;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// An hour on the week: a class, a meeting, anything with a start.
    Block,
    /// A deadline in the to-do list.
    Deadline,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub key: String,
    pub kind: Kind,
    pub title: String,
    pub location: Option<String>,
    pub at: DateTime<Utc>,
    /// Blocks know when they end; a deadline is a point.
    pub ends: Option<DateTime<Utc>>,
}

/// Everything starting or falling due in `(now, now + lead]`.
///
/// The half-open window matters: something that has already begun is not news,
/// and the caller ticks far more often than `lead` is wide, so nothing can slip
/// between two windows.
pub fn due_soon(db: &Db, now: DateTime<Utc>, lead: Duration, zone: Tz) -> Vec<Notice> {
    let horizon = now + lead;
    let items = get_items(db, &Filter::default());
    let by_id: HashMap<&str, &Item> = items.iter().map(|i| (i.id.as_str(), i)).collect();

    let mut out = Vec::new();
    // (item, instant) for every block announced, so the same thing said with
    // "on" -- which is both a block and something to tick off -- is not
    // announced twice at the same moment.
    let mut placed: HashSet<(String, DateTime<Utc>)> = HashSet::new();

    // Two weeks, because the horizon can sit on the far side of Monday.
    let today = now.with_timezone(&zone).date_naive();
    let monday = today - Duration::days(today.weekday().num_days_from_monday() as i64);
    for week in [monday, monday + Duration::days(7)] {
        for day in get_week(db, week, zone).days {
            for p in day.placements {
                let start = p.starts_at.with_timezone(&Utc);
                if start <= now || start > horizon {
                    continue;
                }
                let Some(item) = by_id.get(p.item_id.as_str()) else {
                    continue;
                };
                if item.is_done(Some(day.date)) {
                    continue;
                }
                placed.insert((p.item_id.clone(), start));
                out.push(Notice {
                    key: format!("{}@{}", p.id, start.to_rfc3339()),
                    kind: Kind::Block,
                    title: item.title.clone(),
                    location: item.location.clone(),
                    at: start,
                    ends: Some(p.ends_at.with_timezone(&Utc)),
                });
            }
        }
    }

    for item in &items {
        let Some(due) = item.due_at else { continue };
        // A block already announces itself; only what sits in the list needs a
        // deadline reminder.
        if !item.listed || item.completed || due <= now || due > horizon {
            continue;
        }
        if placed.contains(&(item.id.clone(), due)) {
            continue;
        }
        out.push(Notice {
            key: format!("due_{}@{}", item.id, due.to_rfc3339()),
            kind: Kind::Deadline,
            title: item.title.clone(),
            location: item.location.clone(),
            at: due,
            ends: None,
        });
    }

    out.sort_by(|a, b| a.at.cmp(&b.at).then_with(|| a.key.cmp(&b.key)));
    out.dedup_by(|a, b| a.key == b.key);
    out
}

pub fn already_sent(db: &Db, key: &str) -> bool {
    db.conn
        .query_row(
            "SELECT count(*) FROM notified WHERE key = ?1",
            rusqlite::params![key],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0
}

pub fn mark_sent(db: &Db, key: &str, now: DateTime<Utc>) -> rusqlite::Result<()> {
    db.conn.execute(
        "INSERT OR REPLACE INTO notified (key, sent_at) VALUES (?1, ?2)",
        rusqlite::params![key, now.to_rfc3339()],
    )?;
    Ok(())
}

/// Drop rows for notices long past. A key is only ever consulted while its
/// occurrence is still in the future, so anything older is dead weight.
pub fn prune_sent(db: &Db, before: DateTime<Utc>) -> rusqlite::Result<usize> {
    db.conn.execute(
        "DELETE FROM notified WHERE sent_at < ?1",
        rusqlite::params![before.to_rfc3339()],
    )
}
