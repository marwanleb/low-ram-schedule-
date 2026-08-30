use crate::db::Db;
use crate::parse::CATEGORIES;
use crate::store::new_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One record in a bulk import. `external_id` is required and caller-owned:
/// it is what makes a re-run a no-op instead of a duplicate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportRecord {
    pub external_id: String,
    pub title: String,
    #[serde(default)]
    pub due_at: Option<String>,
    #[serde(default)]
    pub estimate_min: Option<u32>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub location: Option<String>,
    /// A weekly commitment — a class, a shift. Without this an agent importing
    /// a timetable would have to fall back to `add`, which duplicates on every
    /// re-run, defeating the point of importing.
    #[serde(default)]
    pub repeat: Option<ImportRepeat>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImportRepeat {
    /// `mon` … `sun`.
    pub byday: Vec<String>,
    /// Wall clock, `HH:MM`.
    pub start_time: String,
    pub end_time: String,
    /// IANA zone to fix it to; omit to follow the machine.
    #[serde(default)]
    pub tz: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ImportOutcome {
    pub added: usize,
    pub updated: usize,
}

/// Upsert a batch keyed on `external_id`, in a single transaction.
///
/// Two properties matter more than anything else here, because an agent is the
/// usual caller and agents retry:
///
/// * Re-running the same batch changes nothing. The key is supplied by the
///   caller and enforced by a unique index, not by asking the agent to be
///   careful.
/// * A batch applies completely or not at all. A half-applied import is worse
///   than a failed one, because it looks like it worked.
///
/// Items you typed yourself have no `external_id` and are never matched.
pub fn import(
    db: &Db,
    records: &[ImportRecord],
    now: DateTime<Utc>,
) -> rusqlite::Result<ImportOutcome> {
    // Validate the whole batch before writing anything.
    for (i, r) in records.iter().enumerate() {
        if r.external_id.trim().is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "record {i} ({:?}) has no external_id; nothing was imported",
                r.title
            )));
        }
        if r.title.trim().is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "record {i} has an empty title; nothing was imported"
            )));
        }
        if let Some(rep) = &r.repeat {
            if rep.byday.is_empty() {
                return Err(rusqlite::Error::InvalidParameterName(format!(
                    "record {i} ({:?}) repeats on no days; nothing was imported",
                    r.title
                )));
            }
            if let Some(tz) = &rep.tz {
                if tz.parse::<chrono_tz::Tz>().is_err() {
                    return Err(rusqlite::Error::InvalidParameterName(format!(
                        "record {i} ({:?}) has unknown time zone {tz:?}; nothing was imported",
                        r.title
                    )));
                }
            }
        }
    }

    let mut outcome = ImportOutcome::default();
    let tx = db.conn.unchecked_transaction()?;

    for r in records {
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM items WHERE external_id = ?1",
                rusqlite::params![r.external_id],
                |row| row.get(0),
            )
            .ok();

        let tags = r.tags.join(",");
        // A timetabled commitment occupies the week; it does not belong in the
        // to-do list. Spec 3.1.
        let listed = r.repeat.is_none() as i64;
        let category = r
            .tags
            .iter()
            .find(|t| CATEGORIES.contains(&t.to_ascii_lowercase().as_str()))
            .map(|t| t.to_ascii_lowercase());

        match existing {
            Some(id) => {
                tx.execute(
                    "UPDATE items SET title = ?2, tags = ?3, category = ?4, due_at = ?5,
                                      estimate_min = ?6, location = ?7, listed = ?8
                     WHERE id = ?1",
                    rusqlite::params![
                        id, r.title, tags, category, r.due_at, r.estimate_min,
                        r.location, listed
                    ],
                )?;
                write_repeat(&tx, &id, r)?;
                outcome.updated += 1;
            }
            None => {
                let id = new_id();
                tx.execute(
                    "INSERT INTO items
                       (id, title, tags, category, location, listed, due_at, estimate_min,
                        created_at, source, external_id)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'import', ?10)",
                    rusqlite::params![
                        id,
                        r.title,
                        tags,
                        category,
                        r.location,
                        listed,
                        r.due_at,
                        r.estimate_min,
                        now.to_rfc3339(),
                        r.external_id
                    ],
                )?;
                write_repeat(&tx, &id, r)?;
                outcome.added += 1;
            }
        }
    }

    tx.commit()?;
    Ok(outcome)
}

/// Replace the item's rule with whatever the record says, including removing it
/// when the record no longer repeats — a cancelled class must stop appearing.
fn write_repeat(
    tx: &rusqlite::Transaction<'_>,
    item_id: &str,
    r: &ImportRecord,
) -> rusqlite::Result<()> {
    let Some(rep) = &r.repeat else {
        tx.execute(
            "DELETE FROM recurrence WHERE item_id = ?1",
            rusqlite::params![item_id],
        )?;
        return Ok(());
    };
    tx.execute(
        "INSERT INTO recurrence (item_id, byday, start_time, end_time, tz, from_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(item_id) DO UPDATE
           SET byday = ?2, start_time = ?3, end_time = ?4, tz = ?5",
        rusqlite::params![
            item_id,
            rep.byday.join(","),
            rep.start_time,
            rep.end_time,
            rep.tz,
            "1970-01-01",
        ],
    )?;
    Ok(())
}
