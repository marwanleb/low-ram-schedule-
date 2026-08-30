use crate::db::Db;

#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    pub tag: Option<String>,
    /// Items with both an estimate and finished, non-discarded time.
    pub n: usize,
    /// Percent over the estimate; negative means faster. None when nothing
    /// has been measured.
    pub pct_over: Option<i32>,
    /// Ready to print. None below MIN_CONFIDENT samples.
    pub phrase: Option<String>,
    pub confident: bool,
}

/// Below this, the ratio is noise wearing the costume of a finding.
pub const MIN_CONFIDENT: usize = 5;

/// How much longer things actually take than estimated, as one number.
///
/// The median of each item's own ratio, deliberately not the ratio of the
/// summed totals: one eight-hour overrun would otherwise swamp twenty
/// half-hour tasks and the figure would stop describing a typical day.
/// Spec 5.6.
pub fn stats(db: &Db, tag: Option<&str>) -> Stats {
    let mut ratios = ratios(db, tag);
    let n = ratios.len();

    if n == 0 {
        return Stats { tag: tag.map(str::to_string), n: 0, pct_over: None, phrase: None, confident: false };
    }

    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = if n % 2 == 1 {
        ratios[n / 2]
    } else {
        (ratios[n / 2 - 1] + ratios[n / 2]) / 2.0
    };

    let pct = ((median - 1.0) * 100.0).round() as i32;
    let confident = n >= MIN_CONFIDENT;
    let phrase = confident.then(|| {
        if pct >= 0 {
            format!("{pct}% longer")
        } else {
            format!("{}% shorter", -pct)
        }
    });

    Stats { tag: tag.map(str::to_string), n, pct_over: Some(pct), phrase, confident }
}

/// One actual/estimate ratio per qualifying item.
///
/// Tag matching happens in Rust rather than SQL: tags are a comma-separated
/// column, and a LIKE would match "math" inside "aftermath".
fn ratios(db: &Db, tag: Option<&str>) -> Vec<f64> {
    const SQL: &str = "
        SELECT i.tags, i.estimate_min,
               SUM((julianday(s.ended_at) - julianday(s.started_at)) * 24 * 60)
        FROM items i
        JOIN sessions s ON s.item_id = i.id
        WHERE i.estimate_min IS NOT NULL
          AND i.estimate_min > 0
          AND s.ended_at IS NOT NULL
          AND s.discarded = 0
        GROUP BY i.id
        HAVING SUM((julianday(s.ended_at) - julianday(s.started_at)) * 24 * 60) > 0
    ";

    let Ok(mut stmt) = db.conn.prepare(SQL) else { return Vec::new() };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, f64>(1)?, r.get::<_, f64>(2)?))
    }) else {
        return Vec::new();
    };

    rows.filter_map(|r| r.ok())
        .filter(|(tags, _, _)| match tag {
            None => true,
            Some(want) => tags.split(',').any(|t| t.trim().eq_ignore_ascii_case(want)),
        })
        .map(|(_, estimate, actual)| actual / estimate)
        .collect()
}
