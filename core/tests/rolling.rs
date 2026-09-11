//! The window shows yesterday, today and the five after, so seven days can
//! start on any day of the week.

use chrono::NaiveDate;
use chrono_tz::America::Chicago;
use ms_core::{add_from_text_in, get_days, get_week, Db};

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-08-31T12:00:00Z".parse().unwrap()
}

#[test]
fn a_window_starts_exactly_where_it_is_asked_to() {
    let db = Db::open_in_memory().unwrap();
    let week = get_days(&db, d("2026-09-09"), Chicago); // a Wednesday
    let dates: Vec<NaiveDate> = week.days.iter().map(|x| x.date).collect();
    assert_eq!(dates.len(), 7);
    assert_eq!(dates[0], d("2026-09-09"), "no snapping back to Monday");
    for pair in dates.windows(2) {
        assert_eq!(pair[1], pair[0].succ_opt().unwrap(), "consecutive days: {dates:?}");
    }
    assert_eq!(week.anchor, d("2026-09-09"));
}

#[test]
fn a_repeat_lands_on_its_own_days_whatever_the_window_starts_on() {
    let db = Db::open_in_memory().unwrap();
    add_from_text_in(&db, "PHYS201 every mon wed 10:30-12:00", d("2026-08-31"), now(), Chicago)
        .unwrap();
    // Tuesday 8 Sep to Monday 14 Sep: Wednesday is second, Monday is last.
    let week = get_days(&db, d("2026-09-08"), Chicago);
    let counts: Vec<usize> = week.days.iter().map(|x| x.placements.len()).collect();
    assert_eq!(counts, vec![0, 1, 0, 0, 0, 0, 1]);
}

/// The CLI, the bot and the notifier all still want calendar weeks.
#[test]
fn get_week_still_snaps_to_monday() {
    let db = Db::open_in_memory().unwrap();
    assert_eq!(get_week(&db, d("2026-09-09"), Chicago).days[0].date, d("2026-09-07"));
}

#[test]
fn a_window_at_either_end_of_the_calendar_is_clamped_not_a_panic() {
    let db = Db::open_in_memory().unwrap();
    let late = get_days(&db, NaiveDate::MAX, Chicago);
    assert_eq!(late.days.len(), 7);
    assert!(!late.diagnostics.is_empty(), "the clamp is reported, not silent");
    assert_eq!(get_days(&db, NaiveDate::MIN, Chicago).days.len(), 7);
}
