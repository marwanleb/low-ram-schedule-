//! Turning fields back into a line of the capture grammar.
//!
//! The inverse of [`crate::parse`]. The app's field form and the Telegram
//! ladder both need to build a sentence from what a person filled in; built
//! separately in JavaScript and Rust they would be two implementations of one
//! rule set, and they would drift. There is one, here, and both call it.
//!
//! Nothing composed here reaches the store directly — the line goes through
//! `add_from_text` like anything typed by hand. That is the point: one grammar,
//! one path in.
//!
//! `core/tests/compose.rs` asserts `parse(line(f)) == f` over fixed and
//! generated cases, which is what keeps the two directions honest.

use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// A deadline: says "due", lands in the list.
    #[default]
    Task,
    /// Says "on": takes an hour on the week, and stays tickable.
    Block,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Fields {
    pub title: String,
    pub priority: bool,
    /// Weekday codes as the parser spells them: `mon`, `tue`, …
    pub repeat: Vec<String>,
    pub kind: Kind,
    pub date: Option<NaiveDate>,
    pub at: Option<NaiveTime>,
    /// Present only with `at`; together they make a range.
    pub span_end: Option<NaiveTime>,
    pub estimate_min: Option<u32>,
    pub tag: Option<String>,
    pub place: Option<String>,
}

/// One spelling per value, so the round trip is a function rather than a
/// relation: whole hours read as hours, everything else as minutes.
fn estimate(minutes: u32) -> String {
    if minutes >= 60 && minutes % 60 == 0 {
        format!("~{}h", minutes / 60)
    } else {
        format!("~{minutes}m")
    }
}

/// Build a line the parser will read back as these fields.
pub fn line(f: &Fields) -> String {
    let mut out = String::new();
    if f.priority {
        out.push_str("!! ");
    }
    out.push_str(f.title.trim());

    let mut push = |s: &str| {
        if !s.is_empty() {
            out.push(' ');
            out.push_str(s);
        }
    };

    if !f.repeat.is_empty() {
        push(&format!("every {}", f.repeat.join(" ")));
    } else if f.date.is_some() {
        // Without a date there is nothing for "due"/"on" to attach to, and a
        // trailing filler word would just be swallowed into the title.
        push(match f.kind {
            Kind::Task => "due",
            Kind::Block => "on",
        });
    }

    // A repeat has no single date, and the parser drops one if given. Emitting
    // it anyway would put a date in the preview that silently does nothing.
    if let (Some(date), true) = (f.date, f.repeat.is_empty()) {
        // Day first, four-digit year: the one spelling the parser reads
        // unambiguously regardless of which number could be a month.
        push(&date.format("%d/%m/%Y").to_string());
    }

    if let Some(at) = f.at {
        match f.span_end {
            Some(end) => push(&format!("{}-{}", at.format("%H:%M"), end.format("%H:%M"))),
            None => push(&at.format("%H:%M").to_string()),
        }
    }

    if let Some(m) = f.estimate_min {
        push(&estimate(m));
    }
    if let Some(tag) = f.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        push(&format!("#{}", tag.trim_start_matches('#')));
    }
    // Last: `@` runs to the end of the line or until a recognised token, so the
    // final position is the one with nothing to reason about.
    if let Some(place) = f.place.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        push(&format!("@{}", place.trim_start_matches('@')));
    }

    out
}
