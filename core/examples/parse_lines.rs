//! Reads one line per input on stdin, prints what the parser made of it as
//! JSON. Used by tools/parser_sweep.py to compare against a reference parser.
//!
//!     cargo run -p ms-core --example parse_lines < corpus.txt

use chrono::NaiveDate;
use std::io::BufRead;

fn main() {
    // Fixed, so relative words resolve the same way on every run.
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();

    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        let p = ms_core::parse(&line, today);
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        println!(
            r#"{{"input":"{}","title":"{}","consumed":{},"due":{},"at":{},"span":{},"byday":{},"repeats":{},"listed":{},"scheduled":{},"location":{},"estimate_min":{}}}"#,
            esc(&line),
            esc(&p.title),
            p.consumed.len(),
            p.due.map(|d| format!("\"{d}\"")).unwrap_or("null".into()),
            p.at.map(|t| format!("\"{}\"", t.format("%H:%M"))).unwrap_or("null".into()),
            p.span
                .map(|(a, b)| format!("\"{}-{}\"", a.format("%H:%M"), b.format("%H:%M")))
                .unwrap_or("null".into()),
            p.byday.len(),
            p.repeats,
            p.listed,
            p.scheduled,
            p.location.map(|l| format!("\"{}\"", esc(&l))).unwrap_or("null".into()),
            p.estimate_min.map(|v| v.to_string()).unwrap_or("null".into()),
        );
    }
}
