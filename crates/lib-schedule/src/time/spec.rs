use chrono::{DateTime, Duration, TimeZone, Timelike};
use once_cell::sync::Lazy;
use regex::Regex;
use std::default;
use std::str::FromStr;

use crate::prelude::*;

#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    #[default]
    NA,
    At(u8),
    Every(u8),
}

#[derive(Default, Debug, PartialEq)]
pub struct Spec {
    pub hours: Cycle,
    pub minutes: Cycle,
    pub seconds: Cycle,
}

impl FromStr for Spec {
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

        Ok(Self {
            hours: cycles.remove(0),
            minutes: cycles.remove(0),
            seconds: cycles.remove(0),
        })
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
        let time_spec = "12H:30M:5S".parse::<Spec>().unwrap();
        assert_eq!(
            &time_spec,
            &Spec {
                hours: Cycle::Every(12),
                minutes: Cycle::Every(30),
                seconds: Cycle::Every(5),
                ..Default::default()
            },
        );
    }
}
