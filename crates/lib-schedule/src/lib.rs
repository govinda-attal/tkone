mod biz_day;
pub mod date;
pub mod datetime;
mod error;
mod prelude;
pub mod time;
mod utils;

use biz_day::BizDayProcessor;


#[cfg(test)]
mod tests {
    use biz_day::WeekendSkipper;
    use chrono::{DateTime, TimeZone, Utc};
    use fallible_iterator::FallibleIterator;

    use super::*;
    #[test]
    fn test_works() {
        let tmp = datetime::SpecIteratorBuilder::new(
            "YY:1M:31LT11:00:00",
            &Utc,
            WeekendSkipper::new(),
        )
        .with_end(Utc::with_ymd_and_hms(&Utc, 2025, 07, 31, 11, 00, 0).unwrap())
        .build()
        .unwrap();
        let tmp = tmp.take(6).collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(&tmp);
    }
}
