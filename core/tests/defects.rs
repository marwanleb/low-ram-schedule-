use chrono::NaiveDate;
use chrono_tz::America::Chicago;
use ms_core::{get_week, Db};

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn add_recurring(db: &Db, id: &str, byday: &str, start: &str, end: &str, from: &str, tz: Option<&str>) {
    db.conn.execute(
        "INSERT INTO items (id, title, created_at) VALUES (?1, ?2, '2026-08-01T00:00:00Z')",
        rusqlite::params![id, id],
    ).unwrap();
    db.conn.execute(
        "INSERT INTO recurrence (item_id, byday, start_time, end_time, tz, from_date)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, byday, start, end, tz, from],
    ).unwrap();
}

// A: deleting an item should not leave a rule that renders forever.
#[test]
fn probe_a_deleted_item_stops_rendering() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "gone", "mon", "09:00", "10:00", "2026-08-01", None);
    db.conn.execute("DELETE FROM items WHERE id = 'gone'", []).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert!(week.days[0].placements.is_empty(), "orphaned rule still renders");
}

// B1: an event spanning the fall-back hour must keep BOTH its length and the
// times it was written with. Getting only one of the two is what the first
// attempt did: it held the duration but rendered "01:30-01:30".
#[test]
fn probe_b1_fall_back_keeps_both_duration_and_labels() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "night", "sun", "01:30", "02:30", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-10-26"), Chicago);
    let p = &week.days[6].placements[0];

    let mins = (p.ends_at - p.starts_at).num_minutes();
    assert_eq!(mins, 60, "01:30-02:30 is an hour, got {mins} min");
    assert_eq!(p.starts_at.format("%H:%M").to_string(), "01:30");
    assert_eq!(p.ends_at.format("%H:%M").to_string(), "02:30", "the tile must read what was written");
}

// The mirror case: an event ENDING inside the repeated hour.
#[test]
fn probe_b3_event_ending_in_the_repeated_hour() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "late", "sun", "00:30", "01:30", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-10-26"), Chicago);
    let p = &week.days[6].placements[0];

    assert_eq!((p.ends_at - p.starts_at).num_minutes(), 60);
    assert_eq!(p.starts_at.format("%H:%M").to_string(), "00:30");
    assert_eq!(p.ends_at.format("%H:%M").to_string(), "01:30");
}

// B2: an event wholly inside the spring-forward gap must not become zero-length.
#[test]
fn probe_b2_spring_gap_not_zero_length() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "ghost", "sun", "02:00", "02:30", "2026-01-01", Some("America/Chicago"));

    let week = get_week(&db, d("2026-03-02"), Chicago);
    let p = &week.days[6].placements[0];
    let mins = (p.ends_at - p.starts_at).num_minutes();
    assert!(mins > 0, "placement collapsed to {mins} minutes");
}

// C: a non-Monday anchor must not silently produce a misaligned week.
#[test]
fn probe_c_non_monday_anchor_is_normalised() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon", "09:00", "10:00", "2026-08-01", None);

    let week = get_week(&db, d("2026-09-02"), Chicago); // a Wednesday
    assert_eq!(week.days[0].date, d("2026-08-31"), "week should start on Monday");
}

// D: an unreadable store must report, not return a silently empty week.
#[test]
fn probe_d_unreadable_store_reports() {
    let db = Db::open_in_memory().unwrap();
    db.conn.execute("DROP TABLE recurrence", []).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert_eq!(week.days.len(), 7, "still total");
    assert!(!week.diagnostics.is_empty(), "an unreadable store must be reported, not silent");
}

// E: a malformed except_on entry should be reported (spec 5.2 catalogue).
#[test]
fn probe_e_malformed_except_entry_reports() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon", "09:00", "10:00", "2026-08-01", None);
    db.conn.execute("UPDATE recurrence SET except_on = 'not-a-date' WHERE item_id='math210'", []).unwrap();

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert_eq!(week.days[0].placements.len(), 1, "rest of the rule still expands");
    assert!(!week.diagnostics.is_empty(), "malformed exception entry must be reported");
}

// F: "total for any input" includes the ends of the calendar.
#[test]
fn probe_f_extreme_anchor_does_not_panic() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon", "09:00", "10:00", "2026-08-01", None);

    let late = get_week(&db, NaiveDate::MAX, Chicago);
    assert_eq!(late.days.len(), 7);
    let early = get_week(&db, NaiveDate::MIN, Chicago);
    assert_eq!(early.days.len(), 7);
}

// G: an alias naming the same zone is not "foreign".
#[test]
fn probe_g_zone_alias_is_not_foreign() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon", "09:00", "10:00", "2026-08-01", Some("US/Central"));

    let week = get_week(&db, d("2026-08-31"), Chicago);
    let p = &week.days[0].placements[0];
    assert!(!p.foreign, "US/Central is America/Chicago; marking it foreign is a false alarm");
}

// H: a duplicated weekday must not double-book the same item.
#[test]
fn probe_h_repeated_weekday_places_once() {
    let db = Db::open_in_memory().unwrap();
    add_recurring(&db, "math210", "mon,mon,MON", "09:00", "10:00", "2026-08-01", None);

    let week = get_week(&db, d("2026-08-31"), Chicago);
    assert_eq!(week.days[0].placements.len(), 1);
}
