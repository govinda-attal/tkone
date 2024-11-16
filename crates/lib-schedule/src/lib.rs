#![feature(let_chains)]

pub mod biz_day;
pub mod date;
pub mod datetime;
mod error;
mod prelude;
pub mod time;
mod utils;

#[derive(Debug, Clone)]
pub enum NextResult<T: Clone> {
    Single(T),
    AdjustedLater(T, T),
    AdjustedEarlier(T, T),
}

impl<T: Clone> NextResult<T> {
    pub fn single(self) -> Option<T> {
        match self {
            NextResult::Single(t) => Some(t),
            _ => None,
        }
    }

    pub fn earlier(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    pub fn later(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    pub fn observed(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(_, t)
            | NextResult::AdjustedLater(_, t) => t,
        }
    }

    pub fn actual(&self) -> &T {
        match self {
            NextResult::Single(t)
            | NextResult::AdjustedEarlier(t, _)
            | NextResult::AdjustedLater(t, _) => t,
        }
    }

    pub fn as_tuple(&self) -> (&T, &T) {
        match self {
            NextResult::Single(t) => (t, t),
            NextResult::AdjustedEarlier(actual, adjusted)
            | NextResult::AdjustedLater(actual, adjusted) => (actual, adjusted),
        }
    }
}

#[cfg(test)]
mod tests {
    use biz_day::WeekendSkipper;
    use chrono::{DateTime, TimeZone};
    use chrono_tz::America::New_York;
    use fallible_iterator::FallibleIterator;

    use super::*;
    #[test]
    fn test_works() {
        let tmp = datetime::SpecIteratorBuilder::new_with_start(
            "YY:1M:8WT11:00:00",
            WeekendSkipper::new(),
            New_York.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap(),
        )
        .with_end(New_York.with_ymd_and_hms(2025, 7, 31, 11, 00, 0).unwrap())
        .build()
        .unwrap();
        let tmp = tmp
            .take(10)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap();
        dbg!(&tmp);
    }
}
