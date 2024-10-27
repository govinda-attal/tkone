use std::{
    collections::HashSet,
    ops::{Add, Sub},
    str::FromStr,
};

use chrono::{DateTime, Datelike, Days, Months, TimeZone};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::error::{Error, Result};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    NA,
    On(u8),
    Every(u8),
    EveryBizDay(u8),
    LastDay(Option<u8>),
}

#[derive(Debug, PartialEq, Eq)]
pub enum BizDayStep {
    NA,
    Prev(u8),
    Next(u8),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Unit {
    Year(Cycle),
    Month(Cycle),
    Day(DayCycle),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    NA,
    In(u8),
    Every(u8),
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use chrono_tz::Australia::Sydney;

    #[test]
    fn test_one() {
        let spec = DateSpec::from_str("YY:MM:29:P").unwrap();
        dbg!(&spec);
        let now = Local::now().with_timezone(&Sydney);
        println!("{:?}", now);
        // println!("{:?}", spec.next_dtm(now).unwrap());
    }
}

pub const SPEC_EXPR: &str = r"(YY|19|20\d{2}|1Y):(MM|0[1-9]|1[0-2]|[1-9]M|1[0-2]M|MM):(DD|BB|[1-9][BD]|0[1-9]|[12][0-8][BD]?|29[BDL]?|3[01][BDL]?|L)(?::([1-9]{0,1}[PN]))?";
const CYCLE_EXPR: &str = r"(?:YY|MM|DD|BB)|(?:(?<num>\d+)?(?<type>[YMBDPNL])?)";

pub static SPEC_RE: Lazy<Regex> = Lazy::new(|| Regex::new(SPEC_EXPR).unwrap());
static CYCLE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(CYCLE_EXPR).unwrap());

#[derive(Default, Debug)]
pub struct DateSpec {
    pub legs: HashSet<Unit>,
    pub biz_day_step: Option<BizDayStep>,
}

impl FromStr for DateSpec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid date spec"))?;
        dbg!(&caps);
        let year = caps
            .get(1)
            .map(|m| Cycle::from_str(m.as_str()))
            .expect("missing year leg")?;
        let month = caps
            .get(2)
            .map(|m| Cycle::from_str(m.as_str()))
            .expect("missing month leg")?;
        let day = caps
            .get(3)
            .map(|m| DayCycle::from_str(m.as_str()))
            .expect("missing day leg")?;
        let biz_day_step = caps.get(4).map(|m| BizDayStep::from_str(m.as_str()));
        let biz_day_step = if let Some(biz_day_step) = biz_day_step {
            biz_day_step.ok()
        } else {
            None
        };

        let legs = HashSet::from_iter(vec![Unit::Year(year), Unit::Month(month), Unit::Day(day)]);

        Ok(Self { legs, biz_day_step })
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
        let num = num.as_str().parse::<u8>().unwrap();
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
            num.as_str().parse::<u8>().unwrap()
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
            if ty.as_str() == "L" {
                return Ok(DayCycle::LastDay(None));
            };
            return Ok(DayCycle::NA);
        };

        let num = num.as_str().parse::<u8>().unwrap();
        let Some(ty) = cycle.name("type") else {
            return Ok(DayCycle::On(num));
        };

        let cycle = match ty.as_str() {
            "D" => DayCycle::Every(num),
            "B" => DayCycle::EveryBizDay(num),
            "L" => DayCycle::LastDay(Some(num)),
            _ => Err(Error::ParseError("Invalid time spec"))?,
        };

        Ok(cycle)
    }
}

// impl DateSpec {
//     pub fn next_dtm<Tz: TimeZone>(&self, dtm: DateTime<Tz>) -> Result<DateTime<Tz>> {
//         let mut next_dtm =
//             self.intervals
//                 .iter()
//                 .fold(dtm, |next_dtm: DateTime<Tz>, c| -> DateTime<Tz> {
//                     match c {
//                         Cycle::Year(y) => next_dtm.add(Months::new(*y as u32 * 12)),
//                         Cycle::Month(m) => next_dtm.add(Months::new(*m as u32)),
//                         Cycle::Day(d) => next_dtm.add(Days::new(*d as u64)),
//                         Cycle::BusinessDay(bd) => add_business_days(*bd, next_dtm),
//                     }
//                 });

//         if let Some(kind) = &self.day_kind {
//             next_dtm = match kind {
//                 DayKind::Num(d) => next_dtm.with_day(*d as u32).unwrap(),
//                 DayKind::LastDay(d) => {
//                     if d.is_none() {
//                         adjust_month_last_day(next_dtm)
//                     } else {
//                         let d = d.unwrap() as u32;
//                         let adjusted_dtm = adjust_month_last_day(next_dtm);
//                         if adjusted_dtm.day().ge(&d) {
//                             adjusted_dtm.with_day(d).unwrap()
//                         } else {
//                             adjusted_dtm
//                         }
//                     }
//                 }
//             };
//         };

//         if let Some(step) = &self.business_day_step {
//             next_dtm = match step {
//                 BusinessDayStep::Prev => {
//                     let bck = match next_dtm.weekday() {
//                         chrono::Weekday::Sat => 1,
//                         chrono::Weekday::Sun => 2,
//                         _ => 0,
//                     };
//                     next_dtm.sub(Days::new(bck))
//                 }
//                 BusinessDayStep::Next => {
//                     let fwd_days = match next_dtm.weekday() {
//                         chrono::Weekday::Sat => 2,
//                         chrono::Weekday::Sun => 1,
//                         _ => 0,
//                     };
//                     next_dtm.add(Days::new(fwd_days))
//                 }
//             }
//         }
//         Ok(next_dtm)
//     }
// }

// fn add_business_days<Tz: TimeZone>(bd: u8, dtm: DateTime<Tz>) -> DateTime<Tz> {
//     let times = bd / 5;
//     let bal = bd % 5;

//     let next_dtm = (0..times)
//         .into_iter()
//         .into_iter()
//         .fold(dtm, |next_dtm: DateTime<Tz>, _| -> DateTime<Tz> {
//             next_dtm.add(Days::new(7))
//         });

//     next_dtm.add(Days::new(bal as u64))
// }

// fn adjust_month_last_day<Tz: TimeZone>(dtm: DateTime<Tz>) -> DateTime<Tz> {
//     if dtm.month() < 12 {
//         dtm.with_month(dtm.month())
//             .unwrap()
//             .with_day(1)
//             .unwrap()
//             .sub(Days::new(1))
//     } else {
//         dtm.with_year(dtm.year() + 1)
//             .unwrap()
//             .with_month(1)
//             .unwrap()
//             .with_day(1)
//             .unwrap()
//             .sub(Days::new(1))
//     }
// }
