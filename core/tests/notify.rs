//! What gets pushed, and — more importantly — what does not.

use chrono::{DateTime, Duration, NaiveDate, Utc};
use chrono_tz::America::Chicago;
use ms_core::{
    add_from_text_in, already_sent, due_soon, mark_sent, prune_sent, set_done, set_setting,
    setting, Db, Kind,
};

const LEAD: i64 = 10;

fn utc(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
}

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// Capture a line the way every front door does, at a fixed instant.
fn add(db: &Db, text: &str, today: &str, now: &str) -> ms_core::Item {
    add_from_text_in(db, text, d(today), utc(now), Chicago).unwrap()
}

fn soon(db: &Db, now: &str) -> Vec<ms_core::Notice> {
    due_soon(db, utc(now), Duration::minutes(LEAD), Chicago)
}

// Austin is UTC-5 in September, so 09:00 local is 14:00Z.

#[test]
fn a_block_inside_the_window_is_announced() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "standup on 09/09/2026 09:00", "2026-09-08", "2026-09-08T12:00:00Z");

    let out = soon(&db, "2026-09-09T13:52:00Z");
    assert_eq!(out.len(), 1, "expected one notice, got {out:?}");
    assert_eq!(out[0].title, "standup");
    assert_eq!(out[0].kind, Kind::Block);
    assert_eq!(out[0].at, utc("2026-09-09T14:00:00Z"));
}

#[test]
fn a_block_beyond_the_window_is_not() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "standup on 09/09/2026 09:00", "2026-09-08", "2026-09-08T12:00:00Z");

    assert!(soon(&db, "2026-09-09T13:30:00Z").is_empty(), "38 minutes out is not soon");
}

#[test]
fn a_block_already_begun_is_not_announced() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "standup on 09/09/2026 09:00", "2026-09-08", "2026-09-08T12:00:00Z");

    assert!(soon(&db, "2026-09-09T14:00:00Z").is_empty(), "it has started; that is not news");
    assert!(soon(&db, "2026-09-09T14:05:00Z").is_empty());
}

#[test]
fn a_recurring_class_is_announced_on_each_occurrence() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "PHYS201 every wed 10:30-12:00 @Hall 2.106", "2026-09-08", "2026-09-08T12:00:00Z");

    // 10:30 Austin is 15:30Z.
    let first = soon(&db, "2026-09-09T15:22:00Z");
    assert_eq!(first.len(), 1, "got {first:?}");
    assert_eq!(first[0].title, "PHYS201");
    assert_eq!(first[0].location.as_deref(), Some("Hall 2.106"));

    let next = soon(&db, "2026-09-16T15:22:00Z");
    assert_eq!(next.len(), 1);
    assert_ne!(first[0].key, next[0].key, "each occurrence needs its own key");
}

#[test]
fn a_ticked_occurrence_is_not_announced() {
    let db = Db::open_in_memory().unwrap();
    let item = add(&db, "PHYS201 every wed 10:30-12:00", "2026-09-08", "2026-09-08T12:00:00Z");
    set_done(&db, &item.id, true, Some(d("2026-09-09")), utc("2026-09-09T12:00:00Z")).unwrap();

    assert!(soon(&db, "2026-09-09T15:22:00Z").is_empty(), "already done this week");
    assert_eq!(soon(&db, "2026-09-16T15:22:00Z").len(), 1, "next week is still open");
}

#[test]
fn a_deadline_in_the_list_is_announced() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "lab report due 09/09/2026 17:00", "2026-09-08", "2026-09-08T12:00:00Z");

    let out = soon(&db, "2026-09-09T21:55:00Z");
    assert_eq!(out.len(), 1, "got {out:?}");
    assert_eq!(out[0].kind, Kind::Deadline);
    assert_eq!(out[0].ends, None);
}

#[test]
fn a_ticked_deadline_is_not() {
    let db = Db::open_in_memory().unwrap();
    let item = add(&db, "lab report due 09/09/2026 17:00", "2026-09-08", "2026-09-08T12:00:00Z");
    set_done(&db, &item.id, true, None, utc("2026-09-09T12:00:00Z")).unwrap();

    assert!(soon(&db, "2026-09-09T21:55:00Z").is_empty());
}

#[test]
fn the_window_reaches_across_the_week_boundary() {
    let db = Db::open_in_memory().unwrap();
    // Monday 00:05 Austin = 05:05Z Monday.
    add(&db, "early flight on 14/09/2026 00:05", "2026-09-08", "2026-09-08T12:00:00Z");

    // Sunday 23:57 Austin = Monday 04:57Z, still inside the previous week.
    let out = soon(&db, "2026-09-14T04:57:00Z");
    assert_eq!(out.len(), 1, "expanding only the current week would miss this: {out:?}");
}

#[test]
fn a_sent_notice_is_remembered_and_pruned() {
    let db = Db::open_in_memory().unwrap();
    let now = utc("2026-09-09T13:52:00Z");
    assert!(!already_sent(&db, "k"));

    mark_sent(&db, "k", now).unwrap();
    assert!(already_sent(&db, "k"));
    mark_sent(&db, "k", now).unwrap();
    assert!(already_sent(&db, "k"), "marking twice is not an error");

    assert_eq!(prune_sent(&db, now - Duration::days(1)).unwrap(), 0, "not old enough");
    assert!(already_sent(&db, "k"));
    assert_eq!(prune_sent(&db, now + Duration::days(1)).unwrap(), 1);
    assert!(!already_sent(&db, "k"));
}

#[test]
fn a_setting_round_trips() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(setting(&db, "telegram_chat"), None);
    set_setting(&db, "telegram_chat", "12345").unwrap();
    set_setting(&db, "telegram_chat", "67890").unwrap();
    assert_eq!(setting(&db, "telegram_chat").as_deref(), Some("67890"));
}

#[test]
fn nothing_scheduled_means_nothing_to_say() {
    let db = Db::open_in_memory().unwrap();
    add(&db, "renew parking", "2026-09-08", "2026-09-08T12:00:00Z");
    assert!(soon(&db, "2026-09-09T13:52:00Z").is_empty(), "an undated task has no moment");
}
