use super::{
    spec::{BizDayAdjustment, Cycle, DayCycle, EveryDayOption, Spec},
    utils::{NextResulterByDay, NextResulterByMultiplesAndDay, NextResulterByWeekDay},
};
use crate::{
    biz_day::{BizDayProcessor, WeekendSkipper},
    prelude::*,
    utils::WeekdayStartingMonday,
    NextResult,
};
use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use fallible_iterator::FallibleIterator;
use std::{collections::BTreeSet, marker::PhantomData};
use std::{ops::Bound, sync::LazyLock};

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
        tz: Tz,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder::new_after(spec, bdp, Utc::now().with_timezone(&tz))
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
            naive_spec_iter: NaiveSpecIterator::new_after(
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

/// # SpecIterator
/// datetime::SpecIterator is an iterator that combines a date and time specification to generate a sequence of date-times.
/// This iterator is created using the SpecIteratorBuilder.
///
/// ## Example
/// ```rust
/// use lib_schedule::biz_day::WeekendSkipper;
/// use lib_schedule::date::SpecIteratorBuilder;
/// use chrono_tz::America::New_York;
/// use fallible_iterator::FallibleIterator;
/// use chrono::{offset::TimeZone, DateTime};
/// use lib_schedule::NextResult;
/// use chrono::Duration;
///
/// let start = New_York.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap();
/// let iter = SpecIteratorBuilder::new_with_start("YY-1M-31L", WeekendSkipper::new(), start).build().unwrap();
/// let occurrences = iter.take(4).collect::<Vec<NextResult<DateTime<_>>>>().unwrap();
/// assert_eq!(occurrences, vec![
///     NextResult::Single(start.clone()), // 2024-11-30
///     NextResult::Single(start + Duration::days(31)), // 2024-12-31
///     NextResult::Single(start + Duration::days(62)), // 2025-01-31
///     NextResult::Single(start + Duration::days(90)), // 2025-02-28
/// ]);
/// ```
///
/// ## See Also
/// - [SpecIteratorBuilder](crate::date::SpecIteratorBuilder)
/// - [SPEC_EXPR](crate::date::SPEC_EXPR)
/// - [NaiveSpecIterator](crate::date::NaiveSpecIterator)
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

impl<Tz: TimeZone, BDM: BizDayProcessor> SpecIterator<Tz, BDM> {
    pub(crate) fn update_cursor(&mut self, dtm: DateTime<Tz>) {
        self.naive_spec_iter.update_cursor(dtm.naive_local());
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
}

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    fn new_after(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            bd_processor: bdp,
            index: 0,
            start: None,
            end: None,
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
        })
    }

    pub(crate) fn update_cursor(&mut self, dtm: NaiveDateTime) {
        self.dtm = dtm;
        self.start = None;
        self.index = 0;
    }
}

impl<BDP: BizDayProcessor> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = NextResult<NaiveDateTime>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if let Some(end) = &self.end {
            if &self.dtm >= end {
                return Ok(None);
            }
        }

        if self.index == 0 {
            if let Some(start) = &self.start {
                if &self.dtm <= start {
                    self.dtm = start.clone();
                    self.index += 1;
                    return Ok(Some(NextResult::Single(start.clone())));
                }
            }
        }

        let next = self.dtm.clone();

        let spec = (&self.spec.years, &self.spec.months, &self.spec.days);

        let next_result = match spec {
            (Cycle::NA, Cycle::NA, DayCycle::NA) => Some(NextResult::Single(next)),
            (Cycle::NA, Cycle::NA, DayCycle::On(day, opt)) => NextResulterByDay::new(&next)
                .last_day_option(opt)
                .day(*day)
                .build(),
            (Cycle::NA, Cycle::NA, DayCycle::Every(num, EveryDayOption::Regular)) => {
                Some(NextResult::Single(next + Duration::days(*num as i64)))
            }
            (Cycle::NA, Cycle::NA, DayCycle::Every(num_days, EveryDayOption::BizDay)) => {
                Some(NextResult::Single(self.bd_processor.add(&next, *num_days)?))
            }
            (Cycle::NA, Cycle::NA, DayCycle::Every(num_days, EveryDayOption::WeekDay)) => {
                Some(NextResult::Single(WEEKEND_SKIPPER.add(&next, *num_days)?))
            }
            (Cycle::NA, Cycle::NA, DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt).build()
            }
            (Cycle::NA, Cycle::NA, DayCycle::OnLastDay) => {
                NextResulterByDay::new(&next).last_day().build()
            }
            (Cycle::NA, Cycle::NA, DayCycle::OnDays(days)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_days(days)
                    .next()
                // validate!("spec not implemented")
            }
            (Cycle::NA, Cycle::NA, DayCycle::OnWeekDays(weekdays)) => {
                let mut next = next + Duration::days(1);
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                }
                Some(NextResult::Single(next))
            }
            (Cycle::NA, Cycle::In(month), DayCycle::NA) => {
                NextResulterByDay::new(&next).month(*month).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::On(day, opt)) => NextResulterByDay::new(&next)
                .last_day_option(opt)
                .day(*day)
                .month(*month)
                .build(),
            (Cycle::NA, Cycle::In(month), DayCycle::Every(num, EveryDayOption::Regular)) => {
                let next = next + Duration::days(*num as i64);
                NextResulterByDay::new(&next).month(*month).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::Every(num_days, EveryDayOption::BizDay)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next).month(*month).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::Every(num_days, EveryDayOption::WeekDay)) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                NextResulterByDay::new(&next).month(*month).build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .month(*month)
                    .build()
            }
            (Cycle::NA, Cycle::In(month), DayCycle::OnLastDay) => NextResulterByDay::new(&next)
                .last_day()
                .month(*month)
                .build(),
            (Cycle::NA, Cycle::In(month), DayCycle::OnDays(days)) => {
                let months = BTreeSet::from([*month]);
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(&months)
                    .with_days(days)
                    .next()
                // let day = next.day();
                // if next.month() == *month && days.contains(&day) {
                //     let next_day = days.lower_bound(std::ops::Bound::Excluded(&day)).next();
                //     if let Some(next_day) = next_day {
                //         NextResult::Single(next.with_day(*next_day).unwrap())
                //     } else {
                //         let first_day = days.first().unwrap();
                //         let next_date =
                //             NaiveDate::from_ymd_opt(next.year() + 1, *month, *first_day);
                //         let next_date = next_date.unwrap_or(
                //             NaiveDate::from_ymd_opt(next.year() + 1, month + 1, 1)
                //                 .unwrap()
                //                 .pred_opt()
                //                 .unwrap(),
                //         );
                //         NextResult::Single(NaiveDateTime::new(next_date, next.time()))
                //     }
                // } else if next.month() > *month {
                //     let next_date =
                //         NaiveDate::from_ymd_opt(next.year() + 1, *month, *days.first().unwrap())
                //             .unwrap();
                //     NextResult::Single(NaiveDateTime::new(next_date, next.time()))
                // } else {
                //     let next_date =
                //         NaiveDate::from_ymd_opt(next.year(), *month, *days.first().unwrap())
                //             .unwrap();
                //     NextResult::Single(NaiveDateTime::new(next_date, next.time()))
                // }
                // validate!("spec not implemented")
            }
            (Cycle::NA, Cycle::In(month), DayCycle::OnWeekDays(weekdays)) => {
                let month = *month as u32;
                let diff = (month as i32) - (next.month() as i32);
                let mut next = if diff > 0 {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(next.year(), month, 1).unwrap(),
                        next.time(),
                    )
                } else if diff < 0 {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(next.year() + 1, month, 1).unwrap(),
                        next.time(),
                    )
                } else {
                    next + Duration::days(1)
                };
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    if next.month() > month {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::NA, Cycle::Every(num), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::On(day, opt)) => {
                let (year, month) = ffwd_months(&next, *num_months);

                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(month)
                    .year(year)
                    .build()
            }
            (
                Cycle::NA,
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build()
            }
            (
                Cycle::NA,
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build()
            }
            (
                Cycle::NA,
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .num_months(*num_months)
                    .build()
            }
            (Cycle::NA, Cycle::Every(num_months), DayCycle::OnLastDay) => {
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .last_day()
                    .month(month)
                    .year(year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::NA) => {
                NextResulterByDay::new(&next).year(*year).build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::On(day, opt)) => NextResulterByDay::new(&next)
                .last_day_option(opt)
                .day(*day)
                .year(*year)
                .build(),
            (Cycle::In(year), Cycle::NA, DayCycle::Every(num_days, EveryDayOption::Regular)) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next).year(*year).build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::Every(num_days, EveryDayOption::BizDay)) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next).year(*year).build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::Every(num_days, EveryDayOption::WeekDay)) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                NextResulterByDay::new(&next).year(*year).build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::OnLastDay) => {
                NextResulterByDay::new(&next).last_day().year(*year).build()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::OnDays(days)) => {
                let years = BTreeSet::from([*year]);
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(&years)
                    .with_days(days)
                    .next()
            }
            (Cycle::In(year), Cycle::NA, DayCycle::OnWeekDays(weekdays)) => {
                let year = *year as i32;
                let mut next = if next.year() != year {
                    NaiveDateTime::new(NaiveDate::from_ymd_opt(year, 1, 1).unwrap(), next.time())
                } else {
                    next + Duration::days(1)
                };
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    if next.year() > year {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::NA) => NextResulterByDay::new(&next)
                .month(*month)
                .year(*year)
                .build(),
            (Cycle::In(year), Cycle::In(month), DayCycle::On(day, opt)) => {
                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::OnLastDay) => {
                NextResulterByDay::new(&next)
                    .last_day()
                    .month(*month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::OnDays(days)) => {
                let years = BTreeSet::from([*year]);
                let months = BTreeSet::from([*month]);
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(&years)
                    .with_months(&months)
                    .with_days(days)
                    .next()
            }
            (Cycle::In(year), Cycle::In(month), DayCycle::OnWeekDays(weekdays)) => {
                let year = *year as i32;
                let month = *month as u32;
                let mut next = if next.year() != year {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(year, month, 1).unwrap(),
                        next.time(),
                    )
                } else if month > next.month() {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(year, month, 1).unwrap(),
                        next.time(),
                    )
                } else if month < next.month() {
                    return Ok(None);
                } else {
                    next + Duration::days(1)
                };
                if next.year() != year || next.month() != month {
                    return Ok(None);
                }
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    if next.year() > year || next.month() > month {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::NA) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::On(day, opt)) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (
                Cycle::In(year),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .year(*year)
                    .num_months(*num_months)
                    .build()
            }
            (Cycle::In(year), Cycle::Every(num_months), DayCycle::OnLastDay) => {
                let (_, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .last_day()
                    .month(month)
                    .year(*year)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::NA) => NextResulterByDay::new(&next)
                .year(next.year() as u32 + *num_years)
                .build(),
            (Cycle::Every(num_years), Cycle::NA, DayCycle::On(day, opt)) => {
                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::NA,
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::NA,
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::NA,
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .num_years(*num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::NA, DayCycle::OnLastDay) => {
                NextResulterByDay::new(&next)
                    .last_day()
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::NA) => {
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::On(day, opt)) => {
                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::In(month),
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                NextResulterByDay::new(&next)
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .num_years(*num_years)
                    .month(*month)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::In(month), DayCycle::OnLastDay) => {
                NextResulterByDay::new(&next)
                    .last_day()
                    .month(*month)
                    .year(next.year() as u32 + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::NA) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::On(day, opt)) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::Regular),
            ) => {
                let next = next + Duration::days(*num_days as i64);
                let (year, month) = ffwd_months(&next, *num_months);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::BizDay),
            ) => {
                let next = self.bd_processor.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (
                Cycle::Every(num_years),
                Cycle::Every(num_months),
                DayCycle::Every(num_days, EveryDayOption::WeekDay),
            ) => {
                let next = WEEKEND_SKIPPER.add(&next, *num_days)?;
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next)
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::OnWeekDay(wd, opt)) => {
                NextResulterByWeekDay::new(&next, wd, opt)
                    .num_years(*num_years)
                    .num_months(*num_months)
                    .build()
            }
            (Cycle::Every(num_years), Cycle::Every(num_months), DayCycle::OnLastDay) => {
                let (year, month) = ffwd_months(&next, *num_months as u32);
                NextResulterByDay::new(&next)
                    .last_day()
                    .month(month)
                    .year(year + *num_years)
                    .build()
            }
            (Cycle::Every(_), _, DayCycle::OnDays(_)) => {
                Result::Err(Error::Custom("invalid spec"))?
            }
            (Cycle::Every(_), _, DayCycle::OnWeekDays(_)) => {
                Result::Err(Error::Custom("invalid spec"))?
            }
            (Cycle::Every(_), Cycle::Values(_), _) => Result::Err(Error::Custom("invalid spec"))?,
            (_, Cycle::Every(_), DayCycle::OnDays(_)) => {
                Result::Err(Error::Custom("invalid spec"))?
            }
            (_, Cycle::Every(_), DayCycle::OnWeekDays(_)) => {
                Result::Err(Error::Custom("invalid spec"))?
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::NA) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(months)
                    .next()
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::Every(num_days, opt)) => {
                let mut next = next + Duration::days(*num_days as i64);
                while !months.contains(&next.month()) {
                    next = next + Duration::days(*num_days as i64);
                }
                Some(NextResult::Single(next))
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::OnDays(days)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_days(days)
                    .with_months(months)
                    .next()
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::On(day, _)) => {
                let days = BTreeSet::from([*day]);
                NextResulterByMultiplesAndDay::new(&next)
                    .with_days(&days)
                    .with_months(months)
                    .next()
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::OnWeekDay(wd, opt)) => todo!(),
            (Cycle::NA, Cycle::Values(months), DayCycle::OnWeekDays(weekdays)) => {
                let mut next = next + Duration::days(1);
                if !months.contains(&next.month()) {
                    let mut cursor = months.lower_bound(Bound::Excluded(&next.month()));
                    match cursor.next() {
                        Some(month) => {
                            next = NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(next.year(), *month, 1).unwrap(),
                                next.time(),
                            );
                        }
                        None => {
                            let next_year = next.year() + 1;
                            next = NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(next_year, *months.first().unwrap(), 1)
                                    .unwrap(),
                                next.time(),
                            );
                        }
                    }
                }
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    if !months.contains(&next.month()) {
                        let mut cursor = months.lower_bound(Bound::Excluded(&next.month()));
                        match cursor.next() {
                            Some(month) => {
                                next = NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(next.year(), *month, 1).unwrap(),
                                    next.time(),
                                );
                            }
                            None => {
                                let next_year = next.year() + 1;
                                next = NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(next_year, *months.first().unwrap(), 1)
                                        .unwrap(),
                                    next.time(),
                                );
                            }
                        }
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::NA, Cycle::Values(months), DayCycle::OnLastDay) => todo!(),
            (Cycle::In(year), Cycle::Values(months), DayCycle::NA) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(&BTreeSet::from([*year]))
                    .with_months(months)
                    .next()
            }
            (Cycle::In(year), Cycle::Values(months), DayCycle::Every(num_days, opt)) => {
                let year = *year;
                let mut interim = next + Duration::days(*num_days as i64);
                if next.year() as u32 > year {
                    return Ok(None);
                }
                while !(months.contains(&interim.month()) && interim.year() as u32 == year) {
                    interim = interim + Duration::days(*num_days as i64);
                    if interim.year() as u32 > year {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(interim))
            }
            (Cycle::In(year), Cycle::Values(months), DayCycle::OnDays(days)) => {
                let years = BTreeSet::from([*year]);
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(&years)
                    .with_days(days)
                    .with_months(months)
                    .next()
            }
            (Cycle::In(year), Cycle::Values(months), DayCycle::On(day, opt)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(&BTreeSet::from([*year]))
                    .with_days(&BTreeSet::from([*day]))
                    .with_months(months)
                    .next()
            }
            (Cycle::In(year), Cycle::Values(months), DayCycle::OnWeekDay(wd, opt)) => todo!(),
            (Cycle::In(year), Cycle::Values(months), DayCycle::OnWeekDays(weekdays)) => {
                let year = *year as i32;
                let mut next = if next.year() != year {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(year, *months.first().unwrap(), 1).unwrap(),
                        next.time(),
                    )
                } else {
                    let interim = next + Duration::days(1);
                    if !months.contains(&interim.month()) {
                        let mut cursor = months.lower_bound(Bound::Excluded(&interim.month()));
                        match cursor.next() {
                            Some(month) => NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(interim.year(), *month, 1).unwrap(),
                                interim.time(),
                            ),
                            None => {
                                let next_year = next.year() + 1;
                                NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(next_year, *months.first().unwrap(), 1)
                                        .unwrap(),
                                    interim.time(),
                                )
                            }
                        }
                    } else {
                        interim
                    }
                };
                if next.year() != year {
                    return Ok(None);
                }
                while !(months.contains(&next.month())
                    && weekdays.contains(&WeekdayStartingMonday(next.weekday())))
                {
                    next = next + Duration::days(1);
                    if !months.contains(&next.month()) {
                        let mut cursor = months.lower_bound(Bound::Excluded(&next.month()));
                        match cursor.next() {
                            Some(month) => {
                                next = NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(next.year(), *month, 1).unwrap(),
                                    next.time(),
                                );
                            }
                            None => {
                                let next_year = next.year() + 1;
                                next = NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(next_year, *months.first().unwrap(), 1)
                                        .unwrap(),
                                    next.time(),
                                );
                            }
                        }
                    }
                    if next.year() > year {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::In(year), Cycle::Values(months), DayCycle::OnLastDay) => todo!(),
            (Cycle::Values(years), Cycle::NA, DayCycle::NA) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::Every(num_days, opt)) => {
                let last_year = years.last().unwrap();
                let mut next = next + Duration::days(*num_days as i64);

                if next.year() as u32 > *last_year {
                    return Ok(None);
                }
                while !years.contains(&(next.year() as u32)) {
                    next = next + Duration::days(*num_days as i64);
                    if next.year() as u32 > *last_year {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::OnDays(days)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(years)
                    .with_days(days)
                    .next()
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::On(day, opt)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_years(years)
                    .with_days(&BTreeSet::from([*day]))
                    .next()
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::OnWeekDay(wd, opt)) => {
                dbg!(years);
                dbg!(&next);
                dbg!(wd);
                if years.contains(&(next.year() as u32)) {
                    let next_result = NextResulterByWeekDay::new(&next, wd, opt)
                        .year(next.year() as u32)
                        .build();
                    let Some(next_result) = next_result else {
                        return Ok(None);
                    };
                    let next_result_year = next_result.actual().year() as u32;
                    if !years.contains(&next_result_year) {
                        let mut cursor = years.lower_bound(Bound::Excluded(&next_result_year));
                        let Some(next_year) = cursor.next() else {
                            return Ok(None);
                        };
                        let next = NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(*next_year as i32, 1, 1).unwrap(),
                            next.time(),
                        );
                        NextResulterByWeekDay::new(&next, wd, opt)
                            .year(*next_year)
                            .build()
                    } else if next_result.actual() > &next {
                        Some(next_result)
                    } else {
                        let next = NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(
                                next_result_year as i32,
                                next_result.actual().month() + 1,
                                1,
                            )
                            .unwrap_or(
                                NaiveDate::from_ymd_opt(next_result_year as i32 + 1, 1, 1).unwrap(),
                            ),
                            next.time(),
                        );
                        if next.year() as u32 == next_result_year {
                            NextResulterByWeekDay::new(&next, wd, opt)
                                .year(next_result_year)
                                .build()
                        } else {
                            let mut cursor = years.lower_bound(Bound::Excluded(&next_result_year));
                            let Some(next_year) = cursor.next() else {
                                return Ok(None);
                            };
                            let next = NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(*next_year as i32, 1, 1).unwrap(),
                                next.time(),
                            );
                            NextResulterByWeekDay::new(&next, wd, opt)
                                .year(*next_year)
                                .build()
                        }
                    }
                } else {
                    let mut cursor = years.lower_bound(Bound::Excluded(&(next.year() as u32)));
                    let Some(next_year) = cursor.next() else {
                        return Ok(None);
                    };
                    let next = NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(*next_year as i32, 1, 1).unwrap(),
                        next.time(),
                    );
                    dbg!(&next);
                    NextResulterByWeekDay::new(&next, wd, opt)
                        .year(*next_year)
                        .build()
                }
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::OnWeekDays(weekdays)) => {
                let mut next = next + Duration::days(1);
                if !years.contains(&(next.year() as u32)) {
                    let mut cursor = years.lower_bound(Bound::Excluded(&(next.year() as u32)));
                    match cursor.next() {
                        Some(year) => {
                            next = NaiveDateTime::new(
                                NaiveDate::from_ymd_opt(*year as i32, 1, 1).unwrap(),
                                next.time(),
                            )
                        }
                        None => {
                            return Ok(None);
                        }
                    }
                };
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    if !years.contains(&(next.year() as u32)) {
                        let mut cursor = years.lower_bound(Bound::Excluded(&(next.year() as u32)));
                        match cursor.next() {
                            Some(year) => {
                                next = NaiveDateTime::new(
                                    NaiveDate::from_ymd_opt(*year as i32, 1, 1).unwrap(),
                                    next.time(),
                                )
                            }
                            None => {
                                return Ok(None);
                            }
                        }
                    }
                }
                Some(NextResult::Single(next))
            }
            (Cycle::Values(years), Cycle::NA, DayCycle::OnLastDay) => todo!(),
            (Cycle::Values(years), Cycle::In(month), DayCycle::NA) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(&BTreeSet::from([*month]))
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::In(month), DayCycle::Every(num_days, opt)) => {
                let max_year = years.last().unwrap();
                let next = next + Duration::days(*num_days as i64);
                let next_result = NextResulterByDay::new(&next).month(*month).build();
                let Some(mut next_result) = next_result else {
                    return Ok(None);
                };
                if next_result.actual().year() as u32 > *max_year {
                    return Ok(None);
                }
                while !years.contains(&(next_result.actual().year() as u32)) {
                    let next = next_result.actual().clone() + Duration::days(*num_days as i64);
                    let Some(interim_result) = NextResulterByDay::new(&next).month(*month).build()
                    else {
                        return Ok(None);
                    };
                    if interim_result.actual().year() as u32 > *max_year {
                        return Ok(None);
                    }
                    next_result = interim_result;
                }
                Some(next_result)
            }
            (Cycle::Values(years), Cycle::In(month), DayCycle::OnDays(days)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(&BTreeSet::from([*month]))
                    .with_days(days)
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::In(month), DayCycle::On(day, opt)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(&BTreeSet::from([*month]))
                    .with_days(&BTreeSet::from([*day]))
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::In(month), DayCycle::OnWeekDay(wd, opt)) => todo!(),
            (Cycle::Values(years), Cycle::In(month), DayCycle::OnWeekDays(weekdays)) => todo!(),
            (Cycle::Values(years), Cycle::In(month), DayCycle::OnLastDay) => todo!(),
            (Cycle::Values(years), Cycle::Values(months), DayCycle::NA) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(months)
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::Values(months), DayCycle::Every(num_days, opt)) => {
                let max_year = *years.last().unwrap() as i32;
                let mut interim = next + Duration::days(*num_days as i64);
                if interim.year() > max_year {
                    return Ok(None);
                }

                while !(months.contains(&interim.month())
                    && years.contains(&(interim.year() as u32)))
                {
                    interim = interim + Duration::days(*num_days as i64);
                    // dbg!(&interim, years, months);
                    if interim.year() > max_year {
                        return Ok(None);
                    }
                }
                Some(NextResult::Single(interim))
                // validate!()
            }
            (Cycle::Values(years), Cycle::Values(months), DayCycle::OnDays(days)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(months)
                    .with_days(days)
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::Values(months), DayCycle::On(day, opt)) => {
                NextResulterByMultiplesAndDay::new(&next)
                    .with_months(months)
                    .with_days(&BTreeSet::from([*day]))
                    .with_years(years)
                    .next()
            }
            (Cycle::Values(years), Cycle::Values(months), DayCycle::OnWeekDay(wd, opt)) => todo!(),
            (Cycle::Values(years), Cycle::Values(months), DayCycle::OnWeekDays(weekdays)) => {
                let year_computer = |year: &u32| -> Option<&u32> {
                    years.get(&year).or_else(|| {
                        let mut cursor = years.lower_bound(Bound::Excluded(&year));
                        let Some(year) = cursor.next() else {
                            return None;
                        };
                        Some(year)
                    })
                };

                let year_month_computer = |year: u32, month: u32| -> Option<(u32, u32)> {
                    months.get(&month).map_or_else(
                        || {
                            let mut cursor = months.lower_bound(Bound::Excluded(&month));
                            let Some(month) = cursor.next() else {
                                let mut year_cursor = years.lower_bound(Bound::Excluded(&year));
                                let Some(year) = year_cursor.next() else {
                                    return None;
                                };
                                return Some((*year, *months.first().unwrap()));
                            };
                            let Some(next_year) = year_computer(&year) else {
                                return None;
                            };
                            if *next_year > year {
                                let first_month = months.first().unwrap();
                                return Some((*next_year, *first_month));
                            }
                            Some((*next_year, *month))
                        },
                        |month| {
                            let Some(next_year) = year_computer(&year) else {
                                return None;
                            };
                            if *next_year > year {
                                let first_month = months.first().unwrap();
                                return Some((*next_year, *first_month));
                            }
                            Some((*next_year, *month))
                        },
                    )
                };

                let month = next.month();
                let year = next.year() as u32;

                let nxt_year_month = year_month_computer(year, month);

                let Some((nxt_year, nxt_month)) = nxt_year_month else {
                    return Ok(None);
                };

                let mut next = if nxt_year > year || nxt_month > month {
                    NaiveDateTime::new(
                        NaiveDate::from_ymd_opt(nxt_year as i32, nxt_month, 1).unwrap(),
                        next.time(),
                    )
                } else {
                    next
                };

                next = next + Duration::days(1);
                while !weekdays.contains(&WeekdayStartingMonday(next.weekday())) {
                    next = next + Duration::days(1);
                    let year = next.year() as u32;
                    let month = next.month();
                    let nxt_year_month = year_month_computer(year, month);
                    let Some((nxt_year, nxt_month)) = nxt_year_month else {
                        return Ok(None);
                    };
                    if nxt_year > year || nxt_month > month {
                        next = NaiveDateTime::new(
                            NaiveDate::from_ymd_opt(nxt_year as i32, nxt_month, 1).unwrap(),
                            next.time(),
                        );
                    };
                }
                if year_month_computer(next.year() as u32, next.month()).is_none() {
                    return Ok(None);
                }
                Some(NextResult::Single(next))
            }
            (Cycle::Values(years), Cycle::Values(months), DayCycle::OnLastDay) => {
                // NextResulterByMultiplesAndDay::new(&next)
                //     .with_months(months)
                //     .with_years(years)
                //     .with_days(&BTreeSet::from([31]))
                //     .next()
                todo!()
            }
            (Cycle::Values(years), Cycle::Every(num_months), DayCycle::NA) => {
                let last_year = years.last().unwrap();
                let (mut year, mut month) = ffwd_months(&next, *num_months);
                if year > *last_year {
                    return Ok(None);
                }
                let next_result = NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build();

                let Some(mut next_result) = next_result else {
                    return Ok(None);
                };

                while !years.contains(&year) {
                    (year, month) = ffwd_months(&next, *num_months);
                    if year > *last_year {
                        return Ok(None);
                    }
                    let Some(interim_result) = NextResulterByDay::new(next_result.actual())
                        .month(month)
                        .year(year)
                        .build()
                    else {
                        return Ok(None);
                    };
                    next_result = interim_result;
                }
                Some(next_result)
            }
            (Cycle::Values(years), Cycle::Every(num_months), DayCycle::Every(num_days, opt)) => {
                let last_year = years.last().unwrap();
                let next = next + Duration::days(*num_days as i64);
                let (mut year, mut month) = ffwd_months(&next, *num_months);

                if year > *last_year {
                    return Ok(None);
                }

                let Some(mut next_result) = NextResulterByDay::new(&next)
                    .month(month)
                    .year(year)
                    .build()
                else {
                    return Ok(None);
                };
                while !years.contains(&year) {
                    (year, month) = ffwd_months(&next_result.actual(), *num_months);
                    if year > *last_year {
                        return Ok(None);
                    }
                    let Some(interim_result) = NextResulterByDay::new(next_result.actual())
                        .month(month)
                        .year(year)
                        .build()
                    else {
                        return Ok(None);
                    };
                    next_result = interim_result;
                }
                Some(next_result)
            }
            (Cycle::Values(years), Cycle::Every(num_months), DayCycle::On(day, opt)) => {
                let last_year = years.last().unwrap();
                let (mut year, mut month) = ffwd_months(&next, *num_months);

                if year > *last_year {
                    return Ok(None);
                }

                let Some(mut next_result) = NextResulterByDay::new(&next)
                    .last_day_option(opt)
                    .day(*day)
                    .month(month)
                    .year(year)
                    .build()
                else {
                    return Ok(None);
                };

                while !years.contains(&year) {
                    (year, month) = ffwd_months(&next_result.actual(), *num_months);
                    if year > *last_year {
                        return Ok(None);
                    }
                    let Some(interim_result) = NextResulterByDay::new(next_result.actual())
                        .last_day_option(opt)
                        .day(*day)
                        .month(month)
                        .year(year)
                        .build()
                    else {
                        return Ok(None);
                    };
                    next_result = interim_result;
                }
                Some(next_result)
            }
            (Cycle::Values(years), Cycle::Every(num_months), DayCycle::OnWeekDay(wd, opt)) => {
                todo!()
            }
            (Cycle::Values(years), Cycle::Every(num_months), DayCycle::OnLastDay) => {
                let last_year = years.last().unwrap();
                let (mut year, mut month) = ffwd_months(&next, *num_months);
                if year > *last_year {
                    return Ok(None);
                }

                let next_result = NextResulterByDay::new(&next)
                    .last_day()
                    .month(month)
                    .year(year)
                    .build();

                let Some(mut next_result) = next_result else {
                    return Ok(None);
                };

                while !years.contains(&year) {
                    (year, month) = ffwd_months(&next_result.actual(), *num_months);
                    if year > *last_year {
                        return Ok(None);
                    }
                    let Some(interim_result) = NextResulterByDay::new(next_result.actual())
                        .last_day()
                        .month(month)
                        .year(year)
                        .build()
                    else {
                        return Ok(None);
                    };
                    next_result = interim_result;
                }
                Some(next_result)
            }
        };

        let Some(next_result) = next_result else {
            return Ok(None);
        };

        if next_result.actual() <= &self.dtm {
            return Ok(None);
        }

        let next_result = if let Some(biz_day_adj) = &self.spec.biz_day_adj {
            let (actual, observed) = next_result.as_tuple();
            if self.bd_processor.is_biz_day(&observed)? {
                next_result
            } else {
                match biz_day_adj {
                    BizDayAdjustment::Weekday(dir) => {
                        let adjusted = WEEKEND_SKIPPER.find_biz_day(observed, dir.clone())?;
                        adjusted_to_next_result(*actual, adjusted)
                    }
                    BizDayAdjustment::BizDay(dir) => {
                        let adjusted = self.bd_processor.find_biz_day(observed, dir.clone())?;
                        adjusted_to_next_result(*actual, adjusted)
                    }
                    BizDayAdjustment::Prev(num) => NextResult::AdjustedEarlier(
                        actual.clone(),
                        self.bd_processor.sub(observed, *num)?,
                    ),
                    BizDayAdjustment::Next(num) => NextResult::AdjustedLater(
                        actual.clone(),
                        self.bd_processor.add(observed, *num)?,
                    ),
                    BizDayAdjustment::NA => next_result,
                }
            }
        } else {
            next_result
        };

        if next_result.actual() <= &self.dtm {
            return Ok(None);
        }

        if let Some(end) = &self.end {
            if next_result.actual() > &end {
                self.dtm = end.clone();
                self.index += 1;
                return Ok(Some(NextResult::Single(end.clone())));
            }
        };

        self.index += 1;
        self.dtm = next_result.actual().clone();
        Ok(Some(next_result))
    }
}

fn ffwd_months(dtm: &NaiveDateTime, num: u32) -> (u32, u32) {
    let mut new_month = dtm.month() + num;
    let mut new_year = dtm.year() as u32;
    new_year += (new_month - 1) / 12;
    new_month = (new_month - 1) % 12 + 1;
    (new_year, new_month)
}

static WEEKEND_SKIPPER: LazyLock<WeekendSkipper> = LazyLock::new(|| WeekendSkipper::new());

fn adjusted_to_next_result(
    dtm: NaiveDateTime,
    adjusted: NaiveDateTime,
) -> NextResult<NaiveDateTime> {
    if adjusted == dtm {
        NextResult::Single(adjusted)
    } else if adjusted > dtm {
        NextResult::AdjustedLater(dtm, adjusted)
    } else {
        NextResult::AdjustedEarlier(dtm, adjusted)
    }
}

#[cfg(test)]
mod tests {
    use crate::biz_day::WeekendSkipper;

    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_with_start() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 11, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter = SpecIteratorBuilder::new_with_start("YY-1M-DD", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 31, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_with_start("YY-1M-31N", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter_multiples() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 31, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_after("YY-[02]-[01,02,03]", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter_multiples_every_day() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2024, 12, 1, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_after("[2025,2026]-MM-[MON,TUE]", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(24)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap());
    }
}
