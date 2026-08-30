use crate::db::Db;
use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: String,
    /// Never null: the durable link. A session outlives the placement it was
    /// started from. Spec 3.
    pub item_id: String,
    /// Best-effort provenance. A dangling value is expected, not corruption.
    pub placement_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub discarded: bool,
}

/// Starting a timer may stop one already running; the caller is told.
#[derive(Debug, Clone, PartialEq)]
pub struct Started {
    pub started: Session,
    pub stopped: Option<Session>,
}

const COLS: &str = "id, item_id, placement_id, started_at, ended_at, discarded";

fn row(r: &rusqlite::Row) -> rusqlite::Result<Session> {
    let started: String = r.get(3)?;
    let ended: Option<String> = r.get(4)?;
    Ok(Session {
        id: r.get(0)?,
        item_id: r.get(1)?,
        placement_id: r.get(2)?,
        started_at: parse_ts(&started),
        ended_at: ended.as_deref().map(parse_ts),
        discarded: r.get::<_, i64>(5)? != 0,
    })
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| DateTime::UNIX_EPOCH)
}

fn fetch(db: &Db, id: &str) -> Option<Session> {
    db.conn
        .query_row(
            &format!("SELECT {COLS} FROM sessions WHERE id = ?1"),
            rusqlite::params![id],
            row,
        )
        .ok()
}

/// A session open longer than this cannot be real work; it is a crash or a
/// forgotten stop. Spec 5.5.
pub const STALE_AFTER_HOURS: i64 = 12;

/// Quarantine sessions left running implausibly long. Called at startup.
///
/// They are marked `discarded` rather than deleted: the elapsed time is wrong,
/// so it must never reach the statistics, but the user is the one who decides
/// whether to correct or drop it. Returns what was quarantined so the caller
/// can surface it.
pub fn sweep_stale_sessions(db: &Db, now: DateTime<Utc>) -> rusqlite::Result<Vec<Session>> {
    let cutoff = (now - Duration::hours(STALE_AFTER_HOURS)).to_rfc3339();

    let mut stmt = db.conn.prepare(&format!(
        "SELECT {COLS} FROM sessions WHERE ended_at IS NULL AND discarded = 0 AND started_at < ?1"
    ))?;
    let stale: Vec<Session> = stmt
        .query_map(rusqlite::params![cutoff], row)?
        .filter_map(|r| r.ok())
        .collect();

    for s in &stale {
        db.conn.execute(
            "UPDATE sessions SET discarded = 1 WHERE id = ?1",
            rusqlite::params![s.id],
        )?;
    }

    Ok(stale
        .into_iter()
        .map(|s| Session { discarded: true, ..s })
        .collect())
}

/// The single running session, if any.
pub fn active_session(db: &Db) -> Option<Session> {
    db.conn
        .query_row(
            &format!("SELECT {COLS} FROM sessions WHERE ended_at IS NULL AND discarded = 0 ORDER BY started_at DESC LIMIT 1"),
            [],
            row,
        )
        .ok()
}

/// Start timing an item. Only one session runs at a time: any session already
/// running is stopped and returned, so the caller can say so rather than
/// silently abandoning it.
pub fn timer_start(
    db: &Db,
    item_id: &str,
    placement_id: Option<&str>,
    now: DateTime<Utc>,
) -> rusqlite::Result<Started> {
    let stopped = match active_session(db) {
        Some(running) => timer_stop(db, &running.id, now)?,
        None => None,
    };

    let id = format!("ses_{}", uuid::Uuid::new_v4().simple());
    db.conn.execute(
        "INSERT INTO sessions (id, item_id, placement_id, started_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, item_id, placement_id, now.to_rfc3339()],
    )?;

    let started = fetch(db, &id).expect("just inserted");
    Ok(Started { started, stopped })
}

/// Stop a running session. Returns None if it does not exist or already ended,
/// so stopping twice is harmless rather than an error.
pub fn timer_stop(db: &Db, session_id: &str, now: DateTime<Utc>) -> rusqlite::Result<Option<Session>> {
    let changed = db.conn.execute(
        "UPDATE sessions SET ended_at = ?2 WHERE id = ?1 AND ended_at IS NULL",
        rusqlite::params![session_id, now.to_rfc3339()],
    )?;
    Ok(if changed == 0 { None } else { fetch(db, session_id) })
}
