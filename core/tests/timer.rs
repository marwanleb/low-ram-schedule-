use chrono::{DateTime, TimeZone, Utc};
use ms_core::{active_session, timer_start, timer_stop, Db};

fn t(h: u32, m: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 1, h, m, 0).unwrap()
}

fn add_item(db: &Db, id: &str) {
    db.conn.execute(
        "INSERT INTO items (id, title, created_at) VALUES (?1, ?1, '2026-08-01T00:00:00Z')",
        rusqlite::params![id],
    ).unwrap();
}

#[test]
fn a_timer_records_the_interval_it_ran_for() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "stat240");

    let started = timer_start(&db, "stat240", None, t(13, 0)).unwrap();
    assert!(started.stopped.is_none(), "nothing was running");
    assert_eq!(started.started.item_id, "stat240");
    assert_eq!(started.started.ended_at, None, "a running session has no end");

    let stopped = timer_stop(&db, &started.started.id, t(14, 30)).unwrap().unwrap();
    assert_eq!(stopped.started_at, t(13, 0));
    assert_eq!(stopped.ended_at, Some(t(14, 30)));
    assert_eq!((stopped.ended_at.unwrap() - stopped.started_at).num_minutes(), 90);
}

#[test]
fn only_one_timer_runs_and_starting_another_reports_what_it_stopped() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "stat240");
    add_item(&db, "math210");

    let first = timer_start(&db, "stat240", None, t(13, 0)).unwrap().started;
    let second = timer_start(&db, "math210", None, t(13, 45)).unwrap();

    let stopped = second.stopped.expect("the running timer must be stopped, not silently abandoned");
    assert_eq!(stopped.id, first.id);
    assert_eq!(stopped.ended_at, Some(t(13, 45)), "banked at the moment the next one began");

    let now_running = active_session(&db).expect("the new one is running");
    assert_eq!(now_running.item_id, "math210");
}

#[test]
fn a_session_outlives_the_placement_it_was_started_from() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "stat240");

    // Generated placements have no row at all; the id is derived. Spec 4.3.
    let s = timer_start(&db, "stat240", Some("plc_2026-09-01_stat240"), t(13, 0)).unwrap().started;
    let stopped = timer_stop(&db, &s.id, t(14, 0)).unwrap().unwrap();

    assert_eq!(stopped.item_id, "stat240", "the durable link survives");
    assert_eq!(stopped.placement_id.as_deref(), Some("plc_2026-09-01_stat240"));
}

/// Spec 5.5: a crash or a forgotten stop leaves a session open forever, which
/// would poison the one number the app exists to produce.
#[test]
fn sessions_left_running_too_long_are_quarantined_not_counted_or_deleted() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "stat240");
    let stale = timer_start(&db, "stat240", None, t(1, 0)).unwrap().started;

    // 13 hours later, on startup.
    let swept = ms_core::sweep_stale_sessions(&db, t(14, 0)).unwrap();

    assert_eq!(swept.len(), 1, "the stale session should be surfaced");
    assert_eq!(swept[0].id, stale.id);
    assert!(swept[0].discarded);

    // Never silently deleted: the user gets to correct it.
    let count: i64 = db.conn
        .query_row("SELECT count(*) FROM sessions WHERE id = ?1", rusqlite::params![stale.id], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "quarantined, not destroyed");

    // And it no longer looks like something is running.
    assert!(active_session(&db).is_none());
}

#[test]
fn a_session_running_a_normal_length_of_time_is_left_alone() {
    let db = Db::open_in_memory().unwrap();
    add_item(&db, "stat240");
    timer_start(&db, "stat240", None, t(9, 0)).unwrap();

    let swept = ms_core::sweep_stale_sessions(&db, t(11, 0)).unwrap();

    assert!(swept.is_empty(), "two hours is a plausible work session");
    assert!(active_session(&db).is_some(), "still running");
}
