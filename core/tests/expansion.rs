use chrono::NaiveDate;
use chrono_tz::America::Chicago;
use chrono_tz::Asia::Beirut;
use ms_core::{get_week, Db};

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Insert an item with a weekly recurrence, straight to SQL, so these tests
/// exercise expansion rather than any insert API.
fn add_recurring(db: &Db, id: &str, byday: &str, start: &str, end: &str, from: &str, tz: Option<&str>) {
    db.conn
        .execute(
            "INSERT INTO items (id, title, created_at) VALUES (?1, ?2, '2026-08-01T00:00:00Z')",
            rusqlite::params![id, id],
        )
        .unwrap();
    db.conn
        .execute(
            "INSERT INTO recurrence (item_id, byday, start_time, end_time, tz, from_date)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![id, byday, start, end, tz, from],
        )
        .unwrap();
}

#[test]
fn weekly_rule_places_only_on_its_days() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon,wed", "09:00", "10:15", "2026-08-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-08-31"), Chicago);

    let counts: Vec<usize> = week.days.iter().map(|day| day.placements.len()).collect();
    assert_eq!(counts, vec![1, 0, 1, 0, 0, 0, 0], "expected placements on Mon and Wed only");
}

#[test]
fn pinned_item_is_emitted_in_the_viewing_zone() {
    let db = Db::open_in_memory().unwrap();
    // class 12:00 Austin, per the spec's canonical example
    add_recurring(&db, "math210", "wed", "12:00", "13:00", "2026-08-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-08-31"), Beirut);
    let p = &week.days[2].placements[0];

    // Seen from Beirut, noon in Austin is 20:00 — and the emitted offset is
    // Beirut's, so the frontend can render the string without converting.
    assert_eq!(p.starts_at.to_rfc3339(), "2026-09-02T20:00:00+03:00");
    assert!(p.foreign, "pinned to a zone other than the viewing zone");
    assert_eq!(p.pinned_tz.as_deref(), Some("America/Chicago"));
}

#[test]
fn systime_item_keeps_its_wall_clock_in_every_zone() {
    let db = Db::open_in_memory().unwrap();
    // "wake up 08:00 systime" — follows the machine, per the spec's canonical pair
    add_recurring(&db, "wake", "mon", "08:00", "08:30", "2026-08-01", None);

    let austin = get_week(&db, d("2026-08-31"), Chicago);
    let beirut = get_week(&db, d("2026-08-31"), Beirut);

    assert_eq!(austin.days[0].placements[0].starts_at.to_rfc3339(), "2026-08-31T08:00:00-05:00");
    assert_eq!(beirut.days[0].placements[0].starts_at.to_rfc3339(), "2026-08-31T08:00:00+03:00");
    // Same wall clock, different instants — the opposite of a pinned item.
    assert_ne!(
        austin.days[0].placements[0].starts_at,
        beirut.days[0].placements[0].starts_at
    );
    assert!(!austin.days[0].placements[0].foreign);
}

#[test]
fn unknown_zone_falls_back_to_systime_and_reports() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "ghost", "mon", "09:00", "10:00", "2026-08-01", Some("Mars/Olympus"));

    let week = get_week(&db, d("2026-08-31"), Chicago);

    // Silent omission is the worst failure mode: render it, and say so.
    assert_eq!(week.days[0].placements.len(), 1, "rule must still render");
    assert_eq!(week.days[0].placements[0].starts_at.to_rfc3339(), "2026-08-31T09:00:00-05:00");
    assert_eq!(week.diagnostics.len(), 1);
    assert!(
        week.diagnostics[0].message.contains("Mars/Olympus"),
        "diagnostic should name the bad zone, got: {}",
        week.diagnostics[0].message
    );
    assert_eq!(week.diagnostics[0].item_id.as_deref(), Some("ghost"));
}

fn set_except(db: &Db, id: &str, csv: &str) {
    db.conn
        .execute("UPDATE recurrence SET except_on = ?2 WHERE item_id = ?1", rusqlite::params![id, csv])
        .unwrap();
}

#[test]
fn except_on_suppresses_that_date_only() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon,wed", "09:00", "10:15", "2026-08-01", None);
    set_except(&db, "math210", "2026-09-02"); // the Wednesday

    let week = get_week(&db, d("2026-08-31"), Chicago);

    let counts: Vec<usize> = week.days.iter().map(|day| day.placements.len()).collect();
    assert_eq!(counts, vec![1, 0, 0, 0, 0, 0, 0], "Wednesday excepted, Monday untouched");
    // A cancellation is a normal edit, not a problem to report.
    assert!(week.diagnostics.is_empty(), "excepting a date is not a diagnostic");
}

#[test]
fn spring_forward_gap_shifts_to_first_valid_instant() {
    let db = Db::open_in_memory().unwrap();
    // 2026-03-08 is the US spring-forward Sunday: 02:00 jumps to 03:00, so
    // 02:30 that morning does not exist.
    add_recurring(&db, "early", "sun", "02:30", "03:30", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-03-02"), Chicago);
    let sunday = &week.days[6];

    assert_eq!(sunday.date, d("2026-03-08"));
    assert_eq!(sunday.placements.len(), 1, "a nonexistent local time must not delete the item");
    assert_eq!(sunday.placements[0].starts_at.to_rfc3339(), "2026-03-08T03:00:00-05:00");
    assert!(
        week.diagnostics.iter().any(|x| x.message.contains("does not exist")),
        "the shift must be reported, got: {:?}",
        week.diagnostics
    );
}

#[test]
fn fall_back_prefers_the_reading_that_matches_what_was_written() {
    let db = Db::open_in_memory().unwrap();
    // 2026-11-01 is the US fall-back Sunday: 02:00 returns to 01:00, so 01:30
    // happens twice — once at -05:00 (CDT), again at -06:00 (CST).
    // "01:30-02:30" therefore has two readings: two hours from the first 01:30,
    // or one hour from the second. It was written as an hour, so it stays one.
    add_recurring(&db, "night", "sun", "01:30", "02:30", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-10-26"), Chicago);
    let p = &week.days[6].placements[0];

    assert_eq!(p.starts_at.to_rfc3339(), "2026-11-01T01:30:00-06:00", "the second 01:30");
    assert_eq!(p.ends_at.to_rfc3339(), "2026-11-01T02:30:00-06:00");
    assert_eq!((p.ends_at - p.starts_at).num_minutes(), 60);
    assert!(
        week.diagnostics.iter().any(|x| x.message.contains("clock change")),
        "the clock change should still be reported, got: {:?}",
        week.diagnostics
    );
}

/// When both readings are consistent — an event wholly inside the repeated
/// hour — the earlier one wins, so the block sits where it first occurs.
#[test]
fn fall_back_uses_the_earlier_reading_when_both_fit() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "inside", "sun", "01:10", "01:40", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-10-26"), Chicago);
    let p = &week.days[6].placements[0];

    assert_eq!(p.starts_at.to_rfc3339(), "2026-11-01T01:10:00-05:00", "the first 01:10");
    assert_eq!((p.ends_at - p.starts_at).num_minutes(), 30);
}

/// The core guarantee of spec 5.1: for ANY database state, get_week returns
/// exactly 7 days and never panics or errors.
#[test]
fn get_week_is_total_over_corrupt_data() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "ok", "mon", "09:00", "10:00", "2026-08-01", None);
    add_recurring(&db, "bad_date", "mon", "09:00", "10:00", "not-a-date", None);
    add_recurring(&db, "bad_time", "mon", "9 oclock", "whenever", "2026-08-01", None);
    add_recurring(&db, "empty_days", "", "09:00", "10:00", "2026-08-01", None);
    add_recurring(&db, "junk_days", "funday,mon", "11:00", "12:00", "2026-08-01", None);
    add_recurring(&db, "bad_zone", "mon", "13:00", "14:00", "2026-08-01", Some(""));
    set_except(&db, "ok", "garbage,2026-09-07,,also-garbage");

    let week = get_week(&db, d("2026-08-31"), Chicago);

    assert_eq!(week.days.len(), 7, "always exactly 7 days");
    assert_eq!(week.days[0].date, d("2026-08-31"));
    assert_eq!(week.days[6].date, d("2026-09-06"));

    // The healthy rules still render: "ok", "junk_days" (partially readable),
    // and "bad_zone" (degraded to systime).
    let ids: Vec<&str> = week.days[0].placements.iter().map(|p| p.item_id.as_str()).collect();
    assert_eq!(ids, vec!["ok", "junk_days", "bad_zone"], "good rules survive bad neighbours");

    // Every dropped or degraded rule is accounted for rather than vanishing.
    for id in ["bad_date", "bad_time", "bad_zone"] {
        assert!(
            week.diagnostics.iter().any(|x| x.item_id.as_deref() == Some(id)),
            "no diagnostic for {id}: {:?}", week.diagnostics
        );
    }
}

#[test]
fn rule_with_no_usable_weekdays_reports_itself() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "nowhere", "", "09:00", "10:00", "2026-08-01", None);
    add_recurring(&db, "nonsense", "funday,caturday", "09:00", "10:00", "2026-08-01", None);

    let week = get_week(&db, d("2026-08-31"), Chicago);

    assert!(week.days.iter().all(|day| day.placements.is_empty()));
    // Generating nothing forever is indistinguishable from being lost unless
    // it is reported.
    for id in ["nowhere", "nonsense"] {
        assert!(
            week.diagnostics.iter().any(|x| x.item_id.as_deref() == Some(id)),
            "no diagnostic for {id}: {:?}", week.diagnostics
        );
    }
}

#[test]
fn end_before_start_is_skipped_and_reported() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "backwards", "mon", "14:00", "09:00", "2026-08-01", None);
    add_recurring(&db, "zero_length", "mon", "14:00", "14:00", "2026-08-01", None);

    let week = get_week(&db, d("2026-08-31"), Chicago);

    assert!(week.days[0].placements.is_empty(), "a negative or zero span is not renderable");
    for id in ["backwards", "zero_length"] {
        assert!(
            week.diagnostics.iter().any(|x| x.item_id.as_deref() == Some(id)),
            "no diagnostic for {id}: {:?}", week.diagnostics
        );
    }
}

fn set_until(db: &Db, id: &str, until: &str) {
    db.conn
        .execute("UPDATE recurrence SET until_date = ?2 WHERE item_id = ?1", rusqlite::params![id, until])
        .unwrap();
}

#[test]
fn until_date_bounds_the_series() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "term", "mon,wed,fri", "09:00", "10:00", "2026-08-01", None);
    set_until(&db, "term", "2026-09-02"); // ends on the Wednesday

    let week = get_week(&db, d("2026-08-31"), Chicago);

    let counts: Vec<usize> = week.days.iter().map(|day| day.placements.len()).collect();
    assert_eq!(counts, vec![1, 0, 1, 0, 0, 0, 0], "Friday is past until_date");
    assert!(week.diagnostics.is_empty(), "an ended series is not a problem");
}

#[test]
fn until_before_from_is_skipped_and_reported() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "impossible", "mon", "09:00", "10:00", "2026-08-01", None);
    set_until(&db, "impossible", "2026-07-01"); // before it starts

    let week = get_week(&db, d("2026-08-31"), Chicago);

    assert!(week.days.iter().all(|day| day.placements.is_empty()));
    assert!(
        week.diagnostics.iter().any(|x| x.item_id.as_deref() == Some("impossible")),
        "contradictory range must be reported, got: {:?}", week.diagnostics
    );
}

#[test]
fn overlapping_rules_both_render_in_start_order() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "later", "mon", "09:30", "10:30", "2026-08-01", None);
    add_recurring(&db, "earlier", "mon", "09:00", "10:00", "2026-08-01", None);

    let week = get_week(&db, d("2026-08-31"), Chicago);

    // Double-booking is a real thing that happens; it is never an error and
    // neither item may be dropped.
    let ids: Vec<&str> = week.days[0].placements.iter().map(|p| p.item_id.as_str()).collect();
    assert_eq!(ids, vec!["earlier", "later"], "sorted by start, both present");
    assert!(week.diagnostics.is_empty(), "an overlap is not a diagnostic");
}

fn add_item(db: &Db, id: &str) {
    db.conn.execute(
        "INSERT INTO items (id, title, created_at) VALUES (?1, ?1, '2026-08-01T00:00:00Z')",
        rusqlite::params![id],
    ).unwrap();
}

/// Dropping a task onto the grid creates a one-off placement for the same
/// item — not a copy of it. Spec 3.1.
#[test]
fn a_one_off_placement_appears_on_its_day() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "pset");
    ms_core::add_placement(
        &db, "pset", "2026-09-01T13:00:00-05:00", "2026-09-01T15:00:00-05:00",
    ).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    let counts: Vec<usize> = week.days.iter().map(|day| day.placements.len()).collect();
    assert_eq!(counts, vec![0, 1, 0, 0, 0, 0, 0]);

    let p = &week.days[1].placements[0];
    assert_eq!(p.item_id, "pset");
    assert_eq!(p.starts_at.format("%H:%M").to_string(), "13:00");
    assert_eq!(p.ends_at.format("%H:%M").to_string(), "15:00");
    assert_eq!(p.origin, ms_core::Origin::OneOff);
}

#[test]
fn one_offs_and_repeats_share_the_day_in_start_order() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "class", "tue", "09:00", "10:00", "2026-08-01", None);
    add_item(&db, "pset");
    ms_core::add_placement(
        &db, "pset", "2026-09-01T08:00:00-05:00", "2026-09-01T08:30:00-05:00",
    ).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    let ids: Vec<&str> = week.days[1].placements.iter().map(|p| p.item_id.as_str()).collect();
    assert_eq!(ids, vec!["pset", "class"], "sorted by start, both present");
}

#[test]
fn deleting_the_item_removes_its_one_offs() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "pset");
    ms_core::add_placement(
        &db, "pset", "2026-09-01T13:00:00-05:00", "2026-09-01T15:00:00-05:00",
    ).unwrap();
    db.conn.execute("DELETE FROM items WHERE id = 'pset'", []).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert!(week.days.iter().all(|day| day.placements.is_empty()));
}

#[test]
fn a_placement_with_an_unreadable_time_is_reported_not_dropped_silently() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "pset");
    db.conn.execute(
        "INSERT INTO placements (id, item_id, starts_at, ends_at, origin)
         VALUES ('p1', 'pset', 'whenever', 'later', 'oneoff')", []).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert_eq!(week.days.len(), 7);
    assert!(!week.diagnostics.is_empty(), "must say it could not be placed");
}
