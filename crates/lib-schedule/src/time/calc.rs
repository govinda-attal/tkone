use chrono::{DateTime, Duration, TimeZone, Timelike};
use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::prelude::*;

struct Calculator<Tz: TimeZone> {
    spec: Spec,
    dtm: DateTime<Tz>,
}

impl<Tz: TimeZone> Calculator<Tz> {
    fn new(dtm: DateTime<Tz>, spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
        })
    }
}

impl<Tz: TimeZone> FallibleIterator for Calculator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {

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

        Ok(Some(next))
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_spec_from_str() {
        let dtm = DateTime::parse_from_rfc3339("2021-01-01T12:20:05Z").unwrap();
        let mut calc = Calculator::new(dtm, "12H:30M:5S").unwrap();
        

        dbg!(calc.next());
    }
}