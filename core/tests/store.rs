use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::America::Chicago;
use chrono_tz::Asia::Beirut;
use ms_core::{add_from_text, add_from_text_in, get_items, set_done, Db, Filter};

fn today() -> NaiveDate { NaiveDate::from_ymd_opt(2026, 8, 31).unwrap() }
fn now() -> DateTime<Utc> { Utc.with_ymd_and_hms(2026, 8, 31, 12, 0, 0).unwrap() }

#[test]
fn a_typed_line_becomes_an_item() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "math hw fri ~2h #math", today(), now()).unwrap();

    assert_eq!(item.title, "math hw");
    assert_eq!(item.estimate_min, Some(120));
    assert_eq!(item.tags, vec!["math"]);
    assert!(item.listed);
    assert!(!item.recurs);
    assert_eq!(item.source, "self");
    assert_eq!(item.external_id, None);
    assert!(item.due_at.is_some());
}

#[test]
fn a_repeating_line_also_creates_its_rule() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "MATH210 every mon wed 9:00-10:15", today(), now()).unwrap();

    assert_eq!(item.title, "MATH210");
    assert!(item.recurs);
    assert!(!item.listed, "a block stays out of the to-do list");

    let (byday, start, end): (String, String, String) = db.conn.query_row(
        "SELECT byday, start_time, end_time FROM recurrence WHERE item_id = ?1",
        rusqlite::params![item.id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).unwrap();
    assert_eq!(byday, "mon,wed");
    assert_eq!((start.as_str(), end.as_str()), ("09:00", "10:15"));
}

#[test]
fn a_known_tag_sets_the_colour_category() {
    let db = Db::open_in_memory().unwrap();
    let body = add_from_text(&db, "swim every tue 18:00-19:00 #body", today(), now()).unwrap();
    assert_eq!(body.category.as_deref(), Some("body"));

    let math = add_from_text(&db, "problem set fri #math", today(), now()).unwrap();
    assert_eq!(math.category, None, "not one of the four");
    assert_eq!(math.tags, vec!["math"], "still a tag");
}

#[test]
fn items_can_be_listed_and_filtered() {
    let db = Db::open_in_memory().unwrap();
    add_from_text(&db, "renew parking", today(), now()).unwrap();
    add_from_text(&db, "MATH210 every mon 9:00-10:15", today(), now()).unwrap();

    assert_eq!(get_items(&db, &Filter::default()).len(), 2);

    let listed = get_items(&db, &Filter { listed: Some(true), ..Default::default() });
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].title, "renew parking");

    let repeating = get_items(&db, &Filter { recurs: Some(true), ..Default::default() });
    assert_eq!(repeating.len(), 1);
    assert_eq!(repeating[0].title, "MATH210");
}

/// Spec 3.2: completing a repeating item ticks off one occurrence, not the
/// item forever.
#[test]
fn completion_is_per_occurrence_for_repeating_items() {
    let db = Db::open_in_memory().unwrap();
    let trash = add_from_text(&db, "trash every tue 20:00", today(), now()).unwrap();

    let d1 = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();
    set_done(&db, &trash.id, true, Some(d1), now()).unwrap();

    let again = get_items(&db, &Filter::default());
    let it = again.iter().find(|i| i.id == trash.id).unwrap();
    assert_eq!(it.done_on, vec![d1], "only that week is done");
    assert!(!it.done_on.contains(&d2), "next week is still outstanding");
}

#[test]
fn a_one_off_completes_without_a_date() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "renew parking", today(), now()).unwrap();

    set_done(&db, &item.id, true, None, now()).unwrap();
    let done = get_items(&db, &Filter { done: Some(true), ..Default::default() });
    assert_eq!(done.len(), 1);

    set_done(&db, &item.id, false, None, now()).unwrap();
    let open = get_items(&db, &Filter { done: Some(false), ..Default::default() });
    assert_eq!(open.len(), 1, "un-ticking restores it");
}

/// Regression: ticking off this week's chore must not retire it forever.
/// The completions table is per-occurrence, but a "done" filter that asks
/// "any completion at all?" throws that away.
#[test]
fn a_repeating_item_reopens_for_the_next_occurrence() {
    let db = Db::open_in_memory().unwrap();
    let trash = add_from_text(&db, "trash every tue 20:00", today(), now()).unwrap();
    let this_week = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let next_week = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();

    set_done(&db, &trash.id, true, Some(this_week), now()).unwrap();

    let open_now = get_items(&db, &Filter { done: Some(false), on: Some(this_week), ..Default::default() });
    assert!(!open_now.iter().any(|i| i.id == trash.id), "done for this Tuesday");

    let open_next = get_items(&db, &Filter { done: Some(false), on: Some(next_week), ..Default::default() });
    assert!(open_next.iter().any(|i| i.id == trash.id), "but open again next Tuesday");
}

#[test]
fn a_finished_one_off_stays_finished() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "renew parking", today(), now()).unwrap();
    set_done(&db, &item.id, true, None, now()).unwrap();

    for day in [NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(), NaiveDate::from_ymd_opt(2026, 12, 1).unwrap()] {
        let open = get_items(&db, &Filter { done: Some(false), on: Some(day), ..Default::default() });
        assert!(!open.iter().any(|i| i.id == item.id), "a one-off does not come back on {day}");
    }
}

#[test]
fn a_new_repeat_is_pinned_to_the_zone_it_was_created_in() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "MATH210 every mon 9:00-10:15", today(), now(), Chicago).unwrap();

    let tz: Option<String> = db.conn.query_row(
        "SELECT tz FROM recurrence WHERE item_id = ?1",
        rusqlite::params![item.id], |r| r.get(0)).unwrap();
    assert_eq!(tz.as_deref(), Some("America/Chicago"), "fixed to where you made it");
}

#[test]
fn a_floating_repeat_stores_no_zone() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "wake up daily 08:00 #floating", today(), now(), Chicago).unwrap();

    let tz: Option<String> = db.conn.query_row(
        "SELECT tz FROM recurrence WHERE item_id = ?1",
        rusqlite::params![item.id], |r| r.get(0)).unwrap();
    assert_eq!(tz, None, "follows the machine");
}

/// The whole point: the same item renders differently depending on which of
/// the two it is, once you have travelled.
#[test]
fn fixed_and_floating_diverge_after_you_travel() {
    let db = Db::open_in_memory().unwrap();
    add_from_text_in(&db, "MATH210 every mon 12:00-13:00", today(), now(), Chicago).unwrap();
    add_from_text_in(&db, "wake up every mon 08:00-08:30 #floating", today(), now(), Chicago).unwrap();

    let seen_from_beirut = ms_core::get_week(&db, today(), Beirut);
    let mut by_title: Vec<(String, String)> = seen_from_beirut.days[0]
        .placements
        .iter()
        .map(|p| {
            let t = ms_core::store::fetch(&db, &p.item_id).unwrap().title;
            (t, p.starts_at.format("%H:%M").to_string())
        })
        .collect();
    by_title.sort();

    assert_eq!(
        by_title,
        vec![
            ("MATH210".to_string(), "20:00".to_string()),   // noon in Austin
            ("wake up".to_string(), "08:00".to_string()), // 08:00 wherever you are
        ]
    );
}

#[test]
fn the_zone_can_be_changed_afterwards() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "gym every mon 18:00-19:00", today(), now(), Chicago).unwrap();

    ms_core::set_recurrence_tz(&db, &item.id, None).unwrap();
    let tz: Option<String> = db.conn.query_row(
        "SELECT tz FROM recurrence WHERE item_id = ?1",
        rusqlite::params![item.id], |r| r.get(0)).unwrap();
    assert_eq!(tz, None, "switched to floating");

    ms_core::set_recurrence_tz(&db, &item.id, Some("Asia/Beirut")).unwrap();
    let tz: Option<String> = db.conn.query_row(
        "SELECT tz FROM recurrence WHERE item_id = ?1",
        rusqlite::params![item.id], |r| r.get(0)).unwrap();
    assert_eq!(tz.as_deref(), Some("Asia/Beirut"));
}

#[test]
fn an_unknown_zone_is_refused_rather_than_stored() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "gym every mon 18:00-19:00", today(), now(), Chicago).unwrap();
    assert!(ms_core::set_recurrence_tz(&db, &item.id, Some("Mars/Olympus")).is_err());
}

#[test]
fn estimate_and_listed_can_be_changed() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "problem set fri", today(), now()).unwrap();
    assert_eq!(item.estimate_min, None);
    assert!(item.listed);

    ms_core::set_estimate(&db, &item.id, Some(120)).unwrap();
    ms_core::set_listed(&db, &item.id, false).unwrap();

    let after = ms_core::store::fetch(&db, &item.id).unwrap();
    assert_eq!(after.estimate_min, Some(120));
    assert!(!after.listed);

    // Clearing it means "unestimated", not zero.
    ms_core::set_estimate(&db, &item.id, None).unwrap();
    assert_eq!(ms_core::store::fetch(&db, &item.id).unwrap().estimate_min, None);
}

/// Regression: a one-off's completion is keyed on an empty date, which is not
/// a parseable NaiveDate and therefore never appears in `done_on`. Deriving
/// "is it done" from that list reports false for every finished one-off — the
/// checkbox never ticks and `sched done` looks like it did nothing.
#[test]
fn a_completed_one_off_reports_itself_as_done() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "renew parking", today(), now()).unwrap();
    assert!(!item.completed);

    set_done(&db, &item.id, true, None, now()).unwrap();

    let after = ms_core::store::fetch(&db, &item.id).unwrap();
    assert!(after.completed, "a ticked one-off must say so");
    assert!(after.is_done(None));

    // And the same through the list path the UI actually uses.
    let all = get_items(&db, &Filter::default());
    assert!(all.iter().find(|i| i.id == item.id).unwrap().completed);

    set_done(&db, &item.id, false, None, now()).unwrap();
    assert!(!ms_core::store::fetch(&db, &item.id).unwrap().completed);
}

#[test]
fn a_repeating_item_reports_done_per_occurrence() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text(&db, "trash every tue 20:00", today(), now()).unwrap();
    let d1 = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
    let d2 = NaiveDate::from_ymd_opt(2026, 9, 8).unwrap();

    set_done(&db, &item.id, true, Some(d1), now()).unwrap();
    let after = ms_core::store::fetch(&db, &item.id).unwrap();

    assert!(after.is_done(Some(d1)));
    assert!(!after.is_done(Some(d2)), "next week is still open");
}

/// Regression: `@Hall 2.106` was parsed, stripped from the title, and then
/// dropped — losing information the user had deliberately typed.
#[test]
fn a_location_is_kept_not_just_stripped() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(
        &db, "PHYS201 every monday wednesday 10:30-12:00p @Hall 2.106", today(), now(), Chicago,
    ).unwrap();

    assert_eq!(item.title, "PHYS201");
    assert_eq!(item.location.as_deref(), Some("Hall 2.106"));
    assert_eq!(ms_core::store::fetch(&db, &item.id).unwrap().location.as_deref(), Some("Hall 2.106"));
}

// ── found by a black-box tester that had not seen the code ───────────────

/// A due time is a wall clock in the zone you are standing in. Storing "5pm"
/// as 17:00 UTC makes it noon in Austin — every deadline five hours out.
#[test]
fn a_due_time_is_local_not_utc() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "math hw fri 5pm", today(), now(), Chicago).unwrap();

    let due = item.due_at.expect("has a due date");
    let in_austin = due.with_timezone(&Chicago);
    assert_eq!(in_austin.format("%H:%M").to_string(), "17:00", "5pm means 5pm where you are");
    // Which is 22:00 UTC in September (CDT is UTC-5).
    assert_eq!(due.format("%H:%M").to_string(), "22:00");
}

#[test]
fn a_deadline_with_no_time_lands_at_the_end_of_the_local_day() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "essay fri", today(), now(), Chicago).unwrap();
    let in_austin = item.due_at.unwrap().with_timezone(&Chicago);
    assert_eq!(in_austin.format("%H:%M").to_string(), "23:59");
}

/// Naming today's weekday after that hour has passed used to file the item as
/// already overdue. A weekday means the next one that has not gone by.
#[test]
fn a_weekday_whose_time_has_passed_means_next_week() {
    let db = Db::open_in_memory().unwrap();
    // today() is Monday 2026-08-31; it is already 18:00 in Austin.
    let evening = Chicago.with_ymd_and_hms(2026, 8, 31, 18, 0, 0).unwrap().to_utc();

    let past = add_from_text_in(&db, "gym mon 10am", today(), evening, Chicago).unwrap();
    assert_eq!(
        past.due_at.unwrap().with_timezone(&Chicago).date_naive(),
        NaiveDate::from_ymd_opt(2026, 9, 7).unwrap(),
        "10am Monday has gone; it means next Monday"
    );

    // Still to come today, so it stays today.
    let later = add_from_text_in(&db, "gym mon 11pm", today(), evening, Chicago).unwrap();
    assert_eq!(
        later.due_at.unwrap().with_timezone(&Chicago).date_naive(),
        NaiveDate::from_ymd_opt(2026, 8, 31).unwrap(),
    );
}

/// An explicit date is taken at its word, even if it is in the past — the user
/// may be recording something they already missed.
#[test]
fn an_explicit_past_date_is_not_moved() {
    let db = Db::open_in_memory().unwrap();
    let item = add_from_text_in(&db, "rent 1/8/2026", today(), now(), Chicago).unwrap();
    assert_eq!(
        item.due_at.unwrap().with_timezone(&Chicago).date_naive(),
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap(),
    );
}
