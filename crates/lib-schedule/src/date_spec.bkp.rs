use std::{
    collections::HashSet,
    ops::{Add, Sub},
    str::FromStr,
};

use chrono::{DateTime, Datelike, Days, Months, TimeZone};
use regex::Regex;

use crate::error::{Error, Result};

#[derive(Debug, PartialEq, Eq)]
pub enum DayKind {
    LastDay(Option<u8>),
    Num(u8),
}

#[derive(Debug, PartialEq, Eq)]
pub enum BusinessDayStep {
    Prev,
    Next,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Cycle {
    Year(u8),
    Month(u8),
    Day(u8),
    BusinessDay(u8),
}

pub const DATE_SPEC_EXP: &str =
    r"(\d+Y):?(\d+M):?(\d+[BD]):?(0[1-9]|[12][0-8]|29L|3[01]L|3[01]|[L])?(P|N)?";
pub const CYCLE_EXP: &str = r"(?:(?<num>\d+)(?<type>[YMBD]))";
pub const DAY_KIND_EXP: &str = r"(?:(?<num>\d+)?(?<last>L))?";
pub const BUSINESS_DAY_STEP_EXP: &str = r"[PN]?";

#[derive(Default, Debug)]
pub struct DateSpec {
    pub intervals: HashSet<Cycle>,
    pub day_kind: Option<DayKind>,
    pub business_day_step: Option<BusinessDayStep>,
}

impl FromStr for Cycle {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let cycle_re = Regex::new(CYCLE_EXP).unwrap();
        let caps = cycle_re
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;
        let num = caps
            .name("num")
            .unwrap()
            .as_str()
            .parse::<u8>()
            .or(Err(Error::ParseError("Invalid time spec")))?;
        let cycle_type = caps.name("type").unwrap();
        let cycle = match cycle_type.as_str() {
            "Y" => Cycle::Year(num),
            "M" => Cycle::Month(num),
            "D" => Cycle::Day(num),
            "B" => Cycle::BusinessDay(num),
            _ => Err(Error::ParseError("Invalid time spec"))?,
        };
        Ok(cycle)
    }
}

impl DateSpec {
    pub fn next_dtm<Tz: TimeZone>(&self, dtm: DateTime<Tz>) -> Result<DateTime<Tz>> {
        let mut next_dtm =
            self.intervals
                .iter()
                .fold(dtm, |next_dtm: DateTime<Tz>, c| -> DateTime<Tz> {
                    match c {
                        Cycle::Year(y) => next_dtm.add(Months::new(*y as u32 * 12)),
                        Cycle::Month(m) => next_dtm.add(Months::new(*m as u32)),
                        Cycle::Day(d) => next_dtm.add(Days::new(*d as u64)),
                        Cycle::BusinessDay(bd) => add_business_days(*bd, next_dtm),
                    }
                });

        if let Some(kind) = &self.day_kind {
            next_dtm = match kind {
                DayKind::Num(d) => next_dtm.with_day(*d as u32).unwrap(),
                DayKind::LastDay(d) => {
                    if d.is_none() {
                        adjust_month_last_day(next_dtm)
                    } else {
                        let d = d.unwrap() as u32;
                        let adjusted_dtm = adjust_month_last_day(next_dtm);
                        if adjusted_dtm.day().ge(&d) {
                            adjusted_dtm.with_day(d).unwrap()
                        } else {
                            adjusted_dtm
                        }
                    }
                }
            };
        };

        if let Some(step) = &self.business_day_step {
            next_dtm = match step {
                BusinessDayStep::Prev => {
                    let bck = match next_dtm.weekday() {
                        chrono::Weekday::Sat => 1,
                        chrono::Weekday::Sun => 2,
                        _ => 0,
                    };
                    next_dtm.sub(Days::new(bck))
                }
                BusinessDayStep::Next => {
                    let fwd_days = match next_dtm.weekday() {
                        chrono::Weekday::Sat => 2,
                        chrono::Weekday::Sun => 1,
                        _ => 0,
                    };
                    next_dtm.add(Days::new(fwd_days))
                }
            }
        }
        Ok(next_dtm)
    }
}

impl FromStr for DayKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let daykind_re = Regex::new(DAY_KIND_EXP).unwrap();

        let caps = daykind_re
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;

        let kind = if let (Some(num), Some(_)) = (caps.name("num"), caps.name("last")) {
            DayKind::LastDay(Some(num.as_str().parse::<u8>().unwrap()))
        } else if let Some(num) = caps.name("num") {
            DayKind::Num(num.as_str().parse::<u8>().unwrap())
        } else {
            DayKind::LastDay(None)
        };

        Ok(kind)
    }
}

impl FromStr for BusinessDayStep {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let re = Regex::new(BUSINESS_DAY_STEP_EXP).unwrap();

        let caps = re
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;

        let step = if caps[0].eq("P") {
            BusinessDayStep::Prev
        } else {
            BusinessDayStep::Next
        };

        Ok(step)
    }
}

impl FromStr for DateSpec {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self> {
        let spec_re = Regex::new(DATE_SPEC_EXP).unwrap();
        let caps = &spec_re
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;
        let intervals = caps
            .iter()
            .skip(1)
            .take(3)
            .flatten()
            .map(|m| Cycle::from_str(m.as_str()))
            .flatten()
            .collect::<HashSet<Cycle>>();

        let day_kind = caps
            .iter()
            .nth(4)
            .flatten()
            .map(|m| DayKind::from_str(m.as_str()).unwrap());

        let business_day_step = caps
            .iter()
            .nth(5)
            .flatten()
            .map(|m| BusinessDayStep::from_str(m.as_str()).unwrap());

        dbg!(&intervals, &day_kind, &business_day_step);
        Ok(DateSpec {
            intervals,
            day_kind,
            business_day_step,
        })
    }
}

fn add_business_days<Tz: TimeZone>(bd: u8, dtm: DateTime<Tz>) -> DateTime<Tz> {
    let times = bd / 5;
    let bal = bd % 5;

    let next_dtm = (0..times)
        .into_iter()
        .into_iter()
        .fold(dtm, |next_dtm: DateTime<Tz>, _| -> DateTime<Tz> {
            next_dtm.add(Days::new(7))
        });

    next_dtm.add(Days::new(bal as u64))
}

fn adjust_month_last_day<Tz: TimeZone>(dtm: DateTime<Tz>) -> DateTime<Tz> {
    if dtm.month() < 12 {
        dtm.with_month(dtm.month())
            .unwrap()
            .with_day(1)
            .unwrap()
            .sub(Days::new(1))
    } else {
        dtm.with_year(dtm.year() + 1)
            .unwrap()
            .with_month(1)
            .unwrap()
            .with_day(1)
            .unwrap()
            .sub(Days::new(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;
    use chrono_tz::Australia::Sydney;

    #[test]
    fn test_one() {
        let spec = DateSpec::from_str("3Y:4M:7D:31LN").unwrap();
        let now = Local::now().with_timezone(&Sydney);
        println!("{:?}", now);
        println!("{:?}", spec.next_dtm(now).unwrap());
    }
}
