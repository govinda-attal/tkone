use core::time;
use core::marker::PhantomData;
use std::str::FromStr;

use chrono::{DateTime, TimeZone};
use fallible_iterator::FallibleIterator;
use once_cell::sync::Lazy;
use regex::Regex;
use crate::biz_day::{BizDayProcessor, WeekendSkipper};
use crate::{date, prelude::*};

use crate::date::Spec as DateSpec;
use crate::time::Spec as TimeSpec;
use crate::date::SPEC_EXPR as DATE_SPEC_EXPR;
use crate::time::SPEC_EXPR as TIME_SPEC_EXPR;
use crate::date::{NaiveSpecIterator as DateNaiveSpecIterator, SpecIterator as DateSpecIterator};
use crate::time::{NaiveSpecIterator as TimeNaiveSpecIterator, SpecIterator as TimeSpecIterator};

#[derive(Debug, Clone)]
struct Spec  {
    date_spec: String,
    time_spec: String,
}


#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    start: DateTime<Tz>,
    end: Option<DateTime<Tz>>,
    remaining: Option<u32>,
    dtm: DateTime<Tz>,
    spec: Spec,
    bd_processor: BDP,
}

#[derive(Clone)]
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
#[derive(Clone)]
pub struct EndSpec(String);

#[derive(Clone)]
pub struct NoEnd;

#[derive(Clone)]
pub struct Sealed;

#[derive(Clone)]
pub struct NotSealed;

#[derive(Clone)]
pub struct SpecIteratorBuilder<Tz: TimeZone, E, BDP: BizDayProcessor, S> {
    start: DateTime<Tz>, 
    spec: String,
    bd_processor: BDP,
    end: E,
    marker_sealed: PhantomData<S>,
}


impl <Tz: TimeZone, E, BDP: BizDayProcessor>SpecIteratorBuilder<Tz, E, BDP, NotSealed> {
    pub fn with_end_spec(self, end_spec: impl Into<String>) -> SpecIteratorBuilder<Tz, EndSpec, BDP, Sealed> {
        SpecIteratorBuilder {
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndSpec(end_spec.into()),
            marker_sealed: PhantomData,
        }
    }

    pub fn with_end(self, end: DateTime<Tz>) -> SpecIteratorBuilder<Tz, EndDateTime<Tz>, BDP, Sealed> {
        SpecIteratorBuilder::<Tz, EndDateTime<Tz>, BDP, Sealed> {
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            marker_sealed: PhantomData,
        }
    }
}


impl <Tz: TimeZone, BDP: BizDayProcessor>SpecIteratorBuilder<Tz, EndDateTime<Tz>, BDP, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::new_with_end(&self.spec, self.start, self.bd_processor, self.end.0)?)
    }
}

impl <Tz: TimeZone, BDP: BizDayProcessor>SpecIteratorBuilder<Tz, EndSpec, BDP, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::new_with_end_spec(&self.spec, self.start, self.bd_processor, &self.end.0)?)
    }
}



impl <Tz: TimeZone, BDP: BizDayProcessor, S>SpecIteratorBuilder<Tz, NoEnd, BDP, S> {
    pub fn new(spec: &str, start: DateTime<Tz>, bdp: BDP) -> SpecIteratorBuilder<Tz, NoEnd, BDP, NotSealed> {
        SpecIteratorBuilder {
            start: start,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::new(&self.spec, self.start, self.bd_processor)?)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIterator<Tz, BDP> {
    fn new(spec: &str, start: DateTime<Tz>, bd_processor: BDP) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        Ok(Self {
            spec,
            start: start.clone(),
            end: None,
            remaining: None,
            bd_processor,
            dtm: start,
        })
    }

    fn new_with_end(spec: &str, start: DateTime<Tz>, bd_processor: BDP, end: DateTime<Tz>) -> Result<Self> {
        if end <= start {
            return Err(Error::Custom("end date-time must be after the start date-time"));
        }
        let spec = Spec::from_str(spec)?;
        Ok(Self {
            spec,
            start: start.clone(),
            end: Some(end),
            remaining: None,
            bd_processor,
            dtm: start,
        })
    }

    fn new_with_end_spec(spec: &str, start: DateTime<Tz>, bd_processor: BDP, end_spec: &str) -> Result<Self> {
        let spec = Spec::from_str(spec)?;
        let end_spec = Spec::from_str(end_spec)?;
        let mut time_spec_iter = TimeSpecIterator::new(&end_spec.time_spec, start.clone())?;
        let end = match time_spec_iter.next()? {
            Some(dtm) => {
                let mut date_spec_iter = DateSpecIterator::new(&end_spec.date_spec, dtm)?;
                date_spec_iter.next()?
            },
            None => None,
        };
        let Some(end) = end else{
            return Err(Error::Custom("End spec must result in a date-time after the start date-time"));
        };
        if end <= start {
            return Err(Error::Custom("End spec must result in a date-time after the start date-time"));
        }
        Ok(Self {
            spec,
            start: start.clone(),
            end: Some(end),
            remaining: None,
            bd_processor,
            dtm: start,
        })
    }
}


impl <Tz: TimeZone, BDP: BizDayProcessor>FallibleIterator for SpecIterator<Tz, BDP> {
    type Item = DateTime<Tz>;
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

        let mut time_spec_iter = TimeSpecIterator::new(&self.spec.time_spec, self.dtm.clone())?;
        let next = match time_spec_iter.next()? {
            None => self.dtm.clone(),
            Some(next) => {
                next
            },
        };

        let next = DateSpecIterator::new(&self.spec.date_spec, next)?.next()?;

        let Some(next) = next else{
            return Ok(None);
        };

        if next <= self.dtm {
            return Ok(None);
        }

        self.dtm = next;
        self.remaining = remaining;

        Ok(Some(self.dtm.clone()))
    }
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = &SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid spec"))?;
        let Some(date_spec) = caps.name("date") else {
            return Err(Error::ParseError("missing date spec"));
        };
        let Some(time_spec) = caps.name("time") else {
            return Err(Error::ParseError("missing time spec"));
        };
        
        Ok(Self {
            date_spec: date_spec.as_str().to_string(),
            time_spec: time_spec.as_str().to_string(),
        })
    }
}

pub static SPEC_EXPR: Lazy<String> = Lazy::new(||format!("(?:(?<date>{DATE_SPEC_EXPR})?T(?<time>{TIME_SPEC_EXPR}))").to_string());

pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(&SPEC_EXPR).unwrap());

#[cfg(test)] 
mod tests{
    use super::*;
    use chrono::Utc;
    use crate::biz_day::WeekendSkipper;
    #[test]
    fn test_spec_iter() {
        let tmp = SpecIteratorBuilder::<Utc, NoEnd, WeekendSkipper, NotSealed>::new("YY:1M:28:PT12:00:00", Utc::now(), WeekendSkipper{}).build().unwrap();
        let tmp= tmp.take(5).collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(&tmp);
    }

    #[test]
    fn test_one() {
        let spec = SPEC_RE.captures("YY:1M:DD:PT12:00:00").unwrap();
        dbg!(&spec);
    }

}