use super::spec::{Cycle, DayCycle, Spec};
use crate::biz_day::WeekendSkipper;
use crate::{biz_day::BizDayProcessor, prelude::*};
use chrono::{DateTime, Datelike, Duration, TimeZone};
use fallible_iterator::FallibleIterator;

struct Calculator<Tz: TimeZone> {
    spec: Spec,
    dtm: DateTime<Tz>,
    bd_processor: WeekendSkipper, // Using the example BizDateProcessor
}

impl<Tz: TimeZone> Calculator<Tz> {
    fn new(dtm: DateTime<Tz>, spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            bd_processor: WeekendSkipper {},
        })
    }
}

impl<Tz: TimeZone> FallibleIterator for Calculator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
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
            return Err(Error::NextDateCalcError);
        }

        self.dtm = next;
        Ok(Some(self.dtm.clone()))
    }
}

fn month_end<Tz: TimeZone>(dtm: &DateTime<Tz>) -> DateTime<Tz> {
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

fn day_or_month_end<Tz: TimeZone>(dtm: &DateTime<Tz>, num: u8) -> DateTime<Tz> {
    let last_day_of_month = month_end(dtm).day();
    let target_day = if num as u32 > last_day_of_month {
        last_day_of_month
    } else {
        num as u32
    };
    dtm.with_day(target_day as u32).unwrap()
}

fn ffwd_months<Tz: TimeZone>(dtm: DateTime<Tz>, num: u8) -> DateTime<Tz> {
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
