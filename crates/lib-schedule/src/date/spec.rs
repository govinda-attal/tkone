use crate::prelude::*;
use chrono::Weekday;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{ops::Deref, str::FromStr};

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

const MONTH_EXPR: &str = r"(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM)";
const YEAR_EXPR: &str = r"(YY|19|20\d{2}|1Y)";
const DAY_EXPR: &str = r"(DD|BB|LW|FW|L|[1-9][BDW]|0[1-9]|[12][0-8][BDW]?|29(?:NW|LW|[BDLNOW])?|3[01](?:NW|LW|[BDLNOW])?)";
const BDAY_ADJ_EXPR: &str = r"(?::([1-9]{0,1}[PN]))?";
const WEEKDAY_EXPR: &str = r"([MTWRFSU](?:#[1-3]|[1-4]{0,1}L|[1-3]{0,1}))";

const DAY_EXTRACTOR_EXPR: &str = r"(?:(?<wd>[MTWRFSU])(?:(?<last_num>[12])?(?<last>L)|(?<start_num>[1-3])|#(?<every>[1-3]))?)|(?:(?:DD|BB)|(?<num>\d+)?(?<type>LW|NW|[BDLNOW]))";


/// ## SPEC_EXPR
/// Regular expression for matching date recurrence specifications.
/// It matches various combinations of years, months, and days.
///
/// ### Supported Formats
///
/// - `YY:MM:DD`: Date format with years in the range 1900-2099, months in the range 01-12, and days in the range 01-31.
/// - `<num>Y:<num>M:<num>D`: Duration format with years, months, and days specified as numbers followed by `Y`, `M`, and `D` respectively.
/// - `YY:MM:<num>B`: Duration format with business days specified as number followed by `B`.
/// - `YY:MM:(M|T|W|R|F|S|U)(<num>?L|<num>)?`: Date format with weekdays.
///
/// ### Examples
/// - `YY:MM:1D`: Is recurrence specification for every day.
/// - `YY:1M:DD`: Is recurrence specification for every month on the specified day.
/// - `YY:1M:1W`: Is recurrence specification for nearest weekday to 1st of every month.
/// - `YY:1M:15W`: Is recurrence specification for nearest weekday to the 15th of every month.
/// - `YY:1M:L`: Is recurrence specification for last day of every month.
/// - `YY:1M:29L`: Is recurrence specification for 29th of every month or last day in case of February.
/// - `YY:1M:T1`: Is recurrence specification for first Tuesday of every month.
/// - `YY:1M:T2`: Is recurrence specification for second Tuesday of every month.
/// - `YY:1M:T2L`: Is recurrence specification for second last Tuesday of every month.
/// - `YY:1M:TL`: Is recurrence specification for last Tuesday of every month
/// 
pub static SPEC_EXPR: Lazy<String> = Lazy::new(|| {
    format!("{YEAR_EXPR}:{MONTH_EXPR}:(?:{WEEKDAY_EXPR}|{DAY_EXPR}){BDAY_ADJ_EXPR}").to_string()
});

const CYCLE_EXPR: &str = r"(?:YY|MM|DD|BB)|(?:(?<num>\d+)?(?<type>[YMBDBDLFOPN]|LW|FW)?)";

static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(SPEC_EXPR.as_str()).unwrap());
static CYCLE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CYCLE_EXPR).unwrap());
static DAY_RE: Lazy<Regex> = Lazy::new(|| Regex::new(DAY_EXTRACTOR_EXPR).unwrap());

#[derive(Default, Debug, Clone, PartialEq)]
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

fn weekday_code(wd: &Weekday) -> char {
    match wd {
        Weekday::Mon => 'M',
        Weekday::Tue => 'T',
        Weekday::Wed => 'W',
        Weekday::Thu => 'R',
        Weekday::Fri => 'F',
        Weekday::Sat => 'S',
        Weekday::Sun => 'U',
    }
}

impl ToString for Spec {
    fn to_string(&self) -> String {
        let to_string = |cycle: &Cycle, cycle_type: char| match cycle {
            Cycle::NA => f!("{}{}", cycle_type, cycle_type),
            Cycle::In(num) => f!("{:02}", num),
            Cycle::Every(num) => f!("{}{}", num, cycle_type),
        };
        let day_to_string = |cycle: &DayCycle| match cycle {
            DayCycle::NA => "DD".to_string(),
            DayCycle::On(num, DayOption::NA) => num.to_string(),
            DayCycle::On(num, DayOption::LastDay) => f!("{:02}L", num),
            DayCycle::On(num, DayOption::NextMonthFirstDay) => f!("{:02}N", num),
            DayCycle::On(num, DayOption::NextMonthOverflow) => f!("{:02}O", num),
            DayCycle::On(num, DayOption::Weekday) => f!("{:02}W", num),
            DayCycle::On(num, DayOption::LastWeekday) => f!("{:02}LW", num),
            DayCycle::On(num, DayOption::NextMonthFirstWeekday) => f!("{:02}NW", num),
            DayCycle::Every(num) => f!("{:02}D", num),
            DayCycle::EveryBizDay(num) => f!("{:02}B", num),
            DayCycle::Last(LastDayOption::NA) => "L".to_string(),
            DayCycle::Last(LastDayOption::Weekday) => "LW".to_string(),
            DayCycle::WeekDay(wd, WeekdayOption::NA) => wd.to_string(),
            DayCycle::WeekDay(wd, WeekdayOption::Starting(Some(num))) => {
                f!("{:?}#{}", weekday_code(wd), num)
            }
            DayCycle::WeekDay(wd, WeekdayOption::Ending(Some(num))) => {
                f!("{:?}#{}L", weekday_code(wd), num)
            }
            DayCycle::WeekDay(wd, WeekdayOption::Every(num)) => {
                f!("{:?}#{}", weekday_code(wd), num)
            }
            _ => "DD".to_string(),
        };
        let spec_str = f!(
            "{}:{}:{}",
            to_string(&self.years, 'Y'),
            to_string(&self.months, 'M'),
            day_to_string(&self.days),
        );
        if let Some(biz_day_step) = &self.biz_day_step {
            f!("{}:{}", spec_str, biz_day_step.to_string())
        } else {
            spec_str
        }
    }
}

impl ToString for BizDayStep {
    fn to_string(&self) -> String {
        match self {
            BizDayStep::NA => "".to_string(),
            BizDayStep::Prev(num) => {
                f!("{}P", num.gt(&1).then(|| f!("{}", num)).unwrap_or_default())
            }
            BizDayStep::Next(num) => {
                f!("{}N", num.gt(&1).then(|| f!("{}", num)).unwrap_or_default())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_one() {
        let spec = Spec::from_str("YY:1M:29LW:P").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::NA,
                months: Cycle::Every(1),
                days: DayCycle::On(29, DayOption::LastWeekday),
                biz_day_step: Some(BizDayStep::Prev(1)),
            },
        );
        assert_eq!(spec.to_string(), "YY:1M:29LW:P");
    }

    #[test]
    fn test_weekday() {
        let spec = Spec::from_str("2024:MM:31N:2N").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::NA,
                days: DayCycle::On(31, DayOption::NextMonthFirstDay),
                biz_day_step: Some(BizDayStep::Next(2)),
            },
        );
        assert_eq!(&spec.to_string(), "2024:MM:31N:2N");
    }
}
