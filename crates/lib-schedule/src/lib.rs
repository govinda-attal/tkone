pub mod biz_day;
pub mod date;
pub mod datetime;
mod error;
mod prelude;
pub mod time;
mod utils;

#[cfg(test)]
mod tests {
    use biz_day::WeekendSkipper;
    use chrono::{DateTime, TimeZone, Utc};
    use fallible_iterator::FallibleIterator;

    use super::*;
    #[test]
    fn test_works() {
        let tmp = datetime::SpecIteratorBuilder::new_with_start(
            "YY:1M:31LT11:00:00",
            WeekendSkipper::new(),
            Utc.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap(),
        )
        .with_end(Utc::with_ymd_and_hms(&Utc, 2025, 07, 31, 11, 00, 0).unwrap())
        .build()
        .unwrap();
        let tmp = tmp.take(6).collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(&tmp);
    }
}
