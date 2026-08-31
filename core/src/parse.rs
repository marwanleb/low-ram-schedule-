use chrono::{Datelike, Duration, NaiveDate, NaiveTime};

#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub title: String,
    pub consumed: Vec<String>,
    pub estimate_min: Option<u32>,
    pub at: Option<NaiveTime>,
    pub byday: Vec<String>,
    pub due: Option<NaiveDate>,
    /// Stated by the `every` keyword, never inferred from how many weekdays
    /// were typed. `math hw fri` is due Friday; `trash every tue` repeats.
    pub repeats: bool,
    pub span: Option<(NaiveTime, NaiveTime)>,
    /// False when a time range was given: typing a span means scheduling, not
    /// listing. Spec 3.1.
    pub listed: bool,
    pub tags: Vec<String>,
    /// The one tag that is also a colour category, if any. Spec 3.3.
    pub category: Option<String>,
    pub location: Option<String>,
    pub priority: bool,
    /// True = fixed to the zone it was created in; false = follows the machine.
    /// Defaults to true: a class that drifts to the wrong hour costs more than
    /// a gym slot that does.
    pub pinned: bool,
}

/// Closed set, because each needs a colour the UI can rely on. Spec 3.3.
pub const CATEGORIES: [&str; 4] = ["work", "life", "body", "social"];

/// What a single trailing token turned out to mean.
enum Token {
    Weekday(usize),
    Time(u32, u32),
    Estimate(u32),
    /// Days from today: 0 = today, 1 = tomorrow.
    Relative(i64),
    Date(NaiveDate),
    /// Day and month with the year left open, resolved forward from today.
    DayMonth(u32, u32),
    Every,
    Daily,
    Span(NaiveTime, NaiveTime),
    Tag(String),
    Zone { pinned: bool },
}

const DAY_CODES: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

const DAY_NAMES: [&str; 7] = [
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];

fn weekday(t: &str) -> Option<Token> {
    DAY_CODES
        .iter()
        .position(|d| *d == t)
        .or_else(|| DAY_NAMES.iter().position(|d| *d == t))
        .map(Token::Weekday)
}

/// `5pm`, `5:30pm`, `17:00`.
fn time(t: &str) -> Option<Token> {
    // "12:00p" and "5p" are as common as the two-letter forms when people type
    // in a hurry, so both are accepted.
    let (body, shift) = if let Some(b) = t.strip_suffix("pm").or_else(|| t.strip_suffix('p')) {
        (b, 12)
    } else if let Some(b) = t.strip_suffix("am").or_else(|| t.strip_suffix('a')) {
        (b, 0)
    } else {
        (t, u32::MAX) // marker: 24h form, no meridiem
    };

    let (h, m) = match body.split_once(':') {
        Some((h, m)) => (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?),
        None => (body.parse::<u32>().ok()?, 0),
    };
    if m > 59 {
        return None;
    }

    let hour = if shift == u32::MAX {
        // Bare digits are only a time when written as h:mm; "5" alone is not.
        if !body.contains(':') || h > 23 {
            return None;
        }
        h
    } else {
        if h == 0 || h > 12 {
            return None;
        }
        (h % 12) + shift
    };
    Some(Token::Time(hour, m))
}

/// `~2h`, `~90m`, `~1.5h`.
fn estimate(t: &str) -> Option<Token> {
    let body = t.strip_prefix('~')?;
    if let Some(h) = body.strip_suffix('h') {
        let hours: f64 = h.parse().ok()?;
        if hours <= 0.0 {
            return None;
        }
        return Some(Token::Estimate((hours * 60.0).round() as u32));
    }
    if let Some(m) = body.strip_suffix('m') {
        let mins: u32 = m.parse().ok()?;
        if mins == 0 {
            return None;
        }
        return Some(Token::Estimate(mins));
    }
    None
}

fn relative(t: &str) -> Option<Token> {
    match t {
        "today" => Some(Token::Relative(0)),
        "tomorrow" => Some(Token::Relative(1)),
        _ => None,
    }
}

/// `9:00-10:15`, `9am-10:15am`.
fn span(t: &str) -> Option<Token> {
    let (a, b) = t.split_once('-')?;
    match (time(a)?, time(b)?) {
        (Token::Time(h1, m1), Token::Time(h2, m2)) => Some(Token::Span(
            NaiveTime::from_hms_opt(h1, m1, 0)?,
            NaiveTime::from_hms_opt(h2, m2, 0)?,
        )),
        _ => None,
    }
}

fn tag(raw: &str) -> Option<Token> {
    let body = raw.strip_prefix('#')?;
    if body.is_empty() {
        return None;
    }
    let lower = body.to_ascii_lowercase();
    // Placement markers are consumed rather than kept: they say where the item
    // sits, not what it is about, and a "#floating" chip would be noise.
    match lower.as_str() {
        "fixed" | "pinned" => return Some(Token::Zone { pinned: true }),
        "floating" | "float" | "systime" => return Some(Token::Zone { pinned: false }),
        _ => {}
    }
    Some(Token::Tag(lower))
}

/// Pull a trailing `@place` off the line before anything else looks at it.
///
/// Locations contain spaces — "Hall 2.106", "the corner cafe" — so `@` covers more than
/// one word. It has to happen before the right-to-left scan, which would
/// otherwise halt on "1.314" and never reach the marker.
///
/// It stops at the next `#tag`, so "@Hall 2.106 #work" keeps the tag rather than
/// swallowing it. An `@` mid-word ("bob@example.com") is not a marker.
fn split_location(input: &str) -> (String, Option<String>) {
    let words: Vec<&str> = input.split_whitespace().collect();
    let Some(at) = words.iter().position(|w| w.starts_with('@') && w.len() > 1) else {
        return (input.to_string(), None);
    };

    let stop = words[at + 1..]
        .iter()
        .position(|w| w.starts_with('#'))
        .map(|k| at + 1 + k)
        .unwrap_or(words.len());

    let mut place = words[at][1..].to_string();
    for w in &words[at + 1..stop] {
        place.push(' ');
        place.push_str(w);
    }

    let mut head: Vec<&str> = words[..at].to_vec();
    head.extend_from_slice(&words[stop..]);
    (head.join(" "), Some(place))
}

fn every(t: &str) -> Option<Token> {
    (t == "every" || t == "weekly").then_some(Token::Every)
}

/// `daily` is a keyword; the bare word `day` deliberately is not, because it
/// ends ordinary sentences ("buy milk for the day") and would eat them.
fn daily(t: &str) -> Option<Token> {
    (t == "daily" || t == "everyday").then_some(Token::Daily)
}

/// Split a date on either separator, e.g. `9-02-2026` or `9/02/2026`.
fn date_parts(t: &str) -> Option<Vec<&str>> {
    let sep = if t.contains('-') { '-' } else { '/' };
    let parts: Vec<&str> = t.split(sep).collect();
    (parts.len() == 2 || parts.len() == 3).then_some(parts)
}

/// A written date: `2026-09-15`, `9-02-2026`, `9/02/2026`, or `9/2` with the
/// year left open.
///
/// Three-part forms need a four-digit year, so a version number like `1-2-3`
/// is not mistaken for one — chrono will happily read that as the year 1.
///
/// Where one number is above 12 it can only be the day, which settles the
/// order by itself. Where both could be a month the American order wins: it is
/// this machine's locale and how these get typed here. The app echoes the date
/// it resolved, so a wrong guess is visible rather than silent.
fn date(t: &str) -> Option<Token> {
    let parts = date_parts(t)?;
    let nums: Vec<u32> = parts.iter().filter_map(|p| p.parse::<u32>().ok()).collect();
    if nums.len() != parts.len() {
        return None;
    }

    match parts.len() {
        3 => {
            let (a, b, c) = (nums[0], nums[1], nums[2]);
            // ISO: the year comes first.
            if parts[0].len() == 4 {
                return NaiveDate::from_ymd_opt(a as i32, b, c).map(Token::Date);
            }
            if parts[2].len() != 4 {
                return None;
            }
            let (month, day) = if a > 12 { (b, a) } else { (a, b) };
            NaiveDate::from_ymd_opt(c as i32, month, day).map(Token::Date)
        }
        _ => {
            let (a, b) = (nums[0], nums[1]);
            let (month, day) = if a > 12 { (b, a) } else { (a, b) };
            if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
                return None;
            }
            Some(Token::DayMonth(day, month))
        }
    }
}

fn recognise(raw: &str) -> Option<Token> {
    // Tags keep the original casing of their body, so they are matched against
    // the raw word rather than the lowered one.
    if let Some(tok) = tag(raw) {
        return Some(tok);
    }
    let t = raw.to_ascii_lowercase();
    weekday(&t)
        .or_else(|| every(&t))
        .or_else(|| daily(&t))
        .or_else(|| relative(&t))
        .or_else(|| span(&t))
        .or_else(|| time(&t))
        .or_else(|| date(&t))
        .or_else(|| estimate(&t))
}

/// First occurrence of `weekday` on or after `today`, so naming today's own
/// weekday means today rather than a week away.
fn next_weekday(today: NaiveDate, target: usize) -> NaiveDate {
    let now = today.weekday().num_days_from_monday() as i64;
    let ahead = (target as i64 - now).rem_euclid(7);
    today + Duration::days(ahead)
}

/// First date with this day and month on or after `today`.
fn next_day_month(today: NaiveDate, day: u32, month: u32) -> Option<NaiveDate> {
    for year in [today.year(), today.year() + 1] {
        if let Some(d) = NaiveDate::from_ymd_opt(year, month, day) {
            if d >= today {
                return Some(d);
            }
        }
    }
    None
}

/// Never fails: unparseable input becomes a title. See spec 6.
///
/// Scans from the right and stops at the first token it does not recognise, so
/// prose containing date-shaped words ("the March report") is left intact.
pub fn parse(input: &str, today: NaiveDate) -> Parsed {
    let (head, location_at) = split_location(input);
    let words: Vec<&str> = head.split_whitespace().collect();
    let mut consumed: Vec<String> = Vec::new();
    let mut estimate_min = None;
    let mut at = None;
    let mut byday: Vec<String> = Vec::new();
    let mut due: Option<NaiveDate> = None;
    let mut repeats = false;
    let mut span_at: Option<(NaiveTime, NaiveTime)> = None;
    let mut tags: Vec<String> = Vec::new();
    let mut pinned = true;
    let mut end = words.len();

    while end > 0 {
        let raw = words[end - 1];

        // "8:00 pm" — the meridiem written separately. Read as one token with
        // the word before it, and consume both. Only the two-letter forms, so
        // a trailing English "a" is never mistaken for one.
        let lower = raw.to_ascii_lowercase();
        if (lower == "am" || lower == "pm") && end >= 2 {
            let joined = format!("{}{}", words[end - 2], lower);
            if let Some(Token::Time(h, m)) = time(&joined) {
                at = NaiveTime::from_hms_opt(h, m, 0);
                consumed.push(raw.to_string());
                consumed.push(words[end - 2].to_string());
                end -= 2;
                continue;
            }
        }

        let Some(token) = recognise(raw) else { break };
        match token {
            // Scanning right-to-left, so a repeated token of the same kind
            // leaves the leftmost value in place.
            Token::Estimate(m) => estimate_min = Some(m),
            Token::Time(h, m) => at = NaiveTime::from_hms_opt(h, m, 0),
            Token::Weekday(i) => {
                if !byday.contains(&DAY_CODES[i].to_string()) {
                    byday.push(DAY_CODES[i].to_string());
                }
            }
            Token::Relative(days) => due = today.checked_add_signed(Duration::days(days)),
            Token::Date(d) => due = Some(d),
            Token::DayMonth(d, m) => due = next_day_month(today, d, m),
            Token::Every => repeats = true,
            Token::Daily => {
                repeats = true;
                for code in DAY_CODES {
                    if !byday.contains(&code.to_string()) {
                        byday.push(code.to_string());
                    }
                }
            }
            Token::Span(a, b) => span_at = Some((a, b)),
            Token::Tag(t) => {
                if !tags.contains(&t) {
                    tags.push(t);
                }
            }
            Token::Zone { pinned: p } => pinned = p,
        }
        consumed.push(raw.to_string());
        end -= 1;
    }
    consumed.reverse();
    byday.reverse();
    byday.sort_by_key(|c| DAY_CODES.iter().position(|d| d == c).unwrap_or(usize::MAX));
    tags.reverse();

    // `!!` is written in front of the title rather than trailing, so it is
    // stripped from the left.
    let mut title = words[..end].join(" ");
    let stripped = title.trim_start_matches('!').trim_start().to_string();
    // Only treat `!!` as a marker when something survives it: "!!!" on its own
    // is a capture, and capture is never discarded. Spec 6.
    let priority = title.starts_with("!!") && !stripped.is_empty();
    if priority {
        title = stripped;
    }

    let category = tags
        .iter()
        .find(|t| CATEGORIES.contains(&t.as_str()))
        .cloned();

    // A repeat has no single due date. Otherwise a lone weekday names the next
    // such day; several weekdays without `every` can only mean a repeat, so
    // they are treated as one.
    if repeats || byday.len() > 1 {
        repeats = true;
        due = None;
    } else if due.is_none() && byday.len() == 1 {
        if let Some(i) = DAY_CODES.iter().position(|d| *d == byday[0]) {
            due = Some(next_weekday(today, i));
        }
    }

    Parsed {
        title,
        consumed,
        estimate_min,
        at,
        byday,
        due,
        repeats,
        span: span_at,
        listed: span_at.is_none(),
        tags,
        category,
        location: location_at,
        priority,
        pinned,
    }
}
