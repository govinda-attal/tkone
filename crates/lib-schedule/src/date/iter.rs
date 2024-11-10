use std::marker::PhantomData;

use super::spec::{self, BizDayStep, Cycle, DayCycle, DayOverflow, Spec};
use crate::{biz_day::BizDayProcessor, prelude::*, NextResult};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use fallible_iterator::FallibleIterator;

pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    dtm: DateTime<Tz>,
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
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        let now = Utc::now();
        let now = tz
            .with_ymd_and_hms(
                now.year(),
                now.month(),
                now.day(),
                now.hour(),
                now.minute(),
                now.second(),
            )
            .unwrap();
        SpecIteratorBuilder {
            dtm: now,
            start: NoStart,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
            timezone: tz.clone(),
        }
    }

    pub fn new_after(
        spec: &str,
        bdp: BDP,
        dtm: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            timezone: dtm.timezone(),
            dtm,
            start: NoStart,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator {
            tz: self.dtm.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(
                &self.spec,
                self.bd_processor,
                self.dtm.naive_local(),
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndSpec(end_spec.into()),
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }
}
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        let start = start
            .timezone()
            .with_ymd_and_hms(
                start.year(),
                start.month(),
                start.day(),
                start.hour(),
                start.minute(),
                start.second(),
            )
            .unwrap();
        Ok(SpecIterator {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                &self.spec,
                self.bd_processor,
                start.naive_local(),
                self.end.0.naive_local(),
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        let start = start
            .timezone()
            .with_ymd_and_hms(
                start.year(),
                start.month(),
                start.day(),
                start.hour(),
                start.minute(),
                start.second(),
            )
            .unwrap();
        Ok(SpecIterator {
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

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    pub fn new_with_start(
        spec: &str,
        bdp: BDP,
        start: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            dtm: start.clone(),
            timezone: start.timezone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
        }
    }
}
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::<Tz, BDP> {
            tz: self.start.0.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_start(
                &self.spec,
                self.bd_processor,
                self.start.0.naive_local(),
            )?,
        })
    }
}

#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator<BDP>,
}

impl<Tz: TimeZone, BDM: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDM> {
    type Item = NextResult<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let next = self.naive_spec_iter.next()?;
        let Some(next) = next else {
            return Ok(None);
        };
        Ok(Some(Self::Item::from(W((self.tz.clone(), next)))))
    }
}

#[derive(Debug, Clone)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    spec: Spec,
    dtm: NaiveDateTime,
    bd_processor: BDP,
    index: usize,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    remaining: Option<u32>,
}

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    fn new(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            bd_processor: bdp,
            index: 0,
            start: None,
            end: None,
            remaining: None,
        })
    }

    fn new_with_start(spec: &str, bdp: BDP, start: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            bd_processor: bdp,
            index: 0,
            start: Some(start),
            end: None,
            remaining: None,
        })
    }

    fn new_with_end(
        spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            bd_processor: bdp,
            index: 0,
            start: Some(start),
            end: Some(end),
            remaining: None,
        })
    }

    fn new_with_end_spec(
        spec: &str,
        start: NaiveDateTime,
        bdp: BDP,
        end_spec: &str,
    ) -> Result<Self> {
        let spec = spec.parse()?;
        let end = Self::new_with_start(end_spec, bdp.clone(), start.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            bd_processor: bdp,
            index: 0,
            start: Some(start),
            end: Some(end.observed().clone()),
            remaining: None,
        })
    }
}

impl<BDP: BizDayProcessor> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = NextResult<NaiveDateTime>;
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

        if self.index == 0 {
            if let Some(start) = &self.start {
                if &self.dtm <= start {
                    self.dtm = start.clone();
                    self.remaining = remaining;
                    self.index += 1;
                    return Ok(Some(NextResult::Single(start.clone())));
                }
            }
        }

        let next = self.dtm.clone();

        let spec = (&self.spec.years, &self.spec.months, &self.spec.days);

        let next = match spec {
            (Cycle::NA, Cycle::NA, DayCycle::NA) => NextResult::Single(next),
            (Cycle::NA, Cycle::NA, DayCycle::On(day, overflow)) => {
                NextResulterByDay::new(
                    &next,
                    overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .build()
            }
            (Cycle::NA, Cycle::NA, DayCycle::Every(num)) => {
                NextResult::Single(next + Duration::days(*num as i64))
            }
            (Cycle::NA, Cycle::NA, DayCycle::EveryBizDay(num)) => {
                NextResult::Single(self.bd_processor.add(&next, *num)?)
            }
            // (Cycle::NA, Cycle::NA, DayCycle::WeekDay(num, wd_opt)) => {
            //     
            // }
            (Cycle::NA, Cycle::NA, DayCycle::Overflow(overflow)) => {
                NextResulterByDay::new(&next, overflow).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::NA) => {
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::On(day, overflow)) => {
                NextResulterByDay::new(
                    &next,
                    overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(*month)
                .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::Every(num)) => {
                let next = next + Duration::days(*num as i64);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::EveryBizDay(num)) => {
                let next = self.bd_processor.add(&next, *num)?;
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .build()
            }
            // (Cycle::NA, Cycle::In(month), DayCycle::WeekDay(num, wd_opt)) => {

            // }
            (Cycle::NA, Cycle::In(month), DayCycle::Overflow(overflow)) => {
                NextResulterByDay::new(&next, overflow)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::On(day, day_overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months);

                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(month)
                .year(year)
                .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(year)
                    .build()
            }
            // (Cycle::NA, Cycle::Every(num_months), DayCycle::WeekDay(num, wd_opt)) => {

            // }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::Overflow(day_overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months);
                dbg!(year, month);
                NextResulterByDay::new(&next, day_overflow)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::NA) => {
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::On(day, day_overflow)) => {
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(*year)
                    .build()
            }
            // (Cycle::In(year), Cycle::NA, DayCycle::WeekDay(num, wd_opt)) => {
            // }
            (Cycle::In(year), Cycle::NA, DayCycle::Overflow(day_overflow)) => {
                NextResulterByDay::new(&next, day_overflow)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::NA) => {
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::On(day, day_overflow)) => {
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(*month)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            // (Cycle::In(year), Cycle::In(month), DayCycle::WeekDay(num, wd_opt)) => {
            // }
            (Cycle::In(year), Cycle::In(month), DayCycle::Overflow(day_overflow)) => {
                NextResulterByDay::new(&next, day_overflow)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::NA) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::On(day, day_overflow)) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(month)
                .year(*year)
                .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(*year)
                    .build()
            }
            // (Cycle::In(year), Cycle::Every(num_months), DayCycle::WeekDay(num, wd_opt)) => {
            // }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::Overflow(day_overflow)) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, day_overflow)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::NA) => {
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::On(day, day_overflow)) => {
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .year(next.year() as u32 + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            // (Cycle::Every(num_years), Cycle::NA, DayCycle::WeekDay(num, wd_opt)) => {
            // }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::Overflow(day_overflow)) => {
                NextResulterByDay::new(&next, day_overflow)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::NA) => {
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::On(day, day_overflow)) => {
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(*month)
                .year(next.year() as u32 + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::EveryBizDay(num_days)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            // (Cycle::Every(num_years), Cycle::In(month), DayCycle::WeekDay(num, wd_opt)) => {
            // }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::Overflow(day_overflow)) => {
                NextResulterByDay::new(&next, day_overflow)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
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
                NextResulterByDay::new(
                    &next,
                    day_overflow.as_ref().unwrap_or(&DayOverflow::LastDay),
                )
                .day(*day)
                .month(month)
                .year(year + *num_years)
                .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::Every(num_days)) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
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
                NextResulterByDay::new(&next, &DayOverflow::LastDay)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            // (
            //     Cycle::Every(num_years),
            //     Cycle::Every(num_months),
            //     DayCycle::WeekDay(num, wd_opt),
            // ) => {

            // }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::Overflow(overflow)) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next, overflow)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
        };

        let next = if let Some(biz_day_step) = &self.spec.biz_day_step {
            let (actual, observed) = (&next).as_tuple();
            if self.bd_processor.is_biz_day(&observed)? {
                next
            } else {
                match biz_day_step {
                    BizDayStep::Prev(num) => NextResult::AdjustedEarlier(
                        actual.clone(),
                        self.bd_processor.sub(observed, *num)?,
                    ),
                    BizDayStep::Next(num) => NextResult::AdjustedLater(
                        actual.clone(),
                        self.bd_processor.add(observed, *num)?,
                    ),
                    BizDayStep::NA => next,
                }
            }
        } else {
            next
        };

        if next.actual() <= &self.dtm {
            return Ok(None);
        }

        self.index += 1;
        self.dtm = next.actual().clone();
        self.remaining = remaining;
        Ok(Some(next))
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
    use crate::biz_day::{self, WeekendSkipper};

    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_time_spec_with_start() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 11, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter = SpecIteratorBuilder::new_with_start("YY:1M:DD", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 31, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_with_start("YY:1M:31F", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }
}

#[derive(Debug)]
struct NextResulterByDay<'a> {
    dtm: &'a NaiveDateTime,
    overflow: &'a DayOverflow,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<u32>,
}

impl<'a> NextResulterByDay<'a> {
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

    pub fn build(&self) -> NextResult<NaiveDateTime> {
        use spec::DayOverflow::*;
        let dtm = self.dtm.clone();

        let month = self.month.unwrap_or(dtm.month());
        let year = self
            .year
            .map(|year| year as i32)
            .unwrap_or(dtm.year() as i32);

        let day = self.day.unwrap_or_else(|| {
            if self.overflow == &DayOverflow::LastDay {
                if month == 12 {
                    let next_day = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
                    let last_day = next_day.pred_opt().unwrap();
                    last_day.day()
                } else {
                    let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                    let last_day = next_day.pred_opt().unwrap();
                    last_day.day()
                }
            } else {
                dtm.day()
            }
        });

        if let Some(updated) = NaiveDate::from_ymd_opt(year, month, day) {
            if self.overflow != &DayOverflow::Weekday {
                return NextResult::Single(NaiveDateTime::new(updated, dtm.time()));
            }

            return match &updated.weekday() {
                &chrono::Weekday::Sat => {
                    if updated.day() == 1 {
                        NextResult::AdjustedLater(
                            NaiveDateTime::new(updated, dtm.time()),
                            NaiveDateTime::new(updated + Duration::days(2), dtm.time()) 
                        )
                    } else {
                        NextResult::AdjustedEarlier(
                            NaiveDateTime::new(updated, dtm.time()),
                            NaiveDateTime::new(updated - Duration::days(1), dtm.time()) 
                        )
                    }
                }
                &chrono::Weekday::Sun => {
                    NextResult::AdjustedLater(
                        NaiveDateTime::new(updated, dtm.time()),
                        NaiveDateTime::new(updated + Duration::days(1), dtm.time()) 
                    )
                }
                _ => NextResult::Single(NaiveDateTime::new(updated, dtm.time())),
            };
        }

        let overflow = self.overflow;
        match overflow {
            LastDay => {
                let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_day.pred_opt().unwrap();
                NextResult::Single(NaiveDateTime::new(last_day, dtm.time()))
            }
            NextMonthFirstDay => {
                let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_day.pred_opt().unwrap();
                NextResult::AdjustedLater(
                    NaiveDateTime::new(last_day, dtm.time()),
                    NaiveDateTime::new(next_day, dtm.time()),
                )
            }
            NextMonthOverflow => {
                let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_day.pred_opt().unwrap();
                let last_day_num = last_day.day();
                NextResult::AdjustedLater(
                    NaiveDateTime::new(last_day, dtm.time()),
                    dtm + Duration::days(day as i64 - last_day_num as i64),
                )
            },
            Weekday | LastWeekday => {
                let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_day.pred_opt().unwrap();
                if last_day.weekday() == chrono::Weekday::Sat {
                    NextResult::AdjustedEarlier(
                        NaiveDateTime::new(last_day, dtm.time()),
                        NaiveDateTime::new(last_day - Duration::days(1), dtm.time()),
                    )
                } else if last_day.weekday() == chrono::Weekday::Sun {
                    NextResult::AdjustedEarlier(
                        NaiveDateTime::new(last_day, dtm.time()),
                        NaiveDateTime::new(last_day - Duration::days(2), dtm.time()),
                    )
                } else {
                    NextResult::Single(NaiveDateTime::new(last_day, dtm.time()))
                }
            },
            NextMonthFirstWeekday => {
                let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_day.pred_opt().unwrap();
                if next_day.weekday() == chrono::Weekday::Sat {
                    NextResult::AdjustedLater(
                        NaiveDateTime::new(last_day, dtm.time()),
                        NaiveDateTime::new(next_day + Duration::days(2), dtm.time()),
                    )
                } else if next_day.weekday() == chrono::Weekday::Sun {
                    NextResult::AdjustedLater(
                        NaiveDateTime::new(last_day, dtm.time()),
                        NaiveDateTime::new(next_day + Duration::days(1), dtm.time()),
                    )
                } else {
                    NextResult::AdjustedLater(
                        NaiveDateTime::new(last_day, dtm.time()),
                        NaiveDateTime::new(next_day, dtm.time()),
                    )
                }
            },
        }
    }
}
