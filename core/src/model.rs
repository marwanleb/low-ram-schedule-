use chrono::{DateTime, FixedOffset, NaiveDate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Recurrence,
    OneOff,
    Moved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Warn,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub id: String,
    pub item_id: String,
    pub starts_at: DateTime<FixedOffset>,
    pub ends_at: DateTime<FixedOffset>,
    pub origin: Origin,
    pub moved_from: Option<NaiveDate>,
    pub pinned_tz: Option<String>,
    pub foreign: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    pub level: Level,
    pub item_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Day {
    pub date: NaiveDate,
    pub placements: Vec<Placement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Week {
    pub anchor: NaiveDate,
    pub viewing_tz: String,
    pub days: Vec<Day>,
    pub diagnostics: Vec<Diagnostic>,
}
