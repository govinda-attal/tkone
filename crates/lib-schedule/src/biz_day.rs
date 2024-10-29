use crate::prelude::*;
use chrono::{DateTime, Datelike, Duration, TimeZone};

pub trait BizDayProcessor {
    fn is_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>) -> Result<bool>;
    fn add<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, num: u8) -> Result<DateTime<Tz>>;
    fn sub<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, num: u8) -> Result<DateTime<Tz>>;
}

#[derive(Debug, Clone, Default)]
pub struct WeekendSkipper {}

impl BizDayProcessor for WeekendSkipper {
    fn is_biz_day<Tz: TimeZone>(&self, dtm: &DateTime<Tz>) -> Result<bool> {
        let weekday = dtm.weekday();
        Ok(weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun)
    }

    fn add<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, num: u8) -> Result<DateTime<Tz>> {
        let mut days_added = 0;
        let mut current_date = dtm.clone();

        while days_added < num {
            current_date = current_date + Duration::days(1);
            if self.is_biz_day(&current_date)? {
                days_added += 1;
            }
        }

        Ok(current_date)
    }

    fn sub<Tz: TimeZone>(&self, dtm: &DateTime<Tz>, num: u8) -> Result<DateTime<Tz>> {
        let mut days_subtracted = 0;
        let mut current_date = dtm.clone();

        while days_subtracted < num {
            current_date = current_date - Duration::days(1);
            if self.is_biz_day(&current_date)? {
                days_subtracted += 1;
            }
        }

        Ok(current_date)
    }
}
