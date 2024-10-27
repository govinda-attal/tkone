use chrono::{DateTime, Duration, TimeZone, Timelike};
use core::num;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{collections::HashSet, str::FromStr};

use crate::NextTime;

use crate::error::{Error, Result};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Unit {
    Hour(Cycle),
    Minutes(Cycle),
    Seconds(Cycle),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    At(u8),
    Every(u8),
    NA,
}

#[derive(Default, Debug)]
pub struct TimeSpec {
    pub legs: HashSet<Unit>,
}

impl FromStr for TimeSpec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = &SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;
        let mut cycles = caps
            .iter()
            .skip(1)
            .flatten()
            .map(|m| Cycle::try_from(m.as_str()))
            .collect::<Result<Vec<_>>>()?;

        let legs = HashSet::from_iter(vec![
            Unit::Hour(cycles.remove(0)),
            Unit::Minutes(cycles.remove(0)),
            Unit::Seconds(cycles.remove(0)),
        ]);

        Ok(Self { legs })
    }
}

impl TryFrom<&str> for Cycle {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let cycle = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid time spec"))?;

        let Some(num) = cycle.name("num") else {
            return Ok(Cycle::NA);
        };
        let num = num.as_str().parse::<u8>().unwrap();
        let cycle = if cycle.name("type").is_some() {
            Cycle::Every(num)
        } else {
            Cycle::At(num)
        };
        Ok(cycle)
    }
}

pub const SPEC_EXPR: &str = r"^([01][0-9]|2[0-3]|[0-9]H|[1-2][0-3]H|HH):([0-5][0-9]|[0-5]?[0-9]M|MM):([0-5][0-9]|[0-5]?[0-9]S|SS)$";
const CYCLE_EXPR: &str = r"(?:HH|MM|SS)|(?:(?<num>\d+)(?<type>[HMS])?)";

pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(SPEC_EXPR).unwrap());

static CYCLE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CYCLE_EXPR).unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_spec_from_str() {
        let time_spec = "12H:30M:5S".parse::<TimeSpec>().unwrap();
        assert_eq!(
            &time_spec.legs,
            &HashSet::from_iter(vec![
                Unit::Hour(Cycle::Every(12)),
                Unit::Minutes(Cycle::Every(30)),
                Unit::Seconds(Cycle::Every(5)),
            ])
        );

        dbg!(time_spec.next(&DateTime::parse_from_rfc3339("2021-01-01T12:20:05Z").unwrap()));

        let time_spec = "HH:30M:5S".parse::<TimeSpec>().unwrap();
        dbg!(time_spec.next(&DateTime::parse_from_rfc3339("2021-01-01T12:20:05Z").unwrap()));
    }
}

impl NextTime for TimeSpec {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz> {
        let mut next = from.clone();
        for leg in &self.legs {
            match leg {
                Unit::Hour(Cycle::At(h)) => {
                    next = next.with_hour(*h as u32).unwrap();
                }
                Unit::Minutes(Cycle::At(m)) => {
                    next = next.with_minute(*m as u32).unwrap();
                }
                Unit::Seconds(Cycle::At(s)) => {
                    next = next.with_second(*s as u32).unwrap();
                }
                Unit::Hour(Cycle::Every(h)) => next = next + Duration::hours(*h as i64),
                Unit::Minutes(Cycle::Every(m)) => next = next + Duration::minutes(*m as i64),
                Unit::Seconds(Cycle::Every(s)) => next = next + Duration::seconds(*s as i64),
                _ => {}
            }
        }
        next
    }
}
