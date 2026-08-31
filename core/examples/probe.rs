//! Scratch harness for looking at what the parser does with a given line.
//!
//!     cargo run -p ms-core --example probe

use chrono::NaiveDate;

fn main() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
    for line in [
        "flight to paris on sept 3 at 6:45am",
        "spring break starts march 14",
        "jury duty aug 31",
        "trip to france jan 5-12",
        "meeting 5/9 3pm",
        "quiz sun 10am",
        "quiz tues 10am",
        "psych 301 discussion section thurs 12:30pm",
        "meet bob @ starbucks",
        "meet bob @starbucks 3pm",
        "weekly team meeting mondays at 3",
        "gym mon/wed/fri 6am",
        "CS 314 lecture mon wed fri 10am",
        "lunch at noon monday",
        "9/2 8pm study session",
    ] {
        let p = ms_core::parse(line, today);
        println!("{line:?}");
        println!("   title    {:?}", p.title);
        println!("   consumed {:?}", p.consumed);
        println!("   byday {:?}  span {:?}  loc {:?}  repeats {}", p.byday, p.span, p.location, p.repeats);
        println!();
    }
}
