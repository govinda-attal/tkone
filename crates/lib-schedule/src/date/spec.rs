use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

use crate::biz_day::WeekendSkipper;
use crate::prelude::*;
use crate::BizDayProcessor;

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    #[default]
    NA,
    On(u32, Option<DayOverflow>),
    Every(u32),
    EveryBizDay(u32),
    Overflow(DayOverflow),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayOverflow {
    #[default]
    MonthLastDay,
    NextMonthFirstDay,
    NextMonthOverflow,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum BizDayStep {
    #[default]
    NA,
    Prev(u32),
    Next(u32),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub enum Cycle {
    #[default]
    NA,
    In(u32),
    Every(u32),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use chrono_tz::Australia::Sydney;

    #[test]
    fn test_one() {
        let spec = Spec::from_str("YY:MM:29O:P").unwrap();
        dbg!(&spec);
    }
}

pub const SPEC_EXPR: &str = r"(YY|19|20\d{2}|1Y):(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM):(DD|BB|[1-9][BD]|0[1-9]|[12][0-8][BD]?|29[BDLFO]?|3[01][BDLFO]?|[LFO])(?::([1-9]{0,1}[PN]))?";
const CYCLE_EXPR: &str = r"(?:YY|MM|DD|BB)|(?:(?<num>\d+)?(?<type>[YMBDBDLFOPN])?)";

pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(SPEC_EXPR).unwrap());
static CYCLE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CYCLE_EXPR).unwrap());

#[derive(Default, Debug, Clone)]
pub struct Spec {
    pub years: Cycle,
    pub months: Cycle,
    pub days: DayCycle,
    pub biz_day_step: Option<BizDayStep>,
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid date spec"))?;
        let years = caps
            .get(1)
            .map(|m| Cycle::from_str(m.as_str()))
            .expect("missing year spec")?;
        let months = caps
            .get(2)
            .map(|m| Cycle::from_str(m.as_str()))
            .expect("missing month spec")?;
        let days = caps
            .get(3)
            .map(|m| DayCycle::from_str(m.as_str()))
            .expect("missing day spec")?;
        let biz_day_step = caps.get(4).map(|m| BizDayStep::from_str(m.as_str()));
        let biz_day_step = if let Some(biz_day_step) = biz_day_step {
            biz_day_step.ok()
        } else {
            None
        };

        Ok(Self {
            years,
            months,
            days,
            biz_day_step,
        })
    }
}

impl FromStr for Cycle {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let cycle = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid year or month spec"))?;

        let Some(num) = cycle.name("num") else {
            return Ok(Cycle::NA);
        };
        let num = num.as_str().parse::<u32>().unwrap();
        let cycle = if cycle.name("type").is_some() {
            Cycle::Every(num)
        } else {
            Cycle::In(num)
        };
        Ok(cycle)
    }
}

impl FromStr for BizDayStep {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let step = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid biz day step spec"))?;

        let num = if let Some(num) = step.name("num") {
            num.as_str().parse::<u32>().unwrap()
        } else {
            1
        };
        // let num = num.as_str().parse::<u8>().unwrap();
        let step = if let Some(ty) = step.name("type") {
            match ty.as_str() {
                "P" => BizDayStep::Prev(num),
                "N" => BizDayStep::Next(num),
                _ => BizDayStep::NA,
            }
        } else {
            BizDayStep::NA
        };
        Ok(step)
    }
}

impl FromStr for DayCycle {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        let cycle = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid day spec"))?;

        let Some(num) = cycle.name("num") else {
            let Some(ty) = cycle.name("type") else {
                return Ok(DayCycle::NA);
            };
            match ty.as_str() {
                "F" => return Ok(DayCycle::Overflow(DayOverflow::NextMonthFirstDay)),
                "O" => return Ok(DayCycle::Overflow(DayOverflow::NextMonthOverflow)),
                _ => return Ok(DayCycle::Overflow(DayOverflow::MonthLastDay)),
            };
        };

        let num = num.as_str().parse::<u32>().unwrap();
        let Some(ty) = cycle.name("type") else {
            return Ok(DayCycle::On(num, None));
        };

        let cycle = match ty.as_str() {
            "D" => DayCycle::Every(num),
            "B" => DayCycle::EveryBizDay(num),
            "L" => DayCycle::On(num, Some(DayOverflow::MonthLastDay)),
            "F" => DayCycle::On(num, Some(DayOverflow::NextMonthFirstDay)),
            "O" => DayCycle::On(num, Some(DayOverflow::NextMonthOverflow)),
            _ => Err(Error::ParseError("Invalid time spec"))?,
        };

        Ok(cycle)
    }
}
