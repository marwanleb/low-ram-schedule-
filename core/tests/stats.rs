use chrono::{DateTime, TimeZone, Utc};
use ms_core::{stats, timer_start, timer_stop, Db};

fn t(day: u32, h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, day, h, m, 0).unwrap()
}

/// An item with an estimate, worked for `actual_min`, then finished.
fn worked(db: &Db, id: &str, tags: &str, estimate_min: u32, actual_min: i64, day: u32) {
    db.conn.execute(
        "INSERT INTO items (id, title, tags, estimate_min, created_at)
         VALUES (?1, ?1, ?2, ?3, '2026-08-01T00:00:00Z')",
        rusqlite::params![id, tags, estimate_min],
    ).unwrap();
    let s = timer_start(db, id, None, t(day, 9, 0)).unwrap().started;
    timer_stop(db, &s.id, t(day, 9, 0) + chrono::Duration::minutes(actual_min)).unwrap();
}

/// The decision in spec 5.6: the median of each item's ratio, NOT the ratio of
/// the summed totals. One outsized job must not redefine a typical day.
#[test]
fn one_huge_overrun_does_not_swamp_the_typical_case() {
    let db = Db::open_in_memory().unwrap();
    for i in 0..20 {
        worked(&db, &format!("small{i}"), "", 30, 30, 1); // dead on
    }
    worked(&db, "epic", "", 60, 600, 2); // ten times over

    let s = stats(&db, None);

    assert_eq!(s.n, 21);
    // Ratio of totals would be 1200/660 = +82%. The median says what a normal
    // task actually costs.
    assert_eq!(s.pct_over, Some(0), "median of per-item ratios, not of totals");
}

#[test]
fn the_ratio_reads_as_a_sentence() {
    let db = Db::open_in_memory().unwrap();
    for i in 0..5 {
        worked(&db, &format!("m{i}"), "math", 60, 78, 1); // 30% over
    }

    let s = stats(&db, Some("math"));

    assert_eq!(s.n, 5);
    assert_eq!(s.pct_over, Some(30));
    assert_eq!(s.phrase.as_deref(), Some("30% longer"));
    assert!(s.confident);
    assert_eq!(s.tag.as_deref(), Some("math"));
}

#[test]
fn finishing_early_reads_as_shorter() {
    let db = Db::open_in_memory().unwrap();
    for i in 0..5 {
        worked(&db, &format!("q{i}"), "", 60, 30, 1);
    }

    let s = stats(&db, None);
    assert_eq!(s.pct_over, Some(-50));
    assert_eq!(s.phrase.as_deref(), Some("50% shorter"));
}

/// Below five samples the number is noise dressed as authority, so it is
/// withheld rather than hedged.
#[test]
fn too_few_samples_withholds_the_phrase() {
    let db = Db::open_in_memory().unwrap();
    for i in 0..3 {
        worked(&db, &format!("m{i}"), "", 60, 120, 1);
    }

    let s = stats(&db, None);
    assert_eq!(s.n, 3);
    assert_eq!(s.pct_over, Some(100), "still computed");
    assert_eq!(s.phrase, None, "but not stated");
    assert!(!s.confident);
}

#[test]
fn nothing_measured_yet_says_nothing() {
    let db = Db::open_in_memory().unwrap();
    let s = stats(&db, None);
    assert_eq!(s.n, 0);
    assert_eq!(s.pct_over, None);
    assert_eq!(s.phrase, None);
}

/// A quarantined session carries a wrong elapsed time and must never reach the
/// number. Spec 5.5.
#[test]
fn quarantined_sessions_are_excluded() {
    let db = Db::open_in_memory().unwrap();
    for i in 0..5 {
        worked(&db, &format!("m{i}"), "", 60, 60, 1);
    }
    // A sixth item whose timer was left running overnight and swept.
    db.conn.execute(
        "INSERT INTO items (id, title, tags, estimate_min, created_at)
         VALUES ('forgot', 'forgot', '', 60, '2026-08-01T00:00:00Z')", []).unwrap();
    let s = timer_start(&db, "forgot", None, t(3, 1, 0)).unwrap().started;
    ms_core::sweep_stale_sessions(&db, t(3, 20, 0)).unwrap();
    let _ = s;

    let st = stats(&db, None);
    assert_eq!(st.n, 5, "the quarantined item contributes nothing");
    assert_eq!(st.pct_over, Some(0));
}
