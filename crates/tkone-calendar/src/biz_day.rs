//! A [`Calendar`] as a [`BizDayProcessor`]: schedules count, find and move
//! to its business days.

use chrono::{Datelike, Days, NaiveDate, NaiveDateTime};
use tkone_schedule::biz_day::{BizDayProcessor, Direction};

use crate::Calendar;

impl Calendar {
    /// The business day `n` business days after (`forward`) or before `date`,
    /// not counting `date` itself.
    fn step(&self, date: NaiveDate, n: u32, forward: bool) -> crate::Result<NaiveDate> {
        let mut day = date;
        let mut left = n;
        while left > 0 {
            let next = if forward { day.checked_add_days(Days::new(1)) } else { day.checked_sub_days(Days::new(1)) };
            // Past chrono's range is past the calendar's too.
            day = next.unwrap_or(if forward { NaiveDate::MAX } else { NaiveDate::MIN });
            if self.is_business_day(day)? {
                left -= 1;
            }
        }
        Ok(day)
    }

    /// The closest business day to `date`. On a tie, the later one, unless it
    /// is in the next month and the earlier one is not.
    fn nearest(&self, date: NaiveDate) -> crate::Result<NaiveDate> {
        if self.is_business_day(date)? {
            return Ok(date);
        }
        let mut k = 1;
        loop {
            let later = date.checked_add_days(Days::new(k)).unwrap_or(NaiveDate::MAX);
            let earlier = date.checked_sub_days(Days::new(k)).unwrap_or(NaiveDate::MIN);
            match (self.is_business_day(earlier)?, self.is_business_day(later)?) {
                (true, true) if later.month() != date.month() && earlier.month() == date.month() => return Ok(earlier),
                (_, true) => return Ok(later),
                (true, false) => return Ok(earlier),
                (false, false) => k += 1,
            }
        }
    }
}

impl BizDayProcessor for Calendar {
    fn is_biz_day(&self, dtm: &NaiveDateTime) -> tkone_schedule::Result<bool> {
        Ok(self.is_business_day(dtm.date())?)
    }

    fn find_biz_day(&self, dtm: &NaiveDateTime, direction: Direction) -> tkone_schedule::Result<NaiveDateTime> {
        let date = match direction {
            Direction::Next => self.step(dtm.date(), 1, true)?,
            Direction::Prev => self.step(dtm.date(), 1, false)?,
            Direction::Nearest => self.nearest(dtm.date())?,
        };
        Ok(date.and_time(dtm.time()))
    }

    fn add(&self, dtm: &NaiveDateTime, num: u32) -> tkone_schedule::Result<NaiveDateTime> {
        Ok(self.step(dtm.date(), num, true)?.and_time(dtm.time()))
    }

    fn sub(&self, dtm: &NaiveDateTime, num: u32) -> tkone_schedule::Result<NaiveDateTime> {
        Ok(self.step(dtm.date(), num, false)?.and_time(dtm.time()))
    }

    /// The calendar's code (`GBLO`, `GBLO+EUTA`).
    fn code(&self) -> Option<String> {
        Some(Calendar::code(self).to_string())
    }

    /// The calendar's time zone, so a datetime schedule fails rather than
    /// judge a business day on another date than the calendar's place. A
    /// joint calendar of places in different zones has none.
    fn time_zone(&self) -> Option<chrono_tz::Tz> {
        self.zone().and_then(|z| z.parse().ok())
    }
}
