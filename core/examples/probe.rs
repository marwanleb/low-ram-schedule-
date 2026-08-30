//! Scratch harness for looking at what the parser does with a given line.
//!
//!     cargo run -p ms-core --example probe

use chrono::NaiveDate;

fn main() {
    let today = NaiveDate::from_ymd_opt(2026, 8, 31).unwrap();
    for line in [
        "PHYS201 every monday wedneday 10:30-12:00p @Hall 2.106",
        "PHYS201 every monday wednesday 10:30-12:00p @Hall 2.106",
        "PHYS201 every mon wed 10:30-12:00 @ECJ",
        "x every monday",
        "x 12:00p",
        "x @Hall 2.106",
    ] {
        let p = ms_core::parse(line, today);
        println!("{line:?}");
        println!("   title    {:?}", p.title);
        println!("   consumed {:?}", p.consumed);
        println!("   byday {:?}  span {:?}  loc {:?}  repeats {}", p.byday, p.span, p.location, p.repeats);
        println!();
    }
}
