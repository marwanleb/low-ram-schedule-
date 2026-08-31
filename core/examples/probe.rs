//! Scratch harness for looking at what the parser does with a given line.
//!
//!     cargo run -p ms-core --example probe

use chrono::NaiveDate;

fn main() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
    for line in [
        "Pick up a friend from the airport 9-02-2026 8:00 pm",
        "x 8:00 pm",
        "x 9-02-2026",
        "x 9/02/2026",
        "x 25-12-2026",
    ] {
        let p = ms_core::parse(line, today);
        println!("{line:?}");
        println!("   title    {:?}", p.title);
        println!("   consumed {:?}", p.consumed);
        println!("   byday {:?}  span {:?}  loc {:?}  repeats {}", p.byday, p.span, p.location, p.repeats);
        println!();
    }
}
