use crate::db::Db;
use crate::model::{Day, Diagnostic, Level, Origin, Placement, Week};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveTime, Offset, TimeZone, Weekday};
use chrono_tz::Tz;

const DAY_CODES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

fn code_for(w: Weekday) -> &'static str {
    DAY_CODES[w.num_days_from_monday() as usize]
}

/// A recurrence row exactly as stored. Any field may be malformed.
struct RawRule {
    item_id: String,
    byday: String,
    start_time: String,
    end_time: String,
    tz: Option<String>,
    from_date: String,
    until_date: Option<String>,
    except_on: String,
}

/// A rule that has survived validation and can be expanded without failing.
struct Rule {
    item_id: String,
    days: Vec<String>,
    start: NaiveTime,
    end: NaiveTime,
    zone: Tz,
    /// None = systime (follows the machine).
    pinned_tz: Option<String>,
    from: NaiveDate,
    until: Option<NaiveDate>,
    except: Vec<NaiveDate>,
}

fn load_rules(db: &Db, diags: &mut Vec<Diagnostic>) -> Vec<RawRule> {
    let stmt = db
        .conn
        .prepare("SELECT item_id, byday, start_time, end_time, tz, from_date, until_date, except_on FROM recurrence");
    let mut stmt = match stmt {
        Ok(s) => s,
        Err(e) => {
            // An unreadable store must say so. Returning an empty week here
            // would look exactly like "you have nothing scheduled".
            diags.push(Diagnostic {
                level: Level::Warn,
                item_id: None,
                message: format!("Could not read the schedule: {e}"),
            });
            return Vec::new();
        }
    };
    let rows = stmt.query_map([], |r| {
        Ok(RawRule {
            item_id: r.get(0)?,
            byday: r.get(1)?,
            start_time: r.get(2)?,
            end_time: r.get(3)?,
            tz: r.get(4)?,
            from_date: r.get(5)?,
            until_date: r.get(6)?,
            except_on: r.get(7)?,
        })
    });
    match rows {
        Ok(it) => it.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            diags.push(Diagnostic {
                level: Level::Warn,
                item_id: None,
                message: format!("Could not read the schedule: {e}"),
            });
            Vec::new()
        }
    }
}

/// Every instant a wall-clock time maps to in a zone, earliest first.
///
/// Usually one. Twice a year it is two (the repeated hour) or none (the skipped
/// hour), and both cases occur in real schedules.
fn candidates(zone: Tz, date: NaiveDate, t: NaiveTime) -> Vec<DateTime<Tz>> {
    match zone.from_local_datetime(&date.and_time(t)) {
        LocalResult::Single(dt) => vec![dt],
        LocalResult::Ambiguous(a, b) => vec![a, b],
        LocalResult::None => {
            // Skipped hour: walk forward to the first instant that exists.
            // Observed gaps run 30-120 minutes; a day is generous and bounded.
            for step in 1..=1440 {
                let Some(probe) = date.and_time(t).checked_add_signed(Duration::minutes(step))
                else {
                    return Vec::new();
                };
                if let LocalResult::Single(dt) = zone.from_local_datetime(&probe) {
                    return vec![dt];
                }
            }
            Vec::new()
        }
    }
}

/// Where a block actually sits, and whether that needed explaining.
struct Landed {
    starts: DateTime<Tz>,
    ends: DateTime<Tz>,
    note: Option<String>,
}

/// Place a block, keeping the written times AND the written length.
///
/// On the night the clocks go back, "01:30-02:30" has two readings: the first
/// 01:30 to 02:30 is two real hours, the second 01:30 to 02:30 is one. Picking
/// the start in isolation and adding the duration gets the length right but
/// renders "01:30-01:30"; resolving both ends literally renders correctly but
/// makes a one-hour class occupy two. Choosing the *pair* whose elapsed time
/// matches what was written gets both.
fn place(
    zone: Tz,
    date: NaiveDate,
    start: NaiveTime,
    end: NaiveTime,
) -> Option<Landed> {
    let written = end - start;
    let starts = candidates(zone, date, start);
    let ends = candidates(zone, date, end);

    if starts.is_empty() {
        return None;
    }

    // Prefer a pairing that already spans exactly what was written; among
    // those, the earliest start.
    for s in &starts {
        for e in &ends {
            if *e - *s == written {
                let note = (starts.len() > 1 || ends.len() > 1).then(|| format!(
                    "{} {}-{} falls in a clock change in {}; kept at {}",
                    date,
                    start.format("%H:%M"),
                    end.format("%H:%M"),
                    zone.name(),
                    format_args!("{} minutes", written.num_minutes())
                ));
                return Some(Landed { starts: *s, ends: *e, note });
            }
        }
    }

    // No consistent pairing: the times were skipped rather than repeated.
    // Keep the length and report where it moved to.
    let s = starts[0];
    let e = s.checked_add_signed(written)?;
    Some(Landed {
        starts: s,
        ends: e,
        note: Some(format!(
            "{} {} does not exist in {} (clocks skip forward) — moved to {}",
            date,
            start.format("%H:%M"),
            zone.name(),
            s.format("%H:%M")
        )),
    })
}

fn warn(diags: &mut Vec<Diagnostic>, item_id: &str, message: String) {
    diags.push(Diagnostic {
        level: Level::Warn,
        item_id: Some(item_id.to_string()),
        message,
    });
}

/// Validate once, before the day loop, so a bad rule reports a single
/// diagnostic rather than one per day. Returning None drops the rule from the
/// render; degrading (e.g. an unknown zone) keeps it and still reports.
fn validate(raw: RawRule, viewing: Tz, diags: &mut Vec<Diagnostic>) -> Option<Rule> {
    let id = raw.item_id.clone();

    let Ok(from) = NaiveDate::parse_from_str(&raw.from_date, "%Y-%m-%d") else {
        warn(diags, &id, format!("Recurrence skipped: unreadable from_date {:?}", raw.from_date));
        return None;
    };

    // An unreadable until_date is treated as open-ended rather than dropping
    // the rule: the series still exists, only its end is unknown.
    let until = match raw.until_date.as_deref().filter(|u| !u.trim().is_empty()) {
        None => None,
        Some(text) => match NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            Ok(u) => Some(u),
            Err(_) => {
                warn(diags, &id, format!("Unreadable until_date {text:?} — treated as open-ended"));
                None
            }
        },
    };

    if let Some(u) = until {
        if u < from {
            warn(diags, &id, format!(
                "Recurrence skipped: until_date {u} precedes from_date {from}"
            ));
            return None;
        }
    }

    let (Ok(start), Ok(end)) = (
        NaiveTime::parse_from_str(&raw.start_time, "%H:%M"),
        NaiveTime::parse_from_str(&raw.end_time, "%H:%M"),
    ) else {
        warn(diags, &id, format!("Recurrence skipped: unreadable time range {:?}-{:?}", raw.start_time, raw.end_time));
        return None;
    };

    if end <= start {
        warn(diags, &id, format!(
            "Recurrence skipped: end {} is not after start {}",
            raw.end_time, raw.start_time
        ));
        return None;
    }

    let days: Vec<String> = raw
        .byday
        .split(',')
        .map(|c| c.trim().to_ascii_lowercase())
        .filter(|c| DAY_CODES.contains(&c.as_str()))
        .collect();

    // A rule that matches no weekday generates nothing forever, which is
    // indistinguishable from the item being lost unless it is reported.
    if days.is_empty() {
        warn(diags, &id, format!(
            "Recurrence never occurs: no usable weekday in {:?}", raw.byday
        ));
        return None;
    }

    // An unknown zone degrades to systime rather than vanishing: silent
    // omission is the worst available failure mode.
    let (zone, pinned_tz) = match &raw.tz {
        None => (viewing, None),
        Some(name) => match name.parse::<Tz>() {
            Ok(z) => (z, Some(name.clone())),
            Err(_) => {
                warn(diags, &id, format!("Unknown time zone {:?} — shown on system time instead", name));
                (viewing, None)
            }
        },
    };

    // An except_on entry naming a date the rule never generates, or one
    // outside its range, is a silent no-op — not an error.
    let mut except: Vec<NaiveDate> = Vec::new();
    for entry in raw.except_on.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        match NaiveDate::parse_from_str(entry, "%Y-%m-%d") {
            Ok(date) => except.push(date),
            // Drop just this entry; the rest of the rule still expands. But say
            // so, or a date the user meant to cancel will quietly come back.
            Err(_) => warn(diags, &id, format!(
                "Ignored unreadable exception date {entry:?} — that occurrence will still appear"
            )),
        }
    }

    Some(Rule { item_id: id, days, start, end, zone, pinned_tz, from, until, except })
}

/// Snap to the Monday of the anchor's week, clamped so the whole week is
/// representable. Plain date arithmetic panics at the calendar bounds.
fn normalise_anchor(anchor: NaiveDate, diags: &mut Vec<Diagnostic>) -> NaiveDate {
    let latest_start = NaiveDate::MAX
        .checked_sub_signed(Duration::days(6))
        .unwrap_or(NaiveDate::MAX);

    let clamped = anchor.min(latest_start);
    if clamped != anchor {
        diags.push(Diagnostic {
            level: Level::Warn,
            item_id: None,
            message: format!("Week starting {anchor} is past the end of the calendar; showing {clamped}"),
        });
    }

    let back = clamped.weekday().num_days_from_monday() as i64;
    match clamped.checked_sub_signed(Duration::days(back)) {
        Some(monday) => monday,
        // Only reachable within six days of NaiveDate::MIN, where no earlier
        // Monday exists. Keep the week rather than fail.
        None => clamped,
    }
}

/// One-off blocks stored explicitly, as opposed to generated from a rule.
struct Stored {
    id: String,
    item_id: String,
    starts_at: DateTime<Tz>,
    ends_at: DateTime<Tz>,
    origin: Origin,
    moved_from: Option<NaiveDate>,
}

fn load_placements(db: &Db, viewing: Tz, diags: &mut Vec<Diagnostic>) -> Vec<Stored> {
    let Ok(mut stmt) = db
        .conn
        .prepare("SELECT id, item_id, starts_at, ends_at, origin, moved_from FROM placements")
    else {
        diags.push(Diagnostic {
            level: Level::Warn,
            item_id: None,
            message: "Could not read one-off blocks".into(),
        });
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<String>>(5)?,
        ))
    }) else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (id, item_id, s, e, origin, moved) in rows.filter_map(|r| r.ok()) {
        let (Ok(sd), Ok(ed)) = (
            DateTime::parse_from_rfc3339(&s),
            DateTime::parse_from_rfc3339(&e),
        ) else {
            // Say so rather than let the block vanish.
            warn(diags, &item_id, format!("Could not place a one-off block: unreadable time {s:?}"));
            continue;
        };
        out.push(Stored {
            id,
            item_id,
            starts_at: sd.with_timezone(&viewing),
            ends_at: ed.with_timezone(&viewing),
            origin: if origin == "moved" { Origin::Moved } else { Origin::OneOff },
            moved_from: moved.and_then(|m| NaiveDate::parse_from_str(&m, "%Y-%m-%d").ok()),
        });
    }
    out
}

/// Total function: returns a Week with up to 7 days for any database state.
/// Never panics, never returns Err. Problems surface as `Week.diagnostics`.
pub fn get_week(db: &Db, anchor: NaiveDate, viewing: Tz) -> Week {
    let mut diagnostics = Vec::new();
    let anchor = normalise_anchor(anchor, &mut diagnostics);
    let rules: Vec<Rule> = load_rules(db, &mut diagnostics)
        .into_iter()
        .filter_map(|raw| validate(raw, viewing, &mut diagnostics))
        .collect();

    let stored = load_placements(db, viewing, &mut diagnostics);

    let mut days = Vec::with_capacity(7);
    for offset in 0..7 {
        // Checked: plain `+` panics past NaiveDate::MAX, which would break the
        // totality guarantee on the one input nobody tests by hand.
        let Some(date) = anchor.checked_add_signed(Duration::days(offset)) else {
            break;
        };
        let mut placements = Vec::new();

        for rule in &rules {
            if date < rule.from {
                continue;
            }
            if rule.until.is_some_and(|u| date > u) {
                continue;
            }
            if !rule.days.iter().any(|c| c == code_for(date.weekday())) {
                continue;
            }
            if rule.except.contains(&date) {
                continue;
            }
            let Some(landed) = place(rule.zone, date, rule.start, rule.end) else {
                warn(&mut diagnostics, &rule.item_id, format!(
                    "Could not place {} {} in {} — occurrence skipped",
                    date, rule.start.format("%H:%M"), rule.zone.name()
                ));
                continue;
            };
            if let Some(note) = &landed.note {
                warn(&mut diagnostics, &rule.item_id, note.clone());
            }
            let (starts, ends) = (landed.starts, landed.ends);

            placements.push(Placement {
                id: format!("plc_{}_{}", date, rule.item_id),
                item_id: rule.item_id.clone(),
                // Emitted in the viewing zone so the frontend renders the
                // string directly; the instant is unchanged. See spec 5.3.
                starts_at: starts.with_timezone(&viewing).fixed_offset(),
                ends_at: ends.with_timezone(&viewing).fixed_offset(),
                origin: Origin::Recurrence,
                moved_from: None,
                pinned_tz: rule.pinned_tz.clone(),
                // Compared by UTC offset at this instant, not by zone name:
                // US/Central and America/Chicago are the same clock, and a
                // marker on an item that renders at the identical time is a
                // false alarm.
                foreign: rule.pinned_tz.is_some()
                    && starts.offset().fix() != starts.with_timezone(&viewing).offset().fix(),
            });
        }

        for sp in &stored {
            if sp.starts_at.date_naive() != date {
                continue;
            }
            placements.push(Placement {
                id: sp.id.clone(),
                item_id: sp.item_id.clone(),
                starts_at: sp.starts_at.fixed_offset(),
                ends_at: sp.ends_at.fixed_offset(),
                origin: sp.origin,
                moved_from: sp.moved_from,
                // A one-off is a fixed instant; it is never foreign.
                pinned_tz: None,
                foreign: false,
            });
        }

        placements.sort_by(|a, b| a.starts_at.cmp(&b.starts_at).then(a.item_id.cmp(&b.item_id)));
        days.push(Day { date, placements });
    }

    Week { anchor, viewing_tz: viewing.name().to_string(), days, diagnostics }
}
