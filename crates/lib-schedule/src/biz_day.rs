use crate::prelude::*;
use chrono::{Datelike, Duration, NaiveDateTime};

pub trait BizDayProcessor {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool>;
    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;
    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;
}

#[derive(Debug, Clone, Default)]
pub struct WeekendSkipper {}

impl BizDayProcessor for WeekendSkipper {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool> {
        let weekday = dtm.weekday();
        Ok(weekday != chrono::Weekday::Sat && weekday != chrono::Weekday::Sun)
    }

    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
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

    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime> {
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
