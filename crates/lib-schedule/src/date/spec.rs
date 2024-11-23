use crate::{biz_day::Direction as AdjustmentDirection, prelude::*};
use chrono::Weekday;
use std::{collections::BTreeSet, sync::LazyLock};

use regex::Regex;
use std::str::FromStr;

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    #[default]
    NA,
    Every(u32, EveryDayOption),
    OnDays(BTreeSet<u32>),
    On(u32, LastDayOption),
    OnWeekDay(chrono::Weekday, WeekdayOption),
    OnWeekDays(Vec<chrono::Weekday>),
    OnLastDay,
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    #[default]
    NA,
    Starting(Option<u8>),
    Ending(Option<u8>),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum LastDayOption {
    #[default]
    NA,
    LastDay,
    NextMonthFirstDay,
    NextMonthOverflow,
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum EveryDayOption {
    #[default]
    Regular,
    BizDay,
    WeekDay,
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
    Values(BTreeSet<u32>),
    Every(u32),
}

const MONTH_EXPR: &str =
    r"((?:\[(?:0[1-9]|1[0-2])(?:,(?:0[1-9]|1[0-2]))*\])|(?:MM|\d+M|0[1-9]|1[0-2]))";
const YEAR_EXPR: &str = r"((?:\[(?:20[0-9]{2})(?:,20[0-9]{2})*\])|(?:YY|19|20[0-9]{2}|1Y))";
const DAY_EXPR: &str = r"(?:(?:\[(?:0[1-9]|[12][0-9]|3[01])(?:,(?:0[1-9]|[12][0-9]|3[01]))*\])|(?:DD|L|[1-9](?:BD|WD|D)|0[1-9]|[12][0-8](?:BD|WD|D)?|29(?:BD|WD|D|L|N|O)?|3[01](?:BD|WD|D|L|N|O)?))";
const BDAY_ADJ_EXPR: &str = r"(?:~(PW|NW|PB|NB|B|W|[1-9]{0,1}[PN]))?";
const WEEKDAY_EXPR: &str = r"(?:(?:\[(?:MON|TUE|WED|THU|FRI|SAT|SUN)(?:,(?:MON|TUE|WED|THU|FRI|SAT|SUN))*\])|(?:MON|TUE|WED|THU|FRI|SAT|SUN)(?:#(?:L|[1-4]{0,1}L|[1-4]|L)){0,1})";

const CYCLE_EXPR: &str =
    r"(?:(?:\[(?<values>\d+(?:,\d+)*)\])|(:?(?:YY|MM)|(?:(?<num>\d+)?(?<type>[YMPN])?)))";
const DAY_EXTRACTOR_EXPR: &str = r"(?:(?:\[(?<d_values>\d+(?:,\d+)*)\])|(?:\[(?<wd_values>(:?(?:MON|TUE|WED|THU|FRI|SAT|SUN))(?:,(?:MON|TUE|WED|THU|FRI|SAT|SUN))*)\])|(?:(?<wd>MON|TUE|WED|THU|FRI|SAT|SUN)(?:#(?<last_num>[1-4])L|#(?<last>L)|#(?<start_num>[1-4]))?)|(?:(?:DD|BB)|(?<num>\d+)?(?<type>BD|WD|[DLNO])?))";
/// ## SPEC_EXPR
/// Regular expression for matching date recurrence specifications.
/// It matches various combinations of years, months, and days.
///
/// ### Supported Formats
///
/// - `YY-MM-DD`: Date format with years in the range 1900-2099, months in the range 01-12, and days in the range 01-31.
/// - `<num>Y-<num>M-<num>D`: Duration format with years, months, and days specified as numbers followed by `Y`, `M`, and `D` respectively.
/// - `YY-MM-<num>BD`: Duration format with business days specified as number followed by `BD`.
/// - `YY-MM-(MON|TUE|WED|THU|FRI|SAT|SUN)(<num>?L|<num>)?`: Date format with weekdays.
/// - `YY-MM-DD~(PW|NW|PB|NB|B|W|<num>P|<num>N)?`: Date format with business day adjustments.
///
/// ### Examples
/// - `YY-MM-1D`: Recurrence specification for every day.
/// - `YY-MM-1BD`: Recurrence specification for every business day.
/// - `YY-MM-1WD`: Recurrence specification for every weekday.
/// - `1Y-01-01`: Recurrence specification for every year on the 1st of January.
/// - `2024-1M-01`: Recurrence specification for 1st of every month in 2024.
/// - `YY-1M-DD`: Recurrence specification for every month on the specified day.
/// - `YY-1M-01~W`: Recurrence specification for nearest weekday to 1st of every month.
/// - `YY-1M-15~W`: Recurrence specification for 15th of every month adjusted to nearest weekday.
/// - `YY-1M-15~PW`: Recurrence specification for 15th of every month adjusted to nearest(on previous side) weekday.
/// - `YY-1M-15~NB`: Recurrence specification for 15th of every month adjusted to nearest(on next side) business day.
/// - `YY-1M-L`: Recurrence specification for last day of every month.
/// - `YY-1M-29L`: Recurrence specification for 29th of every month or last day in case of February.
/// - `YY-1M-TUE#1`: Recurrence specification for first Tuesday of every month.
/// - `YY-1M-TUE#2`: Recurrence specification for second Tuesday of every month.
/// - `YY-1M-TUE#2L`: Recurrence specification for second last Tuesday of every month.
/// - `YY-1M-TUE#L`: Recurrence specification for last Tuesday of every month
/// - `YY-1M-TUE#L~B`: Recurrence specification for last Tuesday of every month adjusted to nearest business day.
/// - `YY-MM-TUE`: Recurrence specification for every Tuesday.
pub static SPEC_EXPR: LazyLock<String> = LazyLock::new(|| {
    format!("{YEAR_EXPR}-{MONTH_EXPR}-({WEEKDAY_EXPR}|{DAY_EXPR}){BDAY_ADJ_EXPR}").to_string()
});

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
        dbg!(&SPEC_EXPR.to_string());
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

        if let Some(values) = cycle.name("values") {
            let values: BTreeSet<u32> = values
                .as_str()
                .split(',')
                .map(|v| v.parse::<u32>().unwrap())
                .collect();
            if values.len() > 1 {
                return Ok(Cycle::Values(values));
            }
            return Ok(Cycle::In(values.into_iter().next().unwrap()));
        }

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

        if let Some(d_values) = cycle.name("d_values") {
            let values: BTreeSet<u32> = d_values
                .as_str()
                .split(',')
                .map(|v| v.parse::<u32>().unwrap())
                .collect();
            if values.len() > 1 {
                return Ok(DayCycle::OnDays(values));
            }
            return Ok(DayCycle::On(
                values.into_iter().next().unwrap(),
                LastDayOption::NA,
            ));
        }

        if let Some(wd_values) = cycle.name("wd_values") {
            let values: Vec<chrono::Weekday> = wd_values
                .as_str()
                .split(',')
                .map(|v| v.parse::<chrono::Weekday>().unwrap())
                .collect();
            if values.len() > 1 {
                return Ok(DayCycle::OnWeekDays(values));
            }
            return Ok(DayCycle::OnWeekDay(
                values.into_iter().next().unwrap(),
                WeekdayOption::NA,
            ));
        }

        if let Some(wd) = cycle.name("wd") {
            let wd = wd.as_str();
            let weekday = match wd {
                "MON" => chrono::Weekday::Mon,
                "TUE" => chrono::Weekday::Tue,
                "WED" => chrono::Weekday::Wed,
                "THU" => chrono::Weekday::Thu,
                "FRI" => chrono::Weekday::Fri,
                "SAT" => chrono::Weekday::Sat,
                "SUN" => chrono::Weekday::Sun,
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
            return Ok(DayCycle::OnWeekDay(weekday, wd_option));
        }

        let Some(num) = cycle.name("num") else {
            let Some(ty) = cycle.name("type") else {
                return Ok(DayCycle::NA);
            };
            match ty.as_str() {
                "L" => return Ok(DayCycle::OnLastDay),
                "MON" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Mon, WeekdayOption::NA)),
                "TUE" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Tue, WeekdayOption::NA)),
                "WED" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Wed, WeekdayOption::NA)),
                "THU" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Thu, WeekdayOption::NA)),
                "FRI" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Fri, WeekdayOption::NA)),
                "SAT" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Sat, WeekdayOption::NA)),
                "SUN" => return Ok(DayCycle::OnWeekDay(chrono::Weekday::Sun, WeekdayOption::NA)),
                _ => return Ok(DayCycle::NA),
            };
        };

        let num = num.as_str().parse::<u32>().unwrap();
        let Some(ty) = cycle.name("type") else {
            return Ok(DayCycle::On(num, LastDayOption::NA));
        };

        let cycle = match ty.as_str() {
            "BD" => DayCycle::Every(num, EveryDayOption::BizDay),
            "WD" => DayCycle::Every(num, EveryDayOption::WeekDay),
            "D" => DayCycle::Every(num, EveryDayOption::Regular),
            "L" => DayCycle::On(num, LastDayOption::LastDay),
            "N" => DayCycle::On(num, LastDayOption::NextMonthFirstDay),
            "O" => DayCycle::On(num, LastDayOption::NextMonthOverflow),
            _ => Err(Error::ParseError("Invalid time spec"))?,
        };

        Ok(cycle)
    }
}

fn weekday_code(wd: &Weekday) -> &'static str {
    match wd {
        Weekday::Mon => "MON",
        Weekday::Tue => "TUE",
        Weekday::Wed => "WED",
        Weekday::Thu => "THU",
        Weekday::Fri => "FRI",
        Weekday::Sat => "SAT",
        Weekday::Sun => "SUN",
    }
}

impl ToString for Spec {
    fn to_string(&self) -> String {
        let to_string = |cycle: &Cycle, cycle_type: char| match cycle {
            Cycle::NA => f!("{}{}", cycle_type, cycle_type),
            Cycle::In(num) => f!("{:02}", num),
            Cycle::Every(num) => f!("{}{}", num, cycle_type),
            Cycle::Values(values) => {
                let values = values
                    .iter()
                    .map(|v| f!("{:02}", v))
                    .collect::<Vec<_>>()
                    .join(",");
                f!("[{}]", values)
            }
        };
        let day_to_string = |cycle: &DayCycle| match cycle {
            DayCycle::NA => "DD".to_string(),
            DayCycle::On(num, LastDayOption::NA) => f!("{:02}", num),
            DayCycle::On(num, LastDayOption::LastDay) => f!("{:02}L", num),
            DayCycle::On(num, LastDayOption::NextMonthFirstDay) => f!("{:02}N", num),
            DayCycle::On(num, LastDayOption::NextMonthOverflow) => f!("{:02}O", num),
            DayCycle::OnDays(values) => {
                let values = values
                    .iter()
                    .map(|v| f!("{:02}", v))
                    .collect::<Vec<_>>()
                    .join(",");
                f!("[{}]", values)
            }
            DayCycle::OnWeekDays(values) => {
                let values = values
                    .iter()
                    .map(|v| weekday_code(v))
                    .collect::<Vec<_>>()
                    .join(",");
                f!("[{}]", values)
            }
            DayCycle::Every(num, EveryDayOption::Regular) => f!("{}D", num),
            DayCycle::Every(num, EveryDayOption::BizDay) => f!("{}BD", num),
            DayCycle::Every(num, EveryDayOption::WeekDay) => f!("{}WD", num),
            DayCycle::OnLastDay => "L".to_string(),
            DayCycle::OnWeekDay(wd, WeekdayOption::NA) => wd.to_string(),
            DayCycle::OnWeekDay(wd, WeekdayOption::Starting(Some(num))) => {
                f!("{}#{}", weekday_code(wd), num)
            }
            DayCycle::OnWeekDay(wd, WeekdayOption::Ending(Some(num))) => {
                f!("{}#{}L", weekday_code(wd), num)
            }
            _ => "DD".to_string(),
        };
        let spec_str = f!(
            "{}-{}-{}",
            to_string(&self.years, 'Y'),
            to_string(&self.months, 'M'),
            day_to_string(&self.days),
        );
        if let Some(biz_day_adj) = &self.biz_day_adj {
            f!("{}~{}", spec_str, biz_day_adj.to_string())
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
        let spec = Spec::from_str("YY-1M-29L~W").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::NA,
                months: Cycle::Every(1),
                days: DayCycle::On(29, LastDayOption::LastDay),
                biz_day_adj: Some(BizDayAdjustment::Weekday(AdjustmentDirection::Nearest)),
            },
        );
        assert_eq!(spec.to_string(), "YY-1M-29L~W");
    }

    #[test]
    fn test_two() {
        let spec = Spec::from_str("YY-1M-1WD").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::NA,
                months: Cycle::Every(1),
                days: DayCycle::Every(1, EveryDayOption::WeekDay),
                biz_day_adj: None,
            },
        );
        assert_eq!(spec.to_string(), "YY-1M-1WD");
    }

    #[test]
    fn test_31st() {
        let spec = Spec::from_str("2024-MM-31L~3P").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::NA,
                days: DayCycle::On(31, LastDayOption::LastDay),
                biz_day_adj: Some(BizDayAdjustment::Prev(3)),
            },
        );
        assert_eq!(&spec.to_string(), "2024-MM-31L~3P");
    }

    #[test]
    fn test_weekday() {
        let spec = Spec::from_str("2024-1M-TUE#2L~3P").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::Every(1),
                days: DayCycle::OnWeekDay(chrono::Weekday::Tue, WeekdayOption::Ending(Some(2))),
                biz_day_adj: Some(BizDayAdjustment::Prev(3)),
            },
        );
        assert_eq!(&spec.to_string(), "2024-1M-TUE#2L~3P");
    }

    #[test]
    fn test_month_year_set() {
        let spec = Spec::from_str("[2024]-[01,02]-TUE#2L~3P").unwrap();
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::Values(BTreeSet::from_iter(vec![1, 2])),
                days: DayCycle::OnWeekDay(chrono::Weekday::Tue, WeekdayOption::Ending(Some(2))),
                biz_day_adj: Some(BizDayAdjustment::Prev(3)),
            },
        );
        assert_eq!(&spec.to_string(), "2024-[01,02]-TUE#2L~3P");
    }

    #[test]
    fn test_month_day_set() {
        let spec = Spec::from_str("2024-[01,02]-[05,10,15]~3P").unwrap();
        dbg!(&spec);
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::Values(BTreeSet::from_iter(vec![1, 2])),
                days: DayCycle::OnDays(BTreeSet::from_iter(vec![5, 10, 15])),
                biz_day_adj: Some(BizDayAdjustment::Prev(3)),
            },
        );
        assert_eq!(&spec.to_string(), "2024-[01,02]-[05,10,15]~3P");
    }

    #[test]
    fn test_month_weekday_set() {
        let spec = Spec::from_str("2024-[01,02]-[SAT,SUN]~3P").unwrap();
        dbg!(&spec);
        assert_eq!(
            &spec,
            &Spec {
                years: Cycle::In(2024),
                months: Cycle::Values(BTreeSet::from_iter(vec![1, 2])),
                days: DayCycle::OnWeekDays(Vec::from_iter(vec![
                    chrono::Weekday::Sat,
                    chrono::Weekday::Sun
                ])),
                biz_day_adj: Some(BizDayAdjustment::Prev(3)),
            },
        );
        assert_eq!(&spec.to_string(), "2024-[01,02]-[SAT,SUN]~3P");
    }
}
