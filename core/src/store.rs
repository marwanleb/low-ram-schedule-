use crate::db::Db;
use crate::parse::parse;
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub tags: Vec<String>,
    pub category: Option<String>,
    /// Where it happens: `@Hall 2.106`. Parsed off the title, so it must be
    /// stored or the user loses something they deliberately typed.
    pub location: Option<String>,
    pub listed: bool,
    pub due_at: Option<DateTime<Utc>>,
    pub estimate_min: Option<u32>,
    pub recurs: bool,
    /// Occurrence dates ticked off. A one-off's completion is stored under an
    /// empty key and deliberately does NOT appear here — use `completed`.
    pub done_on: Vec<NaiveDate>,
    /// Ticked off at all: the one-off case, or any occurrence of a repeat.
    /// Kept as a field so no call site has to re-derive it and get it wrong.
    pub completed: bool,
    pub created_at: DateTime<Utc>,
    pub source: String,
    pub external_id: Option<String>,
}

impl Item {
    /// Done for one occurrence, or for the item itself when it does not repeat.
    pub fn is_done(&self, on: Option<NaiveDate>) -> bool {
        match (self.recurs, on) {
            (true, Some(d)) => self.done_on.contains(&d),
            _ => self.completed,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub listed: Option<bool>,
    pub recurs: Option<bool>,
    pub tag: Option<String>,
    pub done: Option<bool>,
    /// Which occurrence `done` refers to. A repeating item ticked off last
    /// Tuesday is open again this Tuesday, so asking "is it done?" without a
    /// date would retire it permanently. Ignored for one-offs.
    pub on: Option<NaiveDate>,
}

/// Resolve a wall clock in a zone to a real instant.
///
/// `LocalResult` has three variants and all three happen: on the spring-forward
/// morning the time does not exist, on the autumn one it happens twice.
fn local_instant(zone: Tz, date: NaiveDate, time: NaiveTime) -> Option<DateTime<Utc>> {
    use chrono::LocalResult;
    match zone.from_local_datetime(&date.and_time(time)) {
        LocalResult::Single(dt) => Some(dt.to_utc()),
        // The earlier of the two, matching how blocks are placed.
        LocalResult::Ambiguous(a, _) => Some(a.to_utc()),
        LocalResult::None => {
            // Skipped hour: the first instant that does exist that day.
            (1..=180).find_map(|m| {
                let probe = date.and_time(time) + Duration::minutes(m);
                match zone.from_local_datetime(&probe) {
                    LocalResult::Single(dt) => Some(dt.to_utc()),
                    _ => None,
                }
            })
        }
    }
}

/// Deadlines land at the end of the day; nobody means midnight.
const DEFAULT_DUE: (u32, u32) = (23, 59);

pub fn new_id() -> String {
    format!("itm_{}", uuid::Uuid::new_v4().simple())
}

/// Parse a typed line and store it, fixing any repeat to the machine's own
/// zone. Never rejects input — unparseable text becomes the title. Spec 6.
pub fn add_from_text(
    db: &Db,
    text: &str,
    today: NaiveDate,
    now: DateTime<Utc>,
) -> rusqlite::Result<Item> {
    add_from_text_in(db, text, today, now, local_zone())
}

/// The machine's current zone, read fresh. Travel is a render parameter, so
/// this is only consulted when an item is created. Spec 5.3.
pub fn local_zone() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(chrono_tz::UTC)
}

/// As `add_from_text`, with the creating zone supplied explicitly so tests and
/// the CLI do not depend on where the machine happens to be.
pub fn add_from_text_in(
    db: &Db,
    text: &str,
    today: NaiveDate,
    now: DateTime<Utc>,
    zone: Tz,
) -> rusqlite::Result<Item> {
    let p = parse(text, today);
    let id = new_id();

    // A due time is a wall clock where you are standing, resolved through the
    // zone and then stored as an instant. Labelling local time as UTC — which
    // this did — puts every deadline out by the offset.
    let due_at = p.due.and_then(|d| {
        let t = p
            .at
            .unwrap_or_else(|| NaiveTime::from_hms_opt(DEFAULT_DUE.0, DEFAULT_DUE.1, 0).unwrap());
        let mut when = local_instant(zone, d, t)?;

        // A bare weekday means the next one still to come. Naming today's own
        // weekday after that hour has passed used to file the item as already
        // overdue, silently. An explicit date is left alone — it may be
        // something already missed, deliberately recorded.
        let from_weekday = !p.repeats && p.byday.len() == 1;
        if from_weekday && when < now {
            when = local_instant(zone, d + Duration::days(7), t)?;
        }
        Some(when)
    });

    db.conn.execute(
        "INSERT INTO items (id, title, tags, category, location, listed, due_at, estimate_min, created_at, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'self')",
        rusqlite::params![
            id,
            p.title,
            p.tags.join(","),
            p.category,
            p.location,
            p.listed as i64,
            due_at.map(|d| d.to_rfc3339()),
            p.estimate_min,
            now.to_rfc3339(),
        ],
    )?;

    if p.repeats && !p.byday.is_empty() {
        let (start, end) = match p.span {
            Some((a, b)) => (a, b),
            // A repeat given only a point time still needs a span to occupy on
            // the week; an hour is the least surprising default.
            None => {
                let a = p.at.unwrap_or_else(|| NaiveTime::from_hms_opt(9, 0, 0).unwrap());
                (a, a + Duration::hours(1))
            }
        };
        // Fixed by default: the item keeps this zone's clock wherever you go.
        // `#floating` opts out and stores NULL. Spec 5.3.
        let tz = p.pinned.then(|| zone.name().to_string());
        db.conn.execute(
            "INSERT INTO recurrence (item_id, byday, start_time, end_time, tz, from_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                id,
                p.byday.join(","),
                start.format("%H:%M").to_string(),
                end.format("%H:%M").to_string(),
                tz,
                today.format("%Y-%m-%d").to_string(),
            ],
        )?;
    }

    Ok(fetch(db, &id).expect("just inserted"))
}

const COLS: &str = "i.id, i.title, i.tags, i.category, i.location, i.listed, i.due_at, i.estimate_min,
                    i.created_at, i.source, i.external_id,
                    (SELECT count(*) FROM recurrence r WHERE r.item_id = i.id)";

fn ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or(DateTime::UNIX_EPOCH)
}

fn row(r: &rusqlite::Row) -> rusqlite::Result<Item> {
    let tags: String = r.get(2)?;
    let due: Option<String> = r.get(6)?;
    let created: String = r.get(8)?;
    Ok(Item {
        id: r.get(0)?,
        title: r.get(1)?,
        tags: tags
            .split(',')
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect(),
        category: r.get(3)?,
        location: r.get(4)?,
        listed: r.get::<_, i64>(5)? != 0,
        due_at: due.as_deref().map(ts),
        estimate_min: r.get::<_, Option<i64>>(7)?.map(|v| v as u32),
        created_at: ts(&created),
        source: r.get(9)?,
        external_id: r.get(10)?,
        recurs: r.get::<_, i64>(11)? != 0,
        done_on: Vec::new(),
        completed: false,
    })
}

fn completions(db: &Db, item_id: &str) -> Vec<NaiveDate> {
    let Ok(mut stmt) = db
        .conn
        .prepare("SELECT on_date FROM completions WHERE item_id = ?1 ORDER BY on_date")
    else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map(rusqlite::params![item_id], |r| r.get::<_, String>(0)) else {
        return Vec::new();
    };
    rows.filter_map(|r| r.ok())
        .filter_map(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
        .collect()
}

/// Has this item been ticked off at all?
///
/// A one-off stores its completion under the empty date, which is not a
/// parseable date and so never reaches `done_on`. This is the only reliable
/// answer for that case.
fn ticked(db: &Db, item_id: &str) -> bool {
    db.conn
        .query_row(
            "SELECT count(*) FROM completions WHERE item_id = ?1",
            rusqlite::params![item_id],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0
}

pub fn fetch(db: &Db, id: &str) -> Option<Item> {
    let mut item = db
        .conn
        .query_row(
            &format!("SELECT {COLS} FROM items i WHERE i.id = ?1"),
            rusqlite::params![id],
            row,
        )
        .ok()?;
    item.done_on = completions(db, &item.id);
    item.completed = ticked(db, &item.id);
    Some(item)
}

pub fn get_items(db: &Db, filter: &Filter) -> Vec<Item> {
    let Ok(mut stmt) = db
        .conn
        .prepare(&format!("SELECT {COLS} FROM items i ORDER BY i.created_at"))
    else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], row) else {
        return Vec::new();
    };

    rows.filter_map(|r| r.ok())
        .map(|mut it| {
            it.done_on = completions(db, &it.id);
            it.completed = ticked(db, &it.id);
            it
        })
        .filter(|it| filter.listed.is_none_or(|w| it.listed == w))
        .filter(|it| filter.recurs.is_none_or(|w| it.recurs == w))
        .filter(|it| filter.done.is_none_or(|w| it.is_done(filter.on) == w))
        .filter(|it| match &filter.tag {
            None => true,
            Some(want) => it.tags.iter().any(|t| t.eq_ignore_ascii_case(want)),
        })
        .collect()
}

/// Tick off one occurrence, or the item itself when `on` is None.
///
/// Completions live in their own table keyed by (item, date) so finishing this
/// week's chore does not finish it forever. Spec 3.2.
pub fn set_done(
    db: &Db,
    id: &str,
    done: bool,
    on: Option<NaiveDate>,
    now: DateTime<Utc>,
) -> rusqlite::Result<bool> {
    let key = on
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_default();
    let changed = if done {
        db.conn.execute(
            "INSERT OR REPLACE INTO completions (item_id, on_date, done_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![id, key, now.to_rfc3339()],
        )?
    } else {
        db.conn.execute(
            "DELETE FROM completions WHERE item_id = ?1 AND on_date = ?2",
            rusqlite::params![id, key],
        )?
    };
    Ok(changed > 0)
}

pub fn delete_item(db: &Db, id: &str) -> rusqlite::Result<bool> {
    Ok(db
        .conn
        .execute("DELETE FROM items WHERE id = ?1", rusqlite::params![id])?
        > 0)
}

/// First item whose title starts with `prefix`, case-insensitively. Used by the
/// `done: <text>` command, where typing the whole title would be tedious.
pub fn find_by_prefix(db: &Db, prefix: &str) -> Vec<Item> {
    let needle = prefix.trim().to_ascii_lowercase();
    get_items(db, &Filter::default())
        .into_iter()
        .filter(|it| it.title.to_ascii_lowercase().starts_with(&needle))
        .collect()
}

/// Fix a repeat to a named zone, or `None` to let it follow the machine.
///
/// An unknown zone is refused rather than stored: expansion would silently
/// degrade it to floating, and the user would never learn the name was wrong.
pub fn set_recurrence_tz(db: &Db, item_id: &str, tz: Option<&str>) -> Result<(), String> {
    if let Some(name) = tz {
        if name.parse::<Tz>().is_err() {
            return Err(format!("{name:?} is not a known time zone"));
        }
    }
    let changed = db
        .conn
        .execute(
            "UPDATE recurrence SET tz = ?2 WHERE item_id = ?1",
            rusqlite::params![item_id, tz],
        )
        .map_err(|e| e.to_string())?;
    if changed == 0 {
        return Err("that item does not repeat".into());
    }
    Ok(())
}

/// Give an item a specific block of time — what dropping a task on the grid
/// does. It creates a placement for the *same* item, never a copy, so ticking
/// it off or timing it stays connected. Spec 3.1.
pub fn add_placement(
    db: &Db,
    item_id: &str,
    starts_at: &str,
    ends_at: &str,
) -> Result<String, String> {
    let (Ok(s), Ok(e)) = (
        DateTime::parse_from_rfc3339(starts_at),
        DateTime::parse_from_rfc3339(ends_at),
    ) else {
        return Err("times must be RFC3339".into());
    };
    if e <= s {
        return Err("a block must end after it starts".into());
    }
    let id = format!("plc_{}", uuid::Uuid::new_v4().simple());
    db.conn
        .execute(
            "INSERT INTO placements (id, item_id, starts_at, ends_at, origin)
             VALUES (?1, ?2, ?3, ?4, 'oneoff')",
            rusqlite::params![id, item_id, starts_at, ends_at],
        )
        .map_err(|e| e.to_string())?;
    Ok(id)
}

/// Move an existing one-off block.
pub fn move_placement(db: &Db, id: &str, starts_at: &str, ends_at: &str) -> Result<(), String> {
    let (Ok(s), Ok(e)) = (
        DateTime::parse_from_rfc3339(starts_at),
        DateTime::parse_from_rfc3339(ends_at),
    ) else {
        return Err("times must be RFC3339".into());
    };
    if e <= s {
        return Err("a block must end after it starts".into());
    }
    let n = db
        .conn
        .execute(
            "UPDATE placements SET starts_at = ?2, ends_at = ?3 WHERE id = ?1",
            rusqlite::params![id, starts_at, ends_at],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("no such block".into());
    }
    Ok(())
}

pub fn delete_placement(db: &Db, id: &str) -> Result<bool, String> {
    db.conn
        .execute("DELETE FROM placements WHERE id = ?1", rusqlite::params![id])
        .map(|n| n > 0)
        .map_err(|e| e.to_string())
}

/// Set or clear an item's estimate. `None` means unestimated, which is
/// distinct from zero and keeps it out of the ratio in §5.6.
pub fn set_estimate(db: &Db, id: &str, minutes: Option<u32>) -> rusqlite::Result<()> {
    db.conn.execute(
        "UPDATE items SET estimate_min = ?2 WHERE id = ?1",
        rusqlite::params![id, minutes],
    )?;
    Ok(())
}

/// Whether the item appears in the To Do pane. Gates that one view and nothing
/// else — it still repeats, completes and times either way. Spec 3.1.
pub fn set_listed(db: &Db, id: &str, listed: bool) -> rusqlite::Result<()> {
    db.conn.execute(
        "UPDATE items SET listed = ?2 WHERE id = ?1",
        rusqlite::params![id, listed as i64],
    )?;
    Ok(())
}
