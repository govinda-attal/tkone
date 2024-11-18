use std::fmt::Debug;

use crate::{prelude::*, utils::DateLikeUtils};
use chrono::{Datelike, Duration, NaiveDateTime};

pub trait BizDayProcessor: Debug + Clone + Send + Sync + 'static {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> Result<bool>;
    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime>;
    fn add(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;
    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> Result<NaiveDateTime>;
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum Direction {
    #[default]
    Nearest,
    Prev,
    Next,
}

#[derive(Debug, Clone, Default)]
pub struct WeekendSkipper {}
unsafe impl Send for WeekendSkipper {}
unsafe impl Sync for WeekendSkipper {}

impl WeekendSkipper {
    pub fn new() -> Self {
        Self {}
    }

    fn nearest_biz_day(&self, dtm: &NaiveDateTime) -> Result<NaiveDateTime> {
        if self.is_biz_day(dtm)? {
            return Ok(dtm.clone());
        }

        let mut current_date = dtm.clone();
        let step = Duration::days(1);
        if dtm.day() == 1 {
            while current_date.weekday() == chrono::Weekday::Sat
                || current_date.weekday() == chrono::Weekday::Sun
            {
                current_date = current_date + step;
            }
            return Ok(current_date);
        }

        let last_day_month = dtm.to_last_day_of_month();
        if dtm.day() == last_day_month.day() {
            while current_date.weekday() == chrono::Weekday::Sat
                || current_date.weekday() == chrono::Weekday::Sun
            {
                current_date = current_date - step;
            }
            return Ok(current_date);
        }

        if dtm.weekday() == chrono::Weekday::Sat {
            return Ok(current_date - step);
        }

        return Ok(current_date + step);
    }
}

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

    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> Result<NaiveDateTime> {
        match direction {
            Direction::Nearest => self.nearest_biz_day(dtm),
            Direction::Prev => self.sub(dtm, 1),
            Direction::Next => self.add(dtm, 1),
        }
    }
}
