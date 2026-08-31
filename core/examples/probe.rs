//! Scratch harness for looking at what the parser does with a given line.
//!
//!     cargo run -p ms-core --example probe

use chrono::NaiveDate;

fn main() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
    for line in [
        "pick up a parcel on tuesday 8pm",
        "pick up a parcel tuesday 8pm",
        "pick up a parcel on tuesday",
        "x on tuesday 8pm",
        "x tuesday 8pm",
        "x on tue 8pm",
    ] {
        let p = ms_core::parse(line, today);
        println!("{line:?}");
        println!("   title    {:?}", p.title);
        println!("   consumed {:?}", p.consumed);
        println!("   byday {:?}  span {:?}  loc {:?}  repeats {}", p.byday, p.span, p.location, p.repeats);
        println!();
    }
}
