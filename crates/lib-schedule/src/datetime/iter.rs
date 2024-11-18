use super::spec::Spec;
use crate::biz_day::BizDayProcessor;
use crate::{prelude::*, NextResult};
use core::marker::PhantomData;
use std::str::FromStr;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use fallible_iterator::FallibleIterator;

use crate::date::{
    SpecIterator as DateSpecIterator, SpecIteratorBuilder as DateSpecIteratorBuilder,
};
use crate::time::SpecIterator as TimeSpecIterator;

/// # SpecIterator
/// datetime::SpecIterator is an iterator that combines a date and time specification to generate a sequence of date-times.
/// This iterator is created using the SpecIteratorBuilder.
///
/// ## Example
/// ```rust
/// use lib_schedule::biz_day::WeekendSkipper;
/// use lib_schedule::datetime::SpecIteratorBuilder;
/// use chrono_tz::America::New_York;
/// use fallible_iterator::FallibleIterator;
/// use chrono::{offset::TimeZone, DateTime};
/// use lib_schedule::NextResult;
///
/// let start = New_York.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap();
/// let end = New_York.with_ymd_and_hms(2025, 7, 31, 11, 0, 0).unwrap();
/// let iter = SpecIteratorBuilder::new_with_start("YY:1M:08:WT11:00:00", WeekendSkipper::new(), start).with_end(end).build().unwrap();
/// let occurrences = iter.take(3).collect::<Vec<NextResult<DateTime<_>>>>().unwrap();
/// ```
#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    date_iter: DateSpecIterator<Tz, BDP>,
    time_iter: TimeSpecIterator<Tz>,
    dtm: DateTime<Tz>,
    start: Option<DateTime<Tz>>,
    end: Option<DateTime<Tz>>,
    index: usize,
}

pub struct StartDateTime;
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

/// # SpecIteratorBuilder
/// datetime::SpecIteratorBuilder is a builder for SpecIterator.
/// It can be used to build a SpecIterator with a start date-time, end date-time, or end spec.
/// If no start date-time is provided, the current date-time is used.
///
/// ## See Also
/// [`SpecIterator`](crate::datetime::SpecIterator)
pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    bd_processor: BDP,
    end: END,
    timezone: Tz,
    marker_sealed: PhantomData<S>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime, NoEnd, NotSealed>
{
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime, EndSpec, Sealed> {
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

    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime, EndDateTime<Tz>, Sealed> {
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
    SpecIteratorBuilder<Tz, BDP, StartDateTime, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.dtm;
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
        Ok(SpecIterator::new_with_end(
            &self.spec,
            start,
            self.bd_processor,
            self.end.0,
        )?)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime, EndSpec, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.dtm;
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
        Ok(SpecIterator::new_with_end_spec(
            &self.spec,
            start,
            self.bd_processor,
            &self.end.0,
        )?)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime, NoEnd, NotSealed>
{
    pub fn new_with_start(
        spec: &str,
        bdp: BDP,
        start: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            dtm: start.clone(),
            timezone: start.timezone(),
            start: StartDateTime,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
        }
    }
}
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime, NoEnd, NotSealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::new_with_start(
            &self.spec,
            self.bd_processor,
            self.dtm,
        )?)
    }
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

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::new(&self.spec, self.bd_processor, self.dtm)?)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIterator<Tz, BDP> {
    fn new(spec: &str, bd_processor: BDP, dtm: DateTime<Tz>) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        let time_iter = TimeSpecIterator::new(&spec.time_spec, dtm.clone())?;
        let date_iter =
            DateSpecIteratorBuilder::new(&spec.date_spec, bd_processor.clone(), &dtm.timezone())
                .build()?;

        Ok(Self {
            time_iter,
            date_iter,
            dtm,
            start: None,
            end: None,
            index: 0,
        })
    }

    fn new_with_start(spec: &str, bd_processor: BDP, start: DateTime<Tz>) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        let time_iter = TimeSpecIterator::new(&spec.time_spec, start.clone())?;
        let date_iter = DateSpecIteratorBuilder::new_with_start(
            &spec.date_spec,
            bd_processor.clone(),
            start.clone(),
        )
        .build()?;

        Ok(Self {
            time_iter,
            date_iter,
            dtm: start.clone(),
            start: Some(start),
            end: None,
            index: 0,
        })
    }

    fn new_with_end(
        spec: &str,
        start: DateTime<Tz>,
        bd_processor: BDP,
        end: DateTime<Tz>,
    ) -> Result<Self> {
        if end <= start {
            return Err(Error::Custom(
                "end date-time must be after the start date-time",
            ));
        }
        let spec = Spec::from_str(spec)?;
        let time_iter =
            TimeSpecIterator::new_with_end(&spec.time_spec, start.clone(), end.clone())?;
        let date_iter = DateSpecIteratorBuilder::new_with_start(
            &spec.date_spec,
            bd_processor.clone(),
            start.clone(),
        )
        .with_end(end.clone())
        .build()?;

        Ok(Self {
            time_iter,
            date_iter,
            dtm: start.clone(),
            start: Some(start),
            end: Some(end),
            index: 0,
        })
    }

    fn new_with_end_spec(
        spec: &str,
        start: DateTime<Tz>,
        bd_processor: BDP,
        end_spec: &str,
    ) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        let end_spec = Spec::from_str(end_spec)?;
        let mut time_spec_iter = TimeSpecIterator::new(&end_spec.time_spec, start.clone())?;
        let end = match time_spec_iter.next()? {
            Some(dtm) => {
                let mut date_spec_iter = DateSpecIteratorBuilder::new_with_start(
                    &end_spec.date_spec,
                    bd_processor.clone(),
                    dtm,
                )
                .build()?;
                date_spec_iter.next()?
            }
            None => None,
        };
        let Some(end) = end else {
            return Err(Error::Custom(
                "End spec must result in a date-time after the start date-time",
            ));
        };
        if end.actual() <= &start {
            return Err(Error::Custom(
                "End spec must result in a date-time after the start date-time",
            ));
        }
        let time_iter =
            TimeSpecIterator::new_with_end(&spec.time_spec, start.clone(), end.actual().clone())?;
        let date_iter = DateSpecIteratorBuilder::new_with_start(
            &spec.date_spec,
            bd_processor.clone(),
            start.clone(),
        )
        .with_end(end.actual().clone())
        .build()?;

        Ok(Self {
            time_iter,
            date_iter,
            dtm: start.clone(),
            start: Some(start),
            end: Some(end.actual().clone()),
            index: 0,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDP> {
    type Item = NextResult<DateTime<Tz>>;
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

        let time_iter = &mut self.time_iter;
        time_iter.update_cursor(self.dtm.clone());
        let next = match time_iter.next()? {
            None => self.dtm.clone(),
            Some(next) => next,
        };

        if let Some(end) = &self.end {
            if &next >= &end {
                return Ok(None);
            }
        };

        let date_iter = &mut self.date_iter;
        date_iter.update_cursor(next.clone());
        let next = date_iter.next()?;

        let Some(next) = next else {
            return Ok(None);
        };

        if next.actual() < &self.dtm {
            return Ok(None);
        }

        if let Some(end) = &self.end {
            if next.actual() >= &end {
                return Ok(None);
            }
        };

        self.index += 1;
        self.dtm = next.actual().clone();

        Ok(Some(next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::biz_day::WeekendSkipper;
    use chrono::Utc;
    #[test]
    fn test_spec_iter() {
        let tmp = SpecIteratorBuilder::new("YY:1M:31LT11:00:00", WeekendSkipper::new(), &Utc)
            // .with_end(Utc::with_ymd_and_hms(&Utc, 2025, 07, 31, 11, 00, 0).unwrap())
            .build()
            .unwrap();
        let tmp = tmp
            .take(6)
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap();
        dbg!(&tmp);
    }
}
