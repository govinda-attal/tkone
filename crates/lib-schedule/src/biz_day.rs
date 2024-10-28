use crate::prelude::*;
use chrono::{DateTime, TimeZone};

pub trait BizDayProcessor {
    fn is_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>) -> Result<bool>;
    fn next_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, count: u8) -> Result<DateTime<Tz>>;
    fn prev_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, count: u8) -> Result<DateTime<Tz>>;
}

pub struct WeekendSkipper {}

impl BizDayProcessor for WeekendSkipper {
    fn is_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>) -> Result<bool> {
        Ok(true)
    }

    fn next_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, count: u8) -> Result<DateTime<Tz>> {
        Ok(dtm.clone())
    }

    fn prev_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, count: u8) -> Result<DateTime<Tz>> {
        Ok(dtm.clone())
    }
}
