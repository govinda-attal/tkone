use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Timelike};

use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::prelude::*;

#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator,
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    pub fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(spec, start.naive_local())?,
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
        })
    }
}

#[derive(Debug, Clone)]
pub struct NaiveSpecIterator {
    spec: Spec,
    end: Option<NaiveDateTime>,
    remaining: Option<u32>,
    dtm: NaiveDateTime,
}

impl NaiveSpecIterator {
    pub fn new(spec: &str, start: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start,
            spec,
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

        let next = match &self.spec.seconds {
            Cycle::At(s) => next.with_second(*s as u32).unwrap(),
            Cycle::Every(s) => next + Duration::seconds(*s as i64),
            _ => next,
        };

        let next = match &self.spec.minutes {
            Cycle::At(m) => next.with_minute(*m as u32).unwrap(),
            Cycle::Every(m) => next + Duration::minutes(*m as i64),
            _ => next,
        };

        let next = match &self.spec.hours {
            Cycle::At(h) => next.with_hour(*h as u32).unwrap(),
            Cycle::Every(h) => next + Duration::hours(*h as i64),
            _ => next,
        };

        self.dtm = next;
        self.remaining = remaining;

        Ok(Some(self.dtm.clone()))
    }
}

impl<Tz: TimeZone> FallibleIterator for SpecIterator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let item = self.naive_spec_iter.next()?;
        let Some(next) = item else {
            return Ok(None);
        };
        Ok(Some(Self::Item::from(W((self.tz.clone(), next.clone())))))
    }
}

#[cfg(test)]
mod tests {

    use chrono_tz::{America::New_York, Europe::London};

    use super::*;

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm.to_rfc3339());
        let dt = DateTime::parse_from_rfc3339("2023-03-11T23:00:00-05:00").unwrap();
        let spec_ter = SpecIterator::new("HH:30M:00", dt.with_timezone(&New_York)).unwrap();
        dbg!(spec_ter.take(6).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_month_end() {
        // US Eastern Time (EST/EDT)
        let london = London;
        // Before DST starts (Standard Time)
        let dtm = london.with_ymd_and_hms(2021, 10, 31, 00, 30, 0).unwrap();
        dbg!(&dtm);
        // let dt = DateTime::parse_from_rfc3339("2023-03-11T23:00:00-05:00").unwrap();
        let spec_ter = SpecIterator::new("1H:MM:00", dtm).unwrap();
        dbg!(spec_ter.take(5).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_end_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);

        let spec_iter = SpecIterator::new_with_end_spec("3H:00:00", dtm, "15H:00:00").unwrap();

        let tmp = spec_iter.collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(tmp);
    }
}
