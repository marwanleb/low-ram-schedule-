//! Repeating once a month, on the same day of the month.
//!
//! A day a month does not have -- the 31st in September, the 30th in
//! February -- lands on that month's last day rather than skipping the month:
//! a bill due "on the 31st" is still due in a thirty-day month.

use chrono::{NaiveDate, NaiveTime};
use chrono_tz::America::Chicago;
use ms_core::compose::{line, Fields};
use ms_core::{add_from_text_in, get_days, parse, Db};

fn d(s: &str) -> NaiveDate {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
}

/// A Monday, and the 31st -- which is what makes the clamping cases bite.
fn today() -> NaiveDate {
    d("2026-08-31")
}

fn now() -> chrono::DateTime<chrono::Utc> {
    "2026-08-31T12:00:00Z".parse().unwrap()
}

/// Every date in [from, to] that has a block on it.
fn occurrences(db: &Db, from: &str, to: &str) -> Vec<NaiveDate> {
    let (mut start, end) = (d(from), d(to));
    let mut out = Vec::new();
    while start <= end {
        for day in get_days(db, start, Chicago).days {
            if day.date <= end && !day.placements.is_empty() && !out.contains(&day.date) {
                out.push(day.date);
            }
        }
        start += chrono::Duration::days(7);
    }
    out
}

#[test]
fn monthly_with_a_date_repeats_on_that_day_of_the_month() {
    let p = parse("rent monthly 01/10/2026", today());
    assert_eq!(p.title, "rent");
    assert!(p.repeats);
    assert_eq!(p.monthday, Some(1));
    assert_eq!(p.repeat_from, Some(d("2026-10-01")), "starts on the date given");
    assert!(p.byday.is_empty(), "not a weekly rule");
    assert_eq!(p.due, None, "a repeat has no single deadline");
}

#[test]
fn monthly_without_a_date_uses_todays_day_of_the_month() {
    let p = parse("water the plants monthly", today());
    assert_eq!(p.title, "water the plants");
    assert!(p.repeats);
    assert_eq!(p.monthday, Some(31));
    assert_eq!(p.repeat_from, None, "no date was given; the caller decides where it starts");
}

#[test]
fn prose_containing_the_word_is_left_alone() {
    let p = parse("read the monthly report", today());
    assert_eq!(p.title, "read the monthly report", "not at the end, so not a detail");
    assert!(!p.repeats);
}

#[test]
fn a_monthly_block_lands_once_a_month_and_not_before_it_starts() {
    let db = Db::open_in_memory().unwrap();
    add_from_text_in(&db, "rent monthly 01/10/2026 09:00", today(), now(), Chicago).unwrap();
    assert_eq!(
        occurrences(&db, "2026-09-01", "2026-12-31"),
        vec![d("2026-10-01"), d("2026-11-01"), d("2026-12-01")],
    );
}

#[test]
fn a_day_the_month_does_not_have_lands_on_its_last_day() {
    let db = Db::open_in_memory().unwrap();
    add_from_text_in(&db, "bills monthly 31/08/2026 09:00", today(), now(), Chicago).unwrap();
    assert_eq!(
        occurrences(&db, "2026-08-01", "2027-03-31"),
        vec![
            d("2026-08-31"), d("2026-09-30"), d("2026-10-31"), d("2026-11-30"),
            d("2026-12-31"), d("2027-01-31"), d("2027-02-28"), d("2027-03-31"),
        ],
    );
}

#[test]
fn monthly_round_trips_through_the_composer() {
    let f = Fields {
        title: "rent".into(),
        monthly: true,
        date: Some(d("2026-10-01")),
        at: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
        ..Fields::default()
    };
    let text = line(&f);
    assert!(text.contains("monthly"), "{text:?}");
    let p = parse(&text, today());
    assert_eq!(p.title, "rent", "{text:?}");
    assert_eq!(p.monthday, Some(1), "{text:?}");
    assert_eq!(p.repeat_from, Some(d("2026-10-01")), "{text:?}");
}

/// Contradictory, so the explicit cadence wins and the weekdays are dropped,
/// rather than producing a rule that means neither.
#[test]
fn monthly_beats_weekdays() {
    let p = parse("x every tue monthly", today());
    assert_eq!(p.monthday, Some(31));
    assert!(p.byday.is_empty(), "{:?}", p.byday);
}

/// The example the help text gives. A written-out date followed by a time
/// used to lose both the day and the time.
#[test]
fn the_help_example_is_read_whole() {
    let p = parse("rent monthly 1 oct 9am", today());
    assert_eq!(p.title, "rent");
    assert_eq!(p.monthday, Some(1));
    assert_eq!(p.at, Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()));
}
