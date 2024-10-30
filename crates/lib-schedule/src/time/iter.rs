use chrono::{DateTime, Duration, TimeZone, Timelike, Utc};

use derivative::Derivative;
use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::{prelude::*, NextTime};

#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone> {
    spec: Spec,
    start: DateTime<Tz>,
    end: Option<DateTime<Tz>>,
    end_spec: Option<String>,
    remaining: Option<u32>,
    dtm: DateTime<Tz>,
}

impl <Tz: TimeZone>SpecIterator<Tz> {
    fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            start: start.clone(),
            dtm: start,
            end: None,
            end_spec: None,
            remaining: None,
        })
    }

    fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            start: start.clone(),
            end: Some(end),
            dtm: start,
            end_spec: None,
            remaining: None,
        })
    }

    fn new_with_end_spec(spec: &str, start: DateTime<Tz>, end_spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        let end = Self::new(end_spec, start.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            spec,
            start: start.clone(),
            end: Some(end),
            dtm: start,
            end_spec: Some(end_spec.into()),
            remaining: None,
        })
    }

    fn new_with_max(spec: &str, start: DateTime<Tz>, max: u32) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            remaining: Some(max),
            spec,
            dtm: start,
            end: None,
            end_spec: None,
        })
    }
}

impl <Tz: TimeZone>NextTime<Tz> for SpecIterator<Tz> {}

impl <Tz: TimeZone>FallibleIterator for SpecIterator<Tz> {
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
        let mut next = self.dtm.clone();
        
        match &self.spec.hours {
            Cycle::At(h) => {
                next = next.with_hour(*h as u32).unwrap();
            }
            Cycle::Every(h) => {
                next = next + Duration::hours(*h as i64);
            }
            _ => {}
        };
        match &self.spec.minutes {
            Cycle::At(m) => {
                next = next.with_minute(*m as u32).unwrap();
            }
            Cycle::Every(m) => {
                next = next + Duration::minutes(*m as i64);
            }
            _ => {}
        };

        match &self.spec.seconds {
            Cycle::At(s) => {
                next = next.with_second(*s as u32).unwrap();
            }
            Cycle::Every(s) => {
                next = next + Duration::seconds(*s as i64);
            }
            _ => {}
        };

        if next <= self.dtm {
            return Ok(None);
        }
        self.dtm = next;
        self.remaining = remaining;
        Ok(Some(self.dtm.clone()))
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
        let spec_ter = SpecIterator::new("3H:MM:00", dtm).unwrap();
        dbg!(spec_ter.take(5).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_max() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2024, 11, 3, 0, 30, 0).unwrap();
        dbg!(&dtm);
        let spec_iter = SpecIterator::new_with_max("2H:00:00", dtm, 5).unwrap();

        let tmp = spec_iter.collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(tmp);
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
