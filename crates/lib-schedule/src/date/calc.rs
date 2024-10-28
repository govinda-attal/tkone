use super::spec::{Cycle, DayCycle, Spec};
use crate::biz_day::WeekendSkipper;
use crate::{biz_day::BizDayProcessor, prelude::*};
use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

struct Calculator {
    start: DateTime<Tz>,
    end: Option<DateTime<Tz>>,
    remaining: Option<u32>,
    spec: Spec,
    dtm: DateTime<Tz>,
    bd_processor: WeekendSkipper, // Using the example BizDateProcessor
}

impl Calculator {
    fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            dtm: start,
            end: None,
            remaining: None,
            spec,
            bd_processor: WeekendSkipper {},
        })
    }

    fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            dtm: start,
            end: Some(end),
            remaining: None,
            spec,
            bd_processor: WeekendSkipper {},
        })
    }

    fn new_with_max(spec: &str, start: DateTime<Tz>, max: u32) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            dtm: start,
            end: None,
            remaining: Some(max),
            spec,
            bd_processor: WeekendSkipper {},
        })
    }
}

impl FallibleIterator for Calculator {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if let Some(remaining) = self.remaining {
            if remaining == 0 {
                return Ok(None);
            }
            self.remaining = Some(remaining - 1);
        }

        if let Some(end) = &self.end {
            if &self.dtm >= end {
                return Ok(None);
            }
        }

        let next = self.dtm.clone();
        let next = match &self.spec.days {
            DayCycle::NA => next,
            DayCycle::On(day) => next.with_day(*day as u32).unwrap(),
            DayCycle::Every(num) => next + Duration::days(*num as i64),
            DayCycle::EveryBizDay(num) => self.bd_processor.add(&next, *num)?,
            DayCycle::LastDay(Some(num)) => day_or_month_end(&next, *num),
            DayCycle::LastDay(None) => month_end(&next),
        };

        let next = match &self.spec.months {
            Cycle::NA => next,
            Cycle::In(num) => next.with_month(*num as u32).unwrap(),
            Cycle::Every(num) => ffwd_months(next, *num as u8),
        };

        let next = match &self.spec.years {
            Cycle::NA => next,
            Cycle::In(num) => next.with_year(*num as i32).unwrap(),
            Cycle::Every(num) => next.with_year(next.year() + *num as i32).unwrap(),
        };

        if next <= self.dtm {
            return Ok(None);
        }

        self.dtm = next;
        Ok(Some(self.dtm.clone()))
    }
}

fn month_end(dtm: &DateTime<Tz>) -> DateTime<Tz> {
    let next_month = if dtm.month() == 12 {
        dtm.with_year(dtm.year() + 1)
            .unwrap()
            .with_month(1)
            .unwrap()
    } else {
        dtm.with_month(dtm.month() + 1).unwrap()
    };
    next_month.with_day(1).unwrap() - Duration::days(1)
}

fn day_or_month_end(dtm: &DateTime<Tz>, num: u8) -> DateTime<Tz> {
    let last_day_of_month = month_end(dtm).day();
    let target_day = if num as u32 > last_day_of_month {
        last_day_of_month
    } else {
        num as u32
    };
    dtm.with_day(target_day as u32).unwrap()
}

fn ffwd_months(dtm: DateTime<Tz>, num: u8) -> DateTime<Tz> {
    let mut new_month = dtm.month() as i64 + num as i64;
    let mut new_year = dtm.year();
    while new_month > 12 {
        new_month -= 12;
        new_year += 1;
    }
    dtm.with_year(new_year)
        .unwrap()
        .with_month(new_month as u32)
        .unwrap()
}

fn add_days_in_timezone(dtm: &DateTime<Tz>, num: i64) -> DateTime<Tz> {
    // Extract the time portion from the given DateTime
    let time = dtm.time();
    // Convert from the given timezone to UTC
    let utc_dt = dtm.with_timezone(&Utc);

    // Add the duration in UTC
    let adjusted_dtm = utc_dt + Duration::days(num);

    // Convert back to the given timezone
    let adjusted_dtm = adjusted_dtm.with_timezone(&dtm.timezone());

    // Adjust the time to keep it constant
    adjusted_dtm.date().and_time(time).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_add_days_in_timezone_standard_to_dst() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 12, 0, 0).unwrap();
        let result = add_days_in_timezone(&dtm, 1);
        dbg!(&dtm, &result);
        let expected = est.with_ymd_and_hms(2023, 3, 12, 12, 0, 0).unwrap(); // DST starts on March 12, 2023
        assert_eq!(result, expected);
    }

    #[test]
    fn test_add_days_in_timezone_dst_to_standard() {
        // US Eastern Time (EST/EDT)
        let us_east = New_York;

        // Before DST ends (Daylight Saving Time)
        let dtm = us_east.with_ymd_and_hms(2023, 11, 4, 12, 0, 0).unwrap();
        let result = add_days_in_timezone(&dtm, 1);
        let expected = us_east.with_ymd_and_hms(2023, 11, 5, 12, 0, 0).unwrap(); // DST ends on November 5, 2023
        dbg!(&dtm, &result, &expected);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_add_days_in_timezone_standard_time() {
        // US Eastern Time (EST/EDT)
        let us_east = New_York;

        // Standard Time
        let dtm = us_east.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap();
        let result = add_days_in_timezone(&dtm, 1);
        let expected = us_east.with_ymd_and_hms(2023, 1, 2, 12, 0, 0).unwrap();
        dbg!(&dtm, &result, &expected);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_add_days_in_timezone_daylight_saving_time() {
        // US Eastern Time (EST/EDT)
        let us_east = New_York;

        // Daylight Saving Time
        let dtm = us_east.with_ymd_and_hms(2023, 6, 1, 12, 0, 0).unwrap();
        let result = add_days_in_timezone(&dtm, 1);
        let expected = us_east.with_ymd_and_hms(2023, 6, 2, 12, 0, 0).unwrap();
        dbg!(&dtm, &result.fixed_offset(), &expected.fixed_offset());
        assert_eq!(result, expected);
    }
}
