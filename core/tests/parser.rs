use chrono::NaiveDate;
use ms_core::parse;

fn today() -> NaiveDate {
    // A Monday, fixed so every date-relative case is deterministic.
    NaiveDate::from_ymd_opt(2026, 8, 31).unwrap()
}

#[test]
fn trailing_tokens_are_stripped_from_the_title() {
    let p = parse("math hw fri 5pm ~2h", today());
    assert_eq!(p.title, "math hw");
}

/// The rule that makes the parser usable: scanning stops at the first token it
/// does not recognise, so prose containing date-shaped words survives intact.
#[test]
fn prose_with_a_date_shaped_word_is_left_alone() {
    let p = parse("meet Sarah about the March report", today());
    assert_eq!(p.title, "meet Sarah about the March report");
    assert!(p.consumed.is_empty());
}

#[test]
fn recognised_tokens_yield_their_values() {
    let p = parse("math hw fri 5pm ~2h", today());
    assert_eq!(p.estimate_min, Some(120), "~2h is 120 minutes");
    assert_eq!(p.at, Some(chrono::NaiveTime::from_hms_opt(17, 0, 0).unwrap()), "5pm is 17:00");
    assert_eq!(p.byday, vec!["fri"]);
}

#[test]
fn estimates_accept_minutes_and_fractions() {
    assert_eq!(parse("x ~90m", today()).estimate_min, Some(90));
    assert_eq!(parse("x ~1.5h", today()).estimate_min, Some(90));
    assert_eq!(parse("x ~30m", today()).estimate_min, Some(30));
}

/// Spec 6: capture must never be lost to a syntax mistake.
#[test]
fn parsing_never_fails_and_never_loses_text() {
    for input in [
        "",
        "   ",
        "renew parking",
        "!!!",
        "~",
        "~~~2h",
        ":::",
        "🙂 buy 🙂",
        "a b c d e f g h i j k l m n o p",
        "fri",
        "~2h",
    ] {
        let p = parse(input, today());
        // Whatever is not recognised survives as text. The one deliberate
        // removal is a leading `!!` priority marker.
        let round_trip: Vec<&str> = p
            .title
            .split_whitespace()
            .chain(p.consumed.iter().map(|s| s.as_str()))
            .collect();
        let mut expected: Vec<&str> = input.split_whitespace().collect();
        if p.priority {
            expected[0] = expected[0].trim_start_matches('!');
            expected.retain(|w| !w.is_empty());
        }
        assert_eq!(round_trip, expected, "input {input:?} lost or reordered words");
    }
}

#[test]
fn a_bare_note_keeps_every_word() {
    let p = parse("renew parking", today());
    assert_eq!(p.title, "renew parking");
    assert!(p.consumed.is_empty());
    assert_eq!(p.estimate_min, None);
    assert!(p.byday.is_empty());
}

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn relative_days_resolve_against_today() {
    // today() is Monday 2026-08-31
    assert_eq!(parse("call mom today", today()).due, Some(ymd(2026, 8, 31)));
    assert_eq!(parse("call mom tomorrow", today()).due, Some(ymd(2026, 9, 1)));
    assert_eq!(parse("call mom tomorrow", today()).title, "call mom");
}

/// The counter-case for the right-to-left rule: a real date word buried in
/// prose must not be eaten.
#[test]
fn a_date_word_inside_prose_is_not_parsed() {
    let p = parse("buy milk for tomorrow's breakfast", today());
    assert_eq!(p.title, "buy milk for tomorrow's breakfast");
    assert_eq!(p.due, None);
}

#[test]
fn a_single_weekday_is_the_next_such_day() {
    assert_eq!(parse("math hw fri", today()).due, Some(ymd(2026, 9, 4)));
    // Today is Monday, so "mon" means today rather than a week away.
    assert_eq!(parse("standup mon", today()).due, Some(ymd(2026, 8, 31)));
}

#[test]
fn several_weekdays_mean_a_repeat_not_a_due_date() {
    let p = parse("gym mon wed fri ~1h", today());
    assert_eq!(p.title, "gym");
    assert_eq!(p.byday, vec!["mon", "wed", "fri"]);
    assert_eq!(p.due, None, "a repeating item has no single due date");
    assert_eq!(p.estimate_min, Some(60));
}

#[test]
fn absolute_dates_parse_in_both_forms() {
    assert_eq!(parse("pay rent 2026-09-15", today()).due, Some(ymd(2026, 9, 15)));

    // Two-part dates read month/day, matching the three-part forms and this
    // machine's locale. `1/9` is 9 January, resolved to the next one.
    assert_eq!(parse("pay rent 1/9", today()).due, Some(ymd(2027, 1, 9)));
    // And where the first number cannot be a month, it settles itself.
    assert_eq!(parse("pay rent 25/12", today()).due, Some(ymd(2026, 12, 25)));
}

/// Recurrence is stated, not guessed. One weekday plus `every` repeats; the
/// same weekday without it is a due date.
#[test]
fn every_states_recurrence_explicitly() {
    let weekly = parse("trash every tue 20:00", today());
    assert_eq!(weekly.title, "trash");
    assert!(weekly.repeats, "`every` means it repeats");
    assert_eq!(weekly.byday, vec!["tue"]);
    assert_eq!(weekly.due, None, "a repeat has no single due date");

    let once = parse("math hw fri", today());
    assert!(!once.repeats);
    assert_eq!(once.due, Some(ymd(2026, 9, 4)));
}

#[test]
fn every_works_with_several_weekdays_and_reads_naturally() {
    let p = parse("gym every mon wed fri ~1h", today());
    assert_eq!(p.title, "gym");
    assert!(p.repeats);
    assert_eq!(p.byday, vec!["mon", "wed", "fri"]);
    assert_eq!(p.estimate_min, Some(60));
}

/// `every` only counts as a keyword where tokens are already being read, so
/// prose ending in the word is untouched.
#[test]
fn every_inside_prose_is_not_a_keyword() {
    let p = parse("water the plants every day", today());
    assert_eq!(p.title, "water the plants every day");
    assert!(!p.repeats);
}

#[test]
fn a_time_range_makes_it_a_block_not_a_list_item() {
    let p = parse("MATH210 every mon wed 9:00-10:15", today());
    assert_eq!(p.title, "MATH210");
    assert_eq!(
        p.span,
        Some((
            chrono::NaiveTime::from_hms_opt(9, 0, 0).unwrap(),
            chrono::NaiveTime::from_hms_opt(10, 15, 0).unwrap()
        ))
    );
    // Typing a span means scheduling, not listing. Spec 3.1.
    assert!(!p.listed, "a block should not clutter the to-do list");
    assert!(p.repeats);
}

#[test]
fn anything_without_a_span_is_listed() {
    assert!(parse("math hw fri ~2h", today()).listed);
    assert!(parse("renew parking", today()).listed);
    // A point time is a to-do with a time, not a block.
    assert!(parse("trash every tue 20:00", today()).listed);
}

#[test]
fn tags_locations_and_priority_are_extracted() {
    let p = parse("problem set fri ~2h #math", today());
    assert_eq!(p.title, "problem set");
    assert_eq!(p.tags, vec!["math"]);
    assert_eq!(p.category, None, "math is not one of the four colour categories");

    let c = parse("swim every tue 18:00-19:00 #body", today());
    assert_eq!(c.category.as_deref(), Some("body"), "a known category colours the item");
    assert_eq!(c.tags, vec!["body"], "and is still a tag");

    let l = parse("MATH210 every mon 9:00-10:15 @Hall 1.204", today());
    assert_eq!(l.title, "MATH210");
    assert_eq!(l.location.as_deref(), Some("Hall 1.204"));

    let bang = parse("!! renew parking", today());
    assert_eq!(bang.title, "renew parking");
    assert!(bang.priority);
}

#[test]
fn daily_repeats_on_every_weekday() {
    let p = parse("water the plants daily", today());
    assert_eq!(p.title, "water the plants");
    assert!(p.repeats);
    assert_eq!(p.byday, vec!["mon", "tue", "wed", "thu", "fri", "sat", "sun"]);
    assert_eq!(p.due, None);

    let v = parse("vitamins daily 08:00", today());
    assert_eq!(v.title, "vitamins");
    assert!(v.repeats);
    assert_eq!(v.at, Some(chrono::NaiveTime::from_hms_opt(8, 0, 0).unwrap()));
}

/// `daily` is safe to treat as a keyword because it rarely ends a sentence.
/// The bare word `day` is not, which is why it stays prose.
#[test]
fn daily_does_not_eat_ordinary_sentences() {
    assert_eq!(parse("read the daily news", today()).title, "read the daily news");
    assert_eq!(parse("water the plants every day", today()).title, "water the plants every day");
}

/// Fixed vs floating is stated with a marker and defaults to fixed: a class
/// that follows you to the wrong hour is worse than a gym slot that does.
#[test]
fn items_are_fixed_to_their_zone_unless_told_to_float() {
    assert!(parse("MATH210 every mon 9:00-10:15", today()).pinned, "fixed by default");
    assert!(parse("renew parking", today()).pinned);

    for text in ["wake up daily 08:00 #floating", "wake up daily 08:00 #systime"] {
        let p = parse(text, today());
        assert!(!p.pinned, "{text:?} should follow the machine");
        assert_eq!(p.title, "wake up");
    }

    let explicit = parse("MATH210 every mon 9:00-10:15 #fixed", today());
    assert!(explicit.pinned);
    assert_eq!(explicit.title, "MATH210");
}

/// The markers are consumed, not left lying around as ordinary tags — they
/// describe placement, and a "#floating" chip on the tile would be noise.
#[test]
fn zone_markers_do_not_become_tags() {
    let p = parse("gym every mon wed #floating #body", today());
    assert_eq!(p.tags, vec!["body"], "only the real tag survives");
    assert_eq!(p.category.as_deref(), Some("body"));
    assert!(!p.pinned);
}

/// Real input that failed: full weekday names, a one-letter meridiem, and a
/// room number with a space in it.
#[test]
fn a_real_class_line_parses() {
    let p = parse("PHYS201 every monday wednesday 10:30-12:00p @Hall 2.106", today());

    assert_eq!(p.title, "PHYS201");
    assert!(p.repeats);
    assert_eq!(p.byday, vec!["mon", "wed"]);
    assert_eq!(
        p.span,
        Some((
            chrono::NaiveTime::from_hms_opt(10, 30, 0).unwrap(),
            chrono::NaiveTime::from_hms_opt(12, 0, 0).unwrap()
        ))
    );
    assert_eq!(p.location.as_deref(), Some("Hall 2.106"));
}

#[test]
fn weekdays_are_accepted_written_out_in_full() {
    for (text, code) in [
        ("x every monday", "mon"), ("x every tuesday", "tue"),
        ("x every wednesday", "wed"), ("x every thursday", "thu"),
        ("x every friday", "fri"), ("x every saturday", "sat"),
        ("x every sunday", "sun"),
    ] {
        let p = parse(text, today());
        assert_eq!(p.byday, vec![code], "{text:?}");
        assert_eq!(p.title, "x", "{text:?}");
    }
    // Mixed forms in one line collapse to the same day, not two.
    assert_eq!(parse("x every mon monday", today()).byday, vec!["mon"]);
}

#[test]
fn a_single_letter_meridiem_is_a_time() {
    let t = |h, m| chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap();
    assert_eq!(parse("x 12:00p", today()).at, Some(t(12, 0)));
    assert_eq!(parse("x 5p", today()).at, Some(t(17, 0)));
    assert_eq!(parse("x 9a", today()).at, Some(t(9, 0)));
    assert_eq!(parse("x 9:30a-11a", today()).span, Some((t(9, 30), t(11, 0))));
}

/// `@` runs to the end of the line, because room numbers and building names
/// contain spaces. It is the last thing on a line in practice.
#[test]
fn a_location_may_contain_spaces() {
    let p = parse("lecture every mon 9:00-10:00 @Hall 2.106", today());
    assert_eq!(p.title, "lecture");
    assert_eq!(p.location.as_deref(), Some("Hall 2.106"));
    assert_eq!(p.byday, vec!["mon"]);

    let bare = parse("coffee @the corner cafe", today());
    assert_eq!(bare.title, "coffee");
    assert_eq!(bare.location.as_deref(), Some("the corner cafe"));
}

/// An email address is not a location: `@` only starts one at a word boundary.
#[test]
fn an_email_address_is_left_alone() {
    let p = parse("email bob@example.com about the lease", today());
    assert_eq!(p.title, "email bob@example.com about the lease");
    assert_eq!(p.location, None);
}

/// A tag after a location must survive: `@` covers the place, not the rest of
/// the line regardless of what follows.
#[test]
fn a_tag_after_a_location_is_still_a_tag() {
    let p = parse("PHYS201 every mon 10:30-12:00 @Hall 2.106 #work", today());
    assert_eq!(p.title, "PHYS201");
    assert_eq!(p.location.as_deref(), Some("Hall 2.106"));
    assert_eq!(p.tags, vec!["work"]);
    assert_eq!(p.category.as_deref(), Some("work"));
    assert_eq!(p.byday, vec!["mon"]);
}

/// Real input that failed: a meridiem written as its own word, and a
/// three-part date.
#[test]
fn an_airport_pickup_parses() {
    let p = parse("Pick up a friend from the airport 9-02-2026 8:00 pm", today());

    assert_eq!(p.title, "Pick up a friend from the airport");
    assert_eq!(p.due, Some(ymd(2026, 9, 2)));
    assert_eq!(p.at, Some(chrono::NaiveTime::from_hms_opt(20, 0, 0).unwrap()));
}

#[test]
fn a_meridiem_may_be_its_own_word() {
    let t = |h, m| chrono::NaiveTime::from_hms_opt(h, m, 0).unwrap();
    assert_eq!(parse("x 8:00 pm", today()).at, Some(t(20, 0)));
    assert_eq!(parse("x 8 pm", today()).at, Some(t(20, 0)));
    assert_eq!(parse("x 9:30 am", today()).at, Some(t(9, 30)));
    // Both words are eaten, not just the marker.
    assert_eq!(parse("x 8:00 pm", today()).title, "x");
}

/// Three-part dates. Where one number cannot be a month it settles itself;
/// where both could be, the American order wins, because that is the machine's
/// locale and how these were typed.
#[test]
fn three_part_dates_resolve_sensibly() {
    assert_eq!(parse("x 9-02-2026", today()).due, Some(ymd(2026, 9, 2)), "month first");
    assert_eq!(parse("x 9/02/2026", today()).due, Some(ymd(2026, 9, 2)), "slashes too");
    assert_eq!(parse("x 25-12-2026", today()).due, Some(ymd(2026, 12, 25)), "25 can only be a day");
    assert_eq!(parse("x 12/25/2026", today()).due, Some(ymd(2026, 12, 25)), "25 can only be a day");
    assert_eq!(parse("x 2026-09-02", today()).due, Some(ymd(2026, 9, 2)), "ISO still wins");
    // Not a date at all.
    assert_eq!(parse("x 45-99-2026", today()).title, "x 45-99-2026");
}

/// A version number is not a date: three parts only count when the last is a
/// four-digit year.
#[test]
fn a_version_number_is_not_a_date() {
    assert_eq!(parse("upgrade to 1-2-3", today()).title, "upgrade to 1-2-3");
    assert_eq!(parse("bump rustc to 1.95.0", today()).title, "bump rustc to 1.95.0");
}
