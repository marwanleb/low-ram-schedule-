//! Clock times as people read them: 12-hour and compact.
//!
//! Display only. Storage, JSON output and what the parser accepts are
//! unchanged; this is what a person is shown -- in Telegram replies and
//! reminders, the CLI's text output, diagnostics, and the line the field form
//! composes. The frontend has the same rules in app/src/clock.js.

use chrono::{NaiveTime, Timelike};

/// 15:30 -> "3:30pm", 15:00 -> "3pm", noon -> "12pm", midnight -> "12am".
pub fn time12(t: NaiveTime) -> String {
    let (h, m) = (t.hour(), t.minute());
    let meridiem = if h < 12 { "am" } else { "pm" };
    let h12 = if h % 12 == 0 { 12 } else { h % 12 };
    if m == 0 {
        format!("{h12}{meridiem}")
    } else {
        format!("{h12}:{m:02}{meridiem}")
    }
}

/// "2–5pm", "10:30am–12pm". The meridiem is written once when both ends share
/// it, which keeps a range short enough for a narrow block.
pub fn range12(start: NaiveTime, end: NaiveTime) -> String {
    let (a, b) = (time12(start), time12(end));
    if (start.hour() < 12) == (end.hour() < 12) {
        format!("{}\u{2013}{b}", &a[..a.len() - 2])
    } else {
        format!("{a}\u{2013}{b}")
    }
}
