use crate::{biz_day::Direction as AdjustmentDirection, prelude::*};
use chrono::Weekday;
use std::sync::LazyLock;

use regex::Regex;
use std::str::FromStr;

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    #[default]
    NA,
    On(u32, DayOption),
    Every(u32),
    EveryBizDay(u32),
    Last,
    WeekDay(chrono::Weekday, WeekdayOption),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    #[default]
    NA,
    Starting(Option<u8>),
    Ending(Option<u8>),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayOption {
    #[default]
    NA,
    LastDay,
    NextMonthFirstDay,
    NextMonthOverflow,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum BizDayAdjustment {
    #[default]
    NA,
    Weekday(AdjustmentDirection),
    BizDay(AdjustmentDirection),
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
const DAY_EXPR: &str =
    r"(DD|BB|L|[1-9][BD]|0[1-9]|[12][0-8][BD]?|29(?:[BDLNO])?|3[01](?:[BDLNO])?)";
const BDAY_ADJ_EXPR: &str = r"(?::(PW|NW|PB|NB|B|W|[1-9]{0,1}[PN]))?";
const WEEKDAY_EXPR: &str = r"([MTWRFSU](?:[1-4]{0,1}L|[1-4])?)";

const DAY_EXTRACTOR_EXPR: &str = r"(?:(?<wd>[MTWRFSU])(?:(?<last_num>[1-4])?(?<last>L)|(?<start_num>[1-4]))?)|(?:(?:DD|BB)|(?<num>\d+)?(?<type>[BDLNO])?)";

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
/// - `YY:MM:DD:(PW|NW|PB|NB|B|W|<num>P|<num>N)?`: Date format with business day adjustments.
///
/// ### Examples
/// - `YY:MM:1D`: Is recurrence specification for every day.
/// - `YY:1M:DD`: Is recurrence specification for every month on the specified day.
/// - `YY:1M:01:W`: Is recurrence specification for nearest weekday to 1st of every month.
/// - `YY:1M:15:W`: Is recurrence specification for nearest weekday to the 15th of every month.
/// - `YY:1M:15:PW`: Is recurrence specification for nearest(on previous side) weekday to 15th of every month.
/// - `YY:1M:15:NW`: Is recurrence specification for nearest(on next side) business day to 15th of every month.
/// - `YY:1M:L`: Is recurrence specification for last day of every month.
/// - `YY:1M:29L`: Is recurrence specification for 29th of every month or last day in case of February.
/// - `YY:1M:T1`: Is recurrence specification for first Tuesday of every month.
/// - `YY:1M:T2`: Is recurrence specification for second Tuesday of every month.
/// - `YY:1M:T2L`: Is recurrence specification for second last Tuesday of every month.
/// - `YY:1M:TL`: Is recurrence specification for last Tuesday of every month
/// - `YY:1M:TL:B`: Is recurrence specification for last Tuesday of every month or nearest business day.
/// - `YY:MM:T`: Is recurrence specification for every Tuesday.
pub static SPEC_EXPR: LazyLock<String> = LazyLock::new(|| {
    format!("{YEAR_EXPR}:{MONTH_EXPR}:(?:{WEEKDAY_EXPR}|{DAY_EXPR}){BDAY_ADJ_EXPR}").to_string()
});

const CYCLE_EXPR: &str = r"(?:YY|MM)|(?:(?<num>\d+)?(?<type>[YMPN])?)";

static SPEC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(SPEC_EXPR.as_str()).unwrap());
static CYCLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(CYCLE_EXPR).unwrap());
static DAY_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(DAY_EXTRACTOR_EXPR).unwrap());

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Spec {
    pub years: Cycle,
    pub months: Cycle,
    pub days: DayCycle,
    pub biz_day_adj: Option<BizDayAdjustment>,
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
        let biz_day_adj = caps.get(4).map(|m| BizDayAdjustment::from_str(m.as_str()));
        let biz_day_adj = if let Some(biz_day_adj) = biz_day_adj {
            biz_day_adj.ok()
        } else {
            None
        };

        Ok(Self {
            years,
            months,
            days,
            biz_day_adj,
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

impl FromStr for BizDayAdjustment {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "" => return Ok(BizDayAdjustment::NA),
            "W" => return Ok(BizDayAdjustment::Weekday(AdjustmentDirection::Nearest)),
            "B" => return Ok(BizDayAdjustment::BizDay(AdjustmentDirection::Nearest)),
            "PB" => return Ok(BizDayAdjustment::BizDay(AdjustmentDirection::Prev)),
            "NB" => return Ok(BizDayAdjustment::BizDay(AdjustmentDirection::Next)),
            "PW" => return Ok(BizDayAdjustment::Weekday(AdjustmentDirection::Prev)),
            "NW" => return Ok(BizDayAdjustment::Weekday(AdjustmentDirection::Next)),
            _ => (),
        }
        let adj = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid biz day adjustment spec"))?;

        let num = if let Some(num) = adj.name("num") {
            num.as_str().parse::<u32>().unwrap()
        } else {
            1
        };
        // let num = num.as_str().parse::<u8>().unwrap();
        let adj = if let Some(ty) = adj.name("type") {
            match ty.as_str() {
                "P" => BizDayAdjustment::Prev(num),
                "N" => BizDayAdjustment::Next(num),
                _ => BizDayAdjustment::NA,
            }
        } else {
            BizDayAdjustment::NA
        };
        Ok(adj)
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
                "L" => return Ok(DayCycle::Last),
                "M" => return Ok(DayCycle::WeekDay(chrono::Weekday::Mon, WeekdayOption::NA)),
                "T" => return Ok(DayCycle::WeekDay(chrono::Weekday::Tue, WeekdayOption::NA)),
                "W" => return Ok(DayCycle::WeekDay(chrono::Weekday::Wed, WeekdayOption::NA)),
                "R" => return Ok(DayCycle::WeekDay(chrono::Weekday::Thu, WeekdayOption::NA)),
                "F" => return Ok(DayCycle::WeekDay(chrono::Weekday::Fri, WeekdayOption::NA)),
                "S" => return Ok(DayCycle::WeekDay(chrono::Weekday::Sat, WeekdayOption::NA)),
                "U" => return Ok(DayCycle::WeekDay(chrono::Weekday::Sun, WeekdayOption::NA)),
                _ => return Ok(DayCycle::NA),
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
            DayCycle::On(num, DayOption::NA) => f!("{:02}", num),
            DayCycle::On(num, DayOption::LastDay) => f!("{:02}L", num),
            DayCycle::On(num, DayOption::NextMonthFirstDay) => f!("{:02}N", num),
            DayCycle::On(num, DayOption::NextMonthOverflow) => f!("{:02}O", num),
            DayCycle::Every(num) => f!("{:02}D", num),
            DayCycle::EveryBizDay(num) => f!("{:02}B", num),
            DayCycle::Last => "L".to_string(),
            DayCycle::WeekDay(wd, WeekdayOption::NA) => wd.to_string(),
            DayCycle::WeekDay(wd, WeekdayOption::Starting(Some(num))) => {
                f!("{:?}{}", weekday_code(wd), num)
            }
            DayCycle::WeekDay(wd, WeekdayOption::Ending(Some(num))) => {
                f!("{:?}{}L", weekday_code(wd), num)
            }
            _ => "DD".to_string(),
        };
        let spec_str = f!(
            "{}:{}:{}",
            to_string(&self.years, 'Y'),
            to_string(&self.months, 'M'),
            day_to_string(&self.days),
        );
        if let Some(biz_day_adj) = &self.biz_day_adj {
            f!("{}:{}", spec_str, biz_day_adj.to_string())
        } else {
            spec_str
        }
    }
}

impl ToString for BizDayAdjustment {
    fn to_string(&self) -> String {
        match self {
            BizDayAdjustment::NA => "".to_string(),
            BizDayAdjustment::Weekday(AdjustmentDirection::Nearest) => "W".to_string(),
            BizDayAdjustment::Weekday(AdjustmentDirection::Next) => "NW".to_string(),
            BizDayAdjustment::Weekday(AdjustmentDirection::Prev) => "PW".to_string(),
            BizDayAdjustment::BizDay(AdjustmentDirection::Nearest) => "B".to_string(),
            BizDayAdjustment::BizDay(AdjustmentDirection::Next) => "NB".to_string(),
            BizDayAdjustment::BizDay(AdjustmentDirection::Prev) => "PB".to_string(),
            BizDayAdjustment::Prev(num) => {
                f!("{}P", num.gt(&1).then(|| f!("{}", num)).unwrap_or_default())
            }
            BizDayAdjustment::Next(num) => {
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
        let spec = Spec::from_str("YY:1M:29L:W").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::NA,
                months: Cycle::Every(1),
                days: DayCycle::On(29, DayOption::LastDay),
                biz_day_adj: Some(BizDayAdjustment::Weekday(AdjustmentDirection::Nearest)),
            },
        );
        assert_eq!(spec.to_string(), "YY:1M:29L:W");
    }

    #[test]
    fn test_two() {
        let spec = Spec::from_str("YY:1M:08:W").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::NA,
                months: Cycle::Every(1),
                days: DayCycle::On(8, DayOption::NA),
                biz_day_adj: Some(BizDayAdjustment::Weekday(AdjustmentDirection::Nearest)),
            },
        );
        assert_eq!(spec.to_string(), "YY:1M:08:W");
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
                biz_day_adj: Some(BizDayAdjustment::Next(2)),
            },
        );
        assert_eq!(&spec.to_string(), "2024:MM:31N:2N");
    }
}
