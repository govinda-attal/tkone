use std::marker::PhantomData;
use std::sync::Arc;

use super::spec::{self, BizDayStep, Cycle, DayCycle, DayOverflow, Spec};
use crate::biz_day::WeekendSkipper;
use crate::{biz_day::BizDayProcessor, prelude::*};
use chrono::{
    DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc,
};
use fallible_iterator::FallibleIterator;

pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;


pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    start: START,
    spec: String,
    bd_processor: BDP,
    end: END,
    timezone: Tz,
    marker_sealed: PhantomData<S>,
}


impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    pub fn new(
        spec: &str,
        bdp: BDP,
        tz: &Tz,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Utc>, NoEnd, NotSealed> {
        let now = Utc::now();
        let now = now.timezone().with_ymd_and_hms(now.year(), now.month(), now.day(), now.hour(), now.minute(), now.second()).unwrap();
        SpecIteratorBuilder {
            start: StartDateTime(now),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
            timezone: tz.clone(),
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.timezone.from_utc_datetime(&Utc::now().naive_utc()).with_timezone(&self.timezone);
        Ok(SpecIterator{
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(&self.spec, start.naive_local(), self.bd_processor)?,
        })
    }
}



impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
        SpecIteratorBuilder {
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndSpec(end_spec.into()),
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }
}
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder{
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        let start = start.timezone().with_ymd_and_hms(start.year(), start.month(), start.day(), start.hour(), start.minute(), start.second()).unwrap();
        Ok(SpecIterator{
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                &self.spec,
                start.naive_local(),
                self.bd_processor,
                self.end.0.naive_local(),
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        let start = start.timezone().with_ymd_and_hms(start.year(), start.month(), start.day(), start.hour(), start.minute(), start.second()).unwrap();
        Ok(SpecIterator{
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
                &self.spec,
                start.naive_local(),
                self.bd_processor,
                &self.end.0,
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
    pub fn new_with_start(
        spec: &str,
        start: DateTime<Tz>,
        bdp: BDP,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            timezone: start.timezone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
        }
    }

}
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::<Tz, BDP>{
            tz: self.start.0.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(&self.spec, self.start.0.naive_local(), self.bd_processor)?,
        })
    }
}


#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator<BDP>,
}

// impl<Tz: TimeZone, BDM: BizDayProcessor> SpecIterator<Tz, BDM>{
//     // fn new(spec: &str, start: DateTime<Tz>, bd_processor: BDM) -> Result<Self> {
//     //     Ok(Self {
//     //         tz: start.timezone(),
//     //         naive_spec_iter: NaiveSpecIterator::new(spec, start.naive_local(), bd_processor)?,
//     //     })
//     // }

//     // pub fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>, bd_processor: BDM) -> Result<Self> {
//     //     Ok(Self {
//     //         tz: start.timezone(),
//     //         naive_spec_iter: NaiveSpecIterator::new_with_end(
//     //             spec,
//     //             start.naive_local(),
//     //             bd_processor,
//     //             end.naive_local(),
//     //         )?,
//     //     })
//     // }

//     // pub fn new_with_end_spec(spec: &str, start: DateTime<Tz>, end_spec: &str, bd_processor: BDM) -> Result<Self> {
//     //     // Ok(Self {
//     //     //     tz: start.timezone(),
//     //     //     naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
//     //     //         spec,
//     //     //         start.naive_local(),
//     //     //         bd_processor,
//     //     //         end_spec,
//     //     //     )?,
//     //     // })
//     // }
// }

impl<Tz: TimeZone, BDM: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDM> {
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
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    spec: Spec,
    end: Option<NaiveDateTime>,
    remaining: Option<u32>,
    dtm: NaiveDateTime,
    bd_processor: BDP,
}

impl <BDP: BizDayProcessor>NaiveSpecIterator<BDP> {
    pub fn new(spec: &str, start: NaiveDateTime, bdp: BDP) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start,
            spec,
            bd_processor: bdp,
            end: None,
            remaining: None,
        })
    }

    pub fn new_with_end(spec: &str, start: NaiveDateTime,bdp: BDP, end: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start,
            end: Some(end),
            spec,
            bd_processor: bdp,
            remaining: None,
        })
    }

    pub fn new_with_end_spec(spec: &str, start: NaiveDateTime, bdp: BDP, end_spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        let end = Self::new(end_spec, start.clone(), bdp.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            end: Some(end),
            spec,
            dtm: start,
            bd_processor: bdp,
            remaining: None,
        })
    }
}

impl <BDP: BizDayProcessor>FallibleIterator for NaiveSpecIterator<BDP> {
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
            (Cycle::NA, Cycle::Every(num_months), DayCycle::On(day, day_overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months);
                
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

fn ffwd_months(dtm: &NaiveDateTime, num: u32) -> (u32, u32) {
    let mut new_month = dtm.month() + num;
    let mut new_year = dtm.year() as u32;
    new_year += (new_month - 1) / 12;
    new_month = (new_month - 1) % 12 + 1;
    (new_year, new_month)
}

#[cfg(test)]
mod tests {
    use crate::biz_day;

    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 11, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter = SpecIteratorBuilder::new_with_start("YY:1M:DD", dtm, WeekendSkipper{}).build().unwrap();
        dbg!(spec_iter.take(14).collect::<Vec<DateTime<_>>>().unwrap());
    }
}

#[derive(Debug)]
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
