//! The composer is the inverse of the parser, so the test that matters is that
//! going one way and back changes nothing.

use chrono::{NaiveDate, NaiveTime};
use ms_core::compose::{line, Fields, Kind};
use ms_core::parse;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 8, 31).unwrap()
}

fn t(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

/// Everything a round trip must preserve. Compared field by field so a failure
/// names the field rather than dumping two structs.
fn assert_round_trip(f: &Fields) {
    let text = line(f);
    let p = parse(&text, today());
    let ctx = format!("composed {text:?}");

    assert_eq!(p.title, f.title, "title — {ctx}");
    // A repeat has no single due date; the parser drops one and so must the
    // composer, or the preview would show a date that does nothing.
    let expected_date = if f.repeat.is_empty() { f.date } else { None };
    assert_eq!(p.due, expected_date, "date — {ctx}");
    assert_eq!(p.estimate_min, f.estimate_min, "estimate — {ctx}");
    assert_eq!(p.priority, f.priority, "priority — {ctx}");
    assert_eq!(p.location, f.place, "place — {ctx}");
    assert_eq!(p.repeats, !f.repeat.is_empty(), "repeats — {ctx}");
    assert_eq!(p.byday, f.repeat, "repeat days — {ctx}");
    assert_eq!(p.location, f.place, "place — {ctx}");

    match f.span_end {
        Some(end) => assert_eq!(p.span, f.at.map(|a| (a, end)), "span — {ctx}"),
        None => assert_eq!(p.at, f.at, "time — {ctx}"),
    }
    if let Some(tag) = &f.tag {
        assert!(p.tags.contains(tag), "tag {tag} missing — {ctx}");
    }
}

fn base(title: &str) -> Fields {
    Fields { title: title.into(), ..Fields::default() }
}

#[test]
fn a_bare_title_composes_to_itself() {
    let f = base("renew parking");
    assert_eq!(line(&f), "renew parking");
    assert_round_trip(&f);
}

#[test]
fn a_task_with_a_deadline() {
    let f = Fields { date: Some(d(2026, 9, 4)), kind: Kind::Task, ..base("math hw") };
    assert_round_trip(&f);
}

#[test]
fn a_block_with_a_time() {
    let f = Fields {
        date: Some(d(2026, 9, 4)),
        at: Some(t(17, 0)),
        kind: Kind::Block,
        ..base("standup")
    };
    let text = line(&f);
    assert!(text.contains(" on "), "a block says on: {text:?}");
    assert_round_trip(&f);
}

#[test]
fn a_block_with_a_range() {
    let f = Fields {
        date: Some(d(2026, 9, 4)),
        at: Some(t(9, 0)),
        span_end: Some(t(10, 15)),
        kind: Kind::Block,
        ..base("seminar")
    };
    assert_round_trip(&f);
}

#[test]
fn a_weekly_commitment() {
    let f = Fields {
        repeat: vec!["mon".into(), "wed".into()],
        at: Some(t(10, 30)),
        span_end: Some(t(12, 0)),
        place: Some("Hall 2.106".into()),
        ..base("PHYS201")
    };
    let text = line(&f);
    assert!(text.contains("every"), "a repeat says every: {text:?}");
    assert_round_trip(&f);
}

#[test]
fn estimates_have_one_spelling_each() {
    assert!(line(&Fields { estimate_min: Some(120), ..base("x") }).contains("~2h"));
    assert!(line(&Fields { estimate_min: Some(90), ..base("x") }).contains("~90m"));
    assert!(line(&Fields { estimate_min: Some(30), ..base("x") }).contains("~30m"));
    for m in [15u32, 30, 45, 60, 90, 120, 180, 240, 45, 480] {
        assert_round_trip(&Fields { estimate_min: Some(m), ..base("thing") });
    }
}

#[test]
fn a_tag_and_a_place_survive_together() {
    let f = Fields {
        tag: Some("math".into()),
        place: Some("Hall 2.106".into()),
        ..base("problem set")
    };
    assert_round_trip(&f);
}

#[test]
fn priority_leads() {
    let f = Fields { priority: true, ..base("renew parking") };
    assert!(line(&f).starts_with("!!"), "{:?}", line(&f));
    assert_round_trip(&f);
}

/// The composer must not emit something the parser then reads as a detail of
/// the title. A title that itself ends in a date-shaped word is the risk.
#[test]
fn a_title_is_never_swallowed() {
    for title in ["meet Sarah about the March report", "read the daily news", "call him on 5"] {
        let f = Fields { date: Some(d(2026, 9, 4)), kind: Kind::Task, ..base(title) };
        assert_round_trip(&f);
    }
}

/// Seeded, so a failure is reproducible.
#[test]
fn round_trips_over_generated_fields() {
    let mut seed = 0x5eed_1234u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let titles = ["gym", "math hw", "renew parking", "call the bank", "problem set"];
    let places = [None, Some("Hall 2.106"), Some("the corner cafe")];
    let tags = [None, Some("math"), Some("work")];

    for _ in 0..400 {
        let r = next();
        let has_date = r % 2 == 0;
        let has_time = (r >> 1) % 2 == 0;
        let has_span = has_time && (r >> 2) % 2 == 0;
        let repeats = (r >> 3) % 4 == 0;
        let at = t(((r >> 4) % 24) as u32, if (r >> 9) % 2 == 0 { 0 } else { 30 });

        let f = Fields {
            title: titles[(r >> 10) as usize % titles.len()].into(),
            priority: (r >> 14) % 8 == 0,
            repeat: if repeats { vec!["tue".into()] } else { vec![] },
            // Monthly has its own round trip in tests/monthly.rs.
            monthly: false,
            kind: if (r >> 15) % 2 == 0 { Kind::Task } else { Kind::Block },
            date: (has_date && !repeats).then(|| d(2026, 9, 1 + ((r >> 16) % 28) as u32)),
            at: has_time.then_some(at),
            span_end: has_span.then(|| at + chrono::Duration::minutes(45)),
            estimate_min: ((r >> 20) % 3 == 0).then(|| 15 * (1 + ((r >> 22) % 8) as u32)),
            tag: tags[(r >> 25) as usize % tags.len()].map(str::to_string),
            place: places[(r >> 27) as usize % places.len()].map(str::to_string),
        };
        assert_round_trip(&f);
    }
}

/// The form sends these across the Tauri boundary as JSON, with dates and
/// times as the HTML inputs produce them. If that shape does not deserialize,
/// the preview breaks with an error nobody sees.
#[test]
fn fields_deserialize_from_what_the_form_sends() {
    let json = r#"{
        "title": "math hw",
        "priority": false,
        "repeat": ["mon","wed"],
        "kind": "block",
        "date": "2026-09-11",
        "at": "17:00",
        "span_end": "18:30",
        "estimate_min": 90,
        "tag": "math",
        "place": "Hall 2.106"
    }"#;
    let f: Fields = serde_json::from_str(json).expect("the form's shape must deserialize");
    assert_eq!(f.date, Some(d(2026, 9, 11)));
    assert_eq!(f.at, Some(t(17, 0)));
    assert_eq!(f.span_end, Some(t(18, 30)));
    assert_eq!(f.kind, Kind::Block);
    assert_round_trip(&f);
}

/// Empty fields arrive as null, and a title alone must still work.
#[test]
fn a_mostly_empty_form_deserializes() {
    let json = r#"{"title":"renew parking","repeat":[],"kind":"task",
                   "date":null,"at":null,"span_end":null,
                   "estimate_min":null,"tag":null,"place":null,"priority":false}"#;
    let f: Fields = serde_json::from_str(json).unwrap();
    assert_eq!(line(&f), "renew parking");
}
