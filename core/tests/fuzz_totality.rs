//! Property test for the spec 5.1 guarantee: get_week never panics and never
//! returns more than a week, for arbitrary stored data and arbitrary anchors.
//!
//! Deterministic: SEED is fixed and recorded so a failure is reproducible.

use chrono::NaiveDate;
use chrono_tz::{Tz, TZ_VARIANTS};
use ms_core::{get_week, Db};

const SEED: u64 = 0x5EED_1234_ABCD_0001;
const CASES: usize = 4000;

/// xorshift64*, so the test carries no dependency and stays reproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[(self.next() % xs.len() as u64) as usize]
    }
}

const BYDAYS: &[&str] = &["mon", "mon,wed", "", "funday", "MON,tue", ",,,", "sun,sun", "mon,,wed", "🙂"];
const TIMES: &[&str] = &["09:00", "00:00", "23:59", "2:30", "25:00", "", "abc", "09:60", "-1:00", "02:30"];
const DATES: &[&str] = &["2026-08-01", "1900-01-01", "9999-12-31", "", "not-a-date", "2026-02-30", "0000-01-01"];
const ZONES: &[&str] = &["America/Chicago", "Europe/Paris", "Asia/Beirut", "US/Central", "Mars/Olympus", "", "UTC"];
const EXCEPTS: &[&str] = &["", "2026-09-02", "garbage", "2026-09-02,junk,", ",,", "2026-13-45"];

#[test]
fn get_week_never_panics_on_arbitrary_data() {
    let mut rng = Rng(SEED);
    let zones: Vec<Tz> = TZ_VARIANTS.to_vec();

    for case in 0..CASES {
        let db = Db::open_in_memory().unwrap();

        for i in 0..(rng.next() % 4) {
            let id = format!("i{i}");
            let _ = db.conn.execute(
                "INSERT INTO items (id, title, created_at) VALUES (?1, ?1, '2026-01-01T00:00:00Z')",
                rusqlite::params![id],
            );
            let until: Option<&str> = if rng.next() % 2 == 0 { None } else { Some(rng.pick(DATES)) };
            let _ = db.conn.execute(
                "INSERT INTO recurrence (item_id, byday, start_time, end_time, tz, from_date, until_date, except_on)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    id,
                    rng.pick(BYDAYS),
                    rng.pick(TIMES),
                    rng.pick(TIMES),
                    rng.pick(ZONES),
                    rng.pick(DATES),
                    until,
                    rng.pick(EXCEPTS)
                ],
            );
        }

        // Anchors across the whole representable calendar, including the edges.
        let anchor = match rng.next() % 10 {
            0 => NaiveDate::MAX,
            1 => NaiveDate::MIN,
            2 => NaiveDate::MAX - chrono::Duration::days((rng.next() % 9) as i64),
            3 => NaiveDate::MIN + chrono::Duration::days((rng.next() % 9) as i64),
            _ => NaiveDate::from_num_days_from_ce_opt((rng.next() % 3_000_000) as i32 - 500_000)
                .unwrap_or(NaiveDate::MAX),
        };
        let viewing = zones[(rng.next() % zones.len() as u64) as usize];

        let week = get_week(&db, anchor, viewing);

        assert!(week.days.len() <= 7, "case {case}: {} days", week.days.len());
        for day in &week.days {
            for p in &day.placements {
                assert!(p.ends_at >= p.starts_at, "case {case}: negative span");
            }
        }
    }
}
