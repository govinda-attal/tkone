use crate::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::str::FromStr;

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    #[default]
    NA,
    On(u32, DayOption),
    Every(u32),
    EveryBizDay(u32),
    Last(LastDayOption),
    WeekDay(chrono::Weekday, WeekdayOption),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    #[default]
    NA,
    Starting(Option<u8>),
    Ending(Option<u8>),
    Every(u8),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayOption {
    #[default]
    NA,
    LastDay,
    NextMonthFirstDay,
    NextMonthOverflow,
    Weekday,
    LastWeekday,
    NextMonthFirstWeekday,
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum LastDayOption {
    #[default]
    NA,
    Weekday,
    LastWeekday,
    NextMonthFirstWeekday,
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

    #[test]
    fn test_one() {
        let spec = Spec::from_str("YY:MM:29LW:P").unwrap();
        dbg!(&spec);
        // todo!("correct this")
    }

    #[test]
    fn test_weekday() {
        let spec = Spec::from_str("YY:MM:31N:2N").unwrap();
        dbg!(&spec);
    }
}

const MONTH_EXPR: &str = r"(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM)";
const YEAR_EXPR: &str = r"(YY|19|20\d{2}|1Y)";
const DAY_EXPR: &str = r"(DD|BB|LW|FW|L|[1-9][BDW]|0[1-9]|[12][0-8][BDW]?|29(?:NW|LW|[BDLNOW])?|3[01](?:NW|LW|[BDLNOW])?)";
const BDAY_ADJ_EXPR: &str = r"(?::([1-9]{0,1}[PN]))?";
const WEEKDAY_EXPR: &str = r"([MTWRFSU](?:#[1-3]|[12]{0,1}L|[1-3]{0,1}))";

const DAY_EXTRACTOR_EXPR: &str = r"(?:(?<wd>[MTWRFSU])(?:(?<last_num>[12])?(?<last>L)|(?<start_num>[1-3])|#(?<every>[1-3]))?)|(?:(?:DD|BB)|(?<num>\d+)?(?<type>LW|NW|[BDLNOW]))";
// pub static SPEC_EXPR: Lazy<String> =
//     Lazy::new(|| "(YY|19|20\\d{2}|1Y):(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM):(?:(DD|BB|[1-9][BD]|0[1-9]|[12][0-8][BDW]?|29[BDWLFO]?|3[01][BDWLFO]?|LW|FW|L)|([MTWRFSU](?:[2]?L|[1-3]|#[1-3])?))(?::([1-9]{0,1}[PN]))?".into());

pub static SPEC_EXPR: Lazy<String> = Lazy::new(|| {
    format!("{YEAR_EXPR}:{MONTH_EXPR}:(?:{WEEKDAY_EXPR}|{DAY_EXPR}){BDAY_ADJ_EXPR}").to_string()
});

// pub const SPEC_EXPR: &str = r"(YY|19|20\d{2}|1Y):(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM):(DD|BB|[1-9][BD]|0[1-9]|[12][0-8][BD]?|29[BDLFO]?|3[01][BDLFO]?|[LFO])|(?:[MTWRFSU](?:[2]?L|[1-3]|#[1-3]))|(?:(?:[1-9]|1[0-9]|2[0-3])W)(?::([1-9]{0,1}[PN]))?";
const CYCLE_EXPR: &str = r"(?:YY|MM|DD|BB)|(?:(?<num>\d+)?(?<type>[YMBDBDLFOPN]|LW|FW)?)";

pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(SPEC_EXPR.as_str()).unwrap());
static CYCLE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CYCLE_EXPR).unwrap());
static DAY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(DAY_EXTRACTOR_EXPR).unwrap());

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

        let caps = caps.iter().filter_map(|m| m).collect::<Vec<_>>();

        let years = caps
            .get(1)
            .map(|m| Cycle::from_str(m.as_str()))
            .expect("")?;
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
        let cycle = DAY_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid day spec"))?;

        if let Some(wd) = cycle.name("wd") {
            let wd = wd.as_str();
            let weekday = match wd {
                "M" => chrono::Weekday::Mon,
                "T" => chrono::Weekday::Tue,
                "W" => chrono::Weekday::Wed,
                "R" => chrono::Weekday::Thu,
                "F" => chrono::Weekday::Fri,
                "S" => chrono::Weekday::Sat,
                "U" => chrono::Weekday::Sun,
                _ => Err(Error::ParseError("Invalid weekday spec"))?,
            };

            let wd_option = if let Some(_) = cycle.name("last") {
                WeekdayOption::Ending(None)
            } else if let Some(num) = cycle.name("last_num") {
                let num = num.as_str().parse::<u8>().unwrap();
                WeekdayOption::Ending(Some(num))
            } else if let Some(num) = cycle.name("start_num") {
                let num = num.as_str().parse::<u8>().unwrap();
                WeekdayOption::Starting(Some(num))
            } else if let Some(every) = cycle.name("every") {
                let every = every.as_str().parse::<u8>().unwrap();
                WeekdayOption::Every(every)
            } else {
                WeekdayOption::NA
            };
            return Ok(DayCycle::WeekDay(weekday, wd_option));
        }

        let Some(num) = cycle.name("num") else {
            let Some(ty) = cycle.name("type") else {
                return Ok(DayCycle::NA);
            };
            match ty.as_str() {
                "L" => return Ok(DayCycle::Last(LastDayOption::NA)),
                "LW" => return Ok(DayCycle::Last(LastDayOption::Weekday)),
                "M" => return Ok(DayCycle::WeekDay(chrono::Weekday::Mon, WeekdayOption::NA)),
                "T" => return Ok(DayCycle::WeekDay(chrono::Weekday::Tue, WeekdayOption::NA)),
                "W" => return Ok(DayCycle::WeekDay(chrono::Weekday::Wed, WeekdayOption::NA)),
                "R" => return Ok(DayCycle::WeekDay(chrono::Weekday::Thu, WeekdayOption::NA)),
                "F" => return Ok(DayCycle::WeekDay(chrono::Weekday::Fri, WeekdayOption::NA)),
                "S" => return Ok(DayCycle::WeekDay(chrono::Weekday::Sat, WeekdayOption::NA)),
                "U" => return Ok(DayCycle::WeekDay(chrono::Weekday::Sun, WeekdayOption::NA)),
                _ => return Ok(DayCycle::Last(LastDayOption::NA)),
            };
        };

        let num = num.as_str().parse::<u32>().unwrap();
        let Some(ty) = cycle.name("type") else {
            return Ok(DayCycle::On(num, DayOption::NA));
        };

        let cycle = match ty.as_str() {
            "D" => DayCycle::Every(num),
            "B" => DayCycle::EveryBizDay(num),
            "L" => DayCycle::On(num, DayOption::LastDay),
            "N" => DayCycle::On(num, DayOption::NextMonthFirstDay),
            "O" => DayCycle::On(num, DayOption::NextMonthOverflow),
            "W" => DayCycle::On(num, DayOption::Weekday),
            "LW" => DayCycle::On(num, DayOption::LastWeekday),
            "NW" => DayCycle::On(num, DayOption::NextMonthFirstWeekday),
            _ => Err(Error::ParseError("Invalid time spec"))?,
        };

        Ok(cycle)
    }
}
