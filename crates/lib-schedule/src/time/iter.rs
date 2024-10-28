use chrono::{DateTime, Duration, TimeZone, Timelike};
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::prelude::*;

pub struct SpecIterator {
    start: DateTime<Tz>,
    end: Option<DateTime<Tz>>,
    remaining: Option<u32>,
    spec: Spec,
    dtm: DateTime<Tz>,
}

impl SpecIterator {
    fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            end: None,
            remaining: None,
            spec,
            dtm: start,
        })
    }

    fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            end: Some(end),
            remaining: None,
            spec,
            dtm: start,
        })
    }

    fn new_with_max(spec: &str, start: DateTime<Tz>, max: u32) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            start: start.clone(),
            end: None,
            remaining: Some(max),
            spec,
            dtm: start,
        })
    }
}

impl FallibleIterator for SpecIterator {
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
        Ok(Some(next))
    }
}

#[cfg(test)]
mod tests {
    use chrono_tz::America::New_York;
    use fallible_iterator::IntoFallibleIterator;

    use super::*;

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);
        let mut calc = SpecIterator::new("3H:00:00", dtm).unwrap();
        dbg!(calc.take(3).collect::<Vec<DateTime<Tz>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_max() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);
        let mut spec_iter = SpecIterator::new_with_max("3H:00:00", dtm, 2).unwrap();
        
        let tmp = spec_iter.collect::<Vec<DateTime<Tz>>>().unwrap();
        dbg!(tmp);
    }
}
