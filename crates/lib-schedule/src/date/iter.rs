use std::default;

use super::spec::{self, BizDayStep, Cycle, DayCycle, DayOverflow, Spec};
use super::HandleOverflow;
use crate::biz_day::WeekendSkipper;
use crate::{biz_day::BizDayProcessor, prelude::*};
use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
};
use datetime_default::DateTimeDefaultNow;
use derivative::Derivative;
use fallible_iterator::FallibleIterator;

#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator,
    bd_processor: WeekendSkipper, // Using the example BizDateProcessor
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    pub fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(spec, start.naive_local())?,
            bd_processor: WeekendSkipper {},
        })
    }

    pub fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                spec,
                start.naive_local(),
                end.naive_local(),
            )?,
            bd_processor: WeekendSkipper {},
        })
    }

    pub fn new_with_end_spec(spec: &str, start: DateTime<Tz>, end_spec: &str) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
                spec,
                start.naive_local(),
                end_spec,
            )?,
            bd_processor: WeekendSkipper {},
        })
    }
}

impl<Tz: TimeZone> FallibleIterator for SpecIterator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let next = self.naive_spec_iter.next()?;
        let Some(next) = next else {
            return Ok(None);
        };

        Ok(Some(Self::Item::from(W((self.tz.clone(), next.clone())))))
    }
}
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator {
    spec: Spec,
    end: Option<NaiveDateTime>,
    remaining: Option<u32>,
    dtm: NaiveDateTime,
    bd_processor: WeekendSkipper, // Using the example BizDateProcessor
}

impl NaiveSpecIterator {
    pub fn new(spec: &str, start: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start,
            spec,
            bd_processor: WeekendSkipper {},
            end: None,
            remaining: None,
        })
    }

    pub fn new_with_end(spec: &str, start: NaiveDateTime, end: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start,
            end: Some(end),
            spec,
            bd_processor: WeekendSkipper {},
            remaining: None,
        })
    }

    pub fn new_with_end_spec(spec: &str, start: NaiveDateTime, end_spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        let end = Self::new(end_spec, start.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            end: Some(end),
            spec,
            dtm: start,
            bd_processor: WeekendSkipper {},
            remaining: None,
        })
    }
}

impl FallibleIterator for NaiveSpecIterator {
    type Item = NaiveDateTime;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let remaining = if let Some(remaining) = self.remaining {
            if remaining == 0 {
                return Ok(None);
            }
            Some(remaining - 1)
        } else {
            None
        };

        if let Some(end) = &self.end {
            if &self.dtm >= end {
                return Ok(None);
            }
        }

        let next = self.dtm.clone();
        dbg!(&next, "<><><><><>");

        let spec = (&self.spec.years, &self.spec.months, &self.spec.days);

        let next = match spec {
            (Cycle::NA, Cycle::NA, DayCycle::NA) => next,
            (Cycle::NA, Cycle::NA, DayCycle::On(day, overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .build()
            }
            (Cycle::NA, Cycle::NA, DayCycle::Every(num)) => next + Duration::days(*num as i64),
            (Cycle::NA, Cycle::NA, DayCycle::EveryBizDay(num)) => {
                self.bd_processor.add(&next, *num)?
            }
            (Cycle::NA, Cycle::NA, DayCycle::Overflow(overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, overflow).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::NA) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::On(day, overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(*month)
                .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::Every(num)) => {
                let next = next + Duration::days(*num as i64);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::EveryBizDay(num)) => {
                let next = self.bd_processor.add(&next, *num)?;
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::Overflow(overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, overflow)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num), DayCycle::On(day, day_overflow)) => {
                let (year, month) = ffwd_months(&next, *num);
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(month)
                .year(year)
                .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::Overflow(day_overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::NA) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::On(day, day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::Overflow(day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::NA) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::On(day, day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(*month)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::Overflow(day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::NA) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::On(day, day_overflow)) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(month)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (_, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (_, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::Overflow(day_overflow)) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::NA) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::On(day, day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .year(next.year() as u32 + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::Overflow(day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::NA) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::On(day, day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(*month)
                .year(next.year() as u32 + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::Overflow(day_overflow)) => {
                NaiveDateTimeWithOverflowBuilder::new(&next, day_overflow)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::Every(num_months),
                DayCycle::On(day, day_overflow),
            ) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NaiveDateTimeWithOverflowBuilder::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::MonthLastDay),
                )
                .day(*day)
                .month(month)
                .year(year + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::Every(num_months),
                DayCycle::EveryBizDay(num_days),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NaiveDateTimeWithOverflowBuilder::new(&next, &DayOverflow::MonthLastDay)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::Overflow(overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NaiveDateTimeWithOverflowBuilder::new(&next, overflow)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
        };

        let next = if let Some(biz_day_step) = &self.spec.biz_day_step {
            if self.bd_processor.is_biz_day(&next)? {
                next
            } else {
                match biz_day_step {
                    BizDayStep::Prev(num) => self.bd_processor.sub(&next, *num)?,
                    BizDayStep::Next(num) => self.bd_processor.add(&next, *num)?,
                    BizDayStep::NA => next,
                }
            }
        } else {
            next
        };

        if next <= self.dtm {
            return Ok(None);
        }

        self.dtm = next;
        self.remaining = remaining;
        Ok(Some(self.dtm.clone()))
    }
}

fn month_end_date(dtm: &NaiveDateTime) -> NaiveDateTime {
    // Calculate the year and month from the input datetime
    let year = dtm.year();
    let month = dtm.month();

    // Create a NaiveDate for the first day of the next month.
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_month_date = NaiveDate::from_ymd_opt(year, next_month, 1)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap());

    // Subtract one day to get the last day of the current month.
    let last_day_of_month = next_month_date.pred_opt().unwrap();

    // Construct the NaiveDateTime for the last day of the month
    // with the same time as the input datetime.
    NaiveDateTime::new(last_day_of_month, dtm.time())
}

fn day_or_month_end(dtm: &NaiveDateTime, num: u8) -> NaiveDateTime {
    let last_day_of_month = month_end_date(dtm).day();
    let target_day = if num as u32 > last_day_of_month {
        last_day_of_month
    } else {
        num as u32
    };
    dtm.with_day(target_day as u32).unwrap()
}

fn ffwd_months(dtm: &NaiveDateTime, num: u32) -> (u32, u32) {
    let mut new_month = dtm.month() + num;
    let mut new_year = dtm.year() as u32;
    new_year += (new_month - 1) / 12;
    new_month = (new_month - 1) % 12 + 1;
    (new_year, new_month)
}

// fn ffwd_months(dtm: NaiveDateTime, num: u8, day_cycle: &DayCycle) -> NaiveDateTime {
//     let mut new_month = dtm.month() + num as u32;
//     let mut new_year = dtm.year() as u32;
//     new_year += (new_month - 1) / 12;
//     new_month = (new_month - 1) % 12 + 1;

//     dbg!(&new_year, &new_month, &dtm);
//     let dtm = adjust_to_last_day(&dtm, dtm.with_year(new_year as i32), day_cycle);
//     dbg!(&new_year, &new_month, &dtm);
//     adjust_to_last_day(&dtm, dtm.with_month(new_month as u32), day_cycle)
// }

fn add_days_in_timezone<Tz: TimeZone>(dtm: &DateTime<Tz>, num: i64) -> DateTime<Tz> {
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

// fn update_day_or_overflow(dtm: &NaiveDateTime, day: u8, overflow: Option<&spec::DayOverflow>) -> NaiveDateTime {
//     if let Some(dtm) = dtm.with_day(day) {
//         return dtm;
//     }
//     let dtm_overflows
//     let overflow = overflow.unwrap_or_default();
//     dtm.day_overflows(overflow)
// }

fn adjust_to_last_day(
    orig: &NaiveDateTime,
    updated: Option<NaiveDateTime>,
    day_cycle: &DayCycle,
) -> NaiveDateTime {
    updated.unwrap()
    // if updated.is_some() {
    //     dbg!(&orig, &updated);
    //     return updated.unwrap();
    // }

    // let next_month_end_date = month_end_date(&(month_end_date(orig) + Duration::days(1)));
    // let target_day = match day_cycle {
    //     DayCycle::LastDay(None) => next_month_end_date.day(),
    //     DayCycle::LastDay(Some(day)) => {
    //         if *day as u32 > next_month_end_date.day() {
    //             next_month_end_date.day()
    //         } else {
    //             *day as u32
    //         }
    //     }
    //     _ => next_month_end_date.day(),
    // };
    // let updated = next_month_end_date.with_day(target_day).unwrap();
    // dbg!(&updated);
    // updated
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);
        let spec_iter = SpecIterator::new("YY:1M:DD", dtm).unwrap();
        dbg!(spec_iter.take(14).collect::<Vec<DateTime<_>>>().unwrap());
    }

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

struct NaiveDateTimeWithOverflowBuilder<'a> {
    dtm: &'a NaiveDateTime,
    overflow: &'a DayOverflow,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<u32>,
}

impl<'a> NaiveDateTimeWithOverflowBuilder<'a> {
    pub fn new(dtm: &'a NaiveDateTime, overflow: &'a DayOverflow) -> Self {
        Self {
            dtm,
            overflow,
            day: None,
            month: None,
            year: None,
        }
    }

    pub fn day(&mut self, day: u32) -> &mut Self {
        self.day = Some(day);
        self
    }

    pub fn month(&mut self, month: u32) -> &mut Self {
        self.month = Some(month);
        self
    }

    pub fn year(&mut self, year: u32) -> &mut Self {
        self.year = Some(year);
        self
    }

    pub fn build(&self) -> NaiveDateTime {
        let dtm = self.dtm.clone();
        let day = self.day.unwrap_or(dtm.day());
        let month = self.month.unwrap_or(dtm.month());
        let year = self
            .year
            .map(|year| year as i32)
            .unwrap_or(dtm.year() as i32);
        if let Some(updated) = NaiveDate::from_ymd_opt(year, month, day) {
            return NaiveDateTime::new(updated, dtm.time());
        }

        let overflow = self.overflow;
        match overflow {
            spec::DayOverflow::MonthLastDay => {
                let next_month_first_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day_of_month = next_month_first_day.pred_opt().unwrap();
                NaiveDateTime::new(last_day_of_month, dtm.time())
            }
            spec::DayOverflow::NextMonthFirstDay => {
                let next_month_first_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                NaiveDateTime::new(next_month_first_day, dtm.time())
            }
            spec::DayOverflow::NextMonthOverflow => {
                let next_month_first_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let dtm_last_day = next_month_first_day.pred_opt().unwrap().day();
                dtm + Duration::days(day as i64 - dtm_last_day as i64)
            }
        }
    }
}
