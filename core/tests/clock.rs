//! 12-hour display, and that everything displayed still reads back.

use chrono::{NaiveDate, NaiveTime};
use ms_core::clock::{range12, time12};

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

#[test]
fn the_ends_of_the_day_are_twelve_not_zero() {
    assert_eq!(time12(t(0, 0)), "12am");
    assert_eq!(time12(t(0, 30)), "12:30am");
    assert_eq!(time12(t(12, 0)), "12pm");
    assert_eq!(time12(t(12, 5)), "12:05pm");
    assert_eq!(time12(t(23, 59)), "11:59pm");
}

#[test]
fn whole_hours_drop_the_minutes() {
    assert_eq!(time12(t(9, 0)), "9am");
    assert_eq!(time12(t(15, 0)), "3pm");
    assert_eq!(time12(t(9, 30)), "9:30am");
}

#[test]
fn a_range_says_the_meridiem_once_when_it_can() {
    assert_eq!(range12(t(14, 0), t(17, 0)), "2\u{2013}5pm");
    assert_eq!(range12(t(9, 0), t(10, 15)), "9\u{2013}10:15am");
    assert_eq!(range12(t(10, 30), t(12, 0)), "10:30am\u{2013}12pm");
    assert_eq!(range12(t(11, 0), t(13, 0)), "11am\u{2013}1pm");
    assert_eq!(range12(t(12, 0), t(13, 30)), "12\u{2013}1:30pm");
    assert_eq!(range12(t(23, 30), t(0, 15)), "11:30pm\u{2013}12:15am");
}

/// Anything shown must still be something the parser reads back as itself,
/// since the form composes its line with these.
#[test]
fn every_minute_of_the_day_reads_back_as_itself() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
    for h in 0..24 {
        for m in 0..60 {
            let shown = time12(t(h, m));
            let p = ms_core::parse(&format!("x {shown}"), today);
            assert_eq!(p.at, Some(t(h, m)), "{shown:?}");
        }
    }
}
