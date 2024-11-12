use super::spec::Spec;
use crate::biz_day::BizDayProcessor;
use crate::{prelude::*, NextResult};
use core::marker::PhantomData;
use std::str::FromStr;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use fallible_iterator::FallibleIterator;

use crate::date::SpecIteratorBuilder as DateSpecIteratorBuilder;
use crate::time::SpecIterator as TimeSpecIterator;

#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    spec: Spec,
    dtm: DateTime<Tz>,
    bd_processor: BDP,
    start: Option<DateTime<Tz>>,
    end: Option<DateTime<Tz>>,
    remaining: Option<u32>,
    index: usize,
}

pub struct StartDateTime;
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
        Ok(Self {
            spec,
            bd_processor,
            dtm,
            start: None,
            end: None,
            remaining: None,
            index: 0,
        })
    }

    fn new_with_start(spec: &str, bd_processor: BDP, start: DateTime<Tz>) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        Ok(Self {
            spec,
            bd_processor,
            dtm: start.clone(),
            start: Some(start),
            end: None,
            remaining: None,
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
        Ok(Self {
            spec,
            bd_processor,
            dtm: start.clone(),
            start: Some(start),
            end: Some(end),
            remaining: None,
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
        Ok(Self {
            spec,
            bd_processor,
            dtm: start.clone(),
            start: Some(start),
            end: Some(end.actual().clone()),
            remaining: None,
            index: 0,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDP> {
    type Item = NextResult<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        dbg!(&self.remaining);
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

        let mut time_spec_iter = TimeSpecIterator::new(&self.spec.time_spec, self.dtm.clone())?;
        let next = match time_spec_iter.next()? {
            None => self.dtm.clone(),
            Some(next) => next,
        };

        let next = DateSpecIteratorBuilder::new_after(
            &self.spec.date_spec,
            self.bd_processor.clone(),
            next,
        )
        .build()?
        .next()?;

        let Some(next) = next else {
            return Ok(None);
        };

        if next.actual() < &self.dtm {
            return Ok(None);
        }

        self.index += 1;
        self.dtm = next.actual().clone();
        self.remaining = remaining;

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
