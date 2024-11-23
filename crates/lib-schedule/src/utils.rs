use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveDateTime, TimeZone, Weekday};

use crate::{prelude::*, NextResult};

impl<Tz: TimeZone> From<W<(Tz, NaiveDateTime)>> for DateTime<Tz> {
    fn from(W((tz, dtm)): W<(Tz, NaiveDateTime)>) -> Self {
        match tz.from_local_datetime(&dtm) {
            chrono::LocalResult::None => {
                // the positive timezone transition (spring forward)
                tz.from_local_datetime(&(dtm.clone() + Duration::hours(1)))
                    .latest()
                    .unwrap()
            }
            chrono::LocalResult::Single(dtm) => dtm,
            chrono::LocalResult::Ambiguous(_, _) => {
                // the negative timezone transition (fallback)
                tz.from_local_datetime(&dtm).earliest().unwrap()
            }
        }
    }
}

impl<Tz: TimeZone> From<W<(Tz, NextResult<NaiveDateTime>)>> for NextResult<DateTime<Tz>> {
    fn from(W((tz, next)): W<(Tz, NextResult<NaiveDateTime>)>) -> Self {
        match next {
            NextResult::Single(dtm) => NextResult::Single(DateTime::<Tz>::from(W((tz, dtm)))),
            NextResult::AdjustedEarlier(actual, adjusted) => NextResult::AdjustedEarlier(
                DateTime::<Tz>::from(W((tz.clone(), actual))),
                DateTime::<Tz>::from(W((tz, adjusted))),
            ),
            NextResult::AdjustedLater(actual, adjusted) => NextResult::AdjustedLater(
                DateTime::<Tz>::from(W((tz.clone(), actual))),
                DateTime::<Tz>::from(W((tz, adjusted))),
            ),
        }
    }
}

pub trait DateLikeUtils: Datelike {
    fn to_last_day_of_month(&self) -> Self;
    fn to_first_day_of_month(&self) -> Self;
    fn to_first_day_of_next_month(&self) -> Self;
    fn to_weekday(&self, weekday: &Weekday) -> Self;
    fn to_weekday_ocurring(&self, weekday: &Weekday, occurence: u8) -> Self;
    fn to_months_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self>;
    fn to_prev_weekday(&self, weekday: &Weekday) -> Self;
    fn to_months_last_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self>;
}

impl DateLikeUtils for NaiveDate {
    fn to_last_day_of_month(&self) -> Self {
        NaiveDate::from_ymd_opt(self.year(), self.month() + 1, 1)
            .unwrap_or(NaiveDate::from_ymd_opt(self.year() + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap()
    }

    fn to_first_day_of_month(&self) -> Self {
        dbg!(self);
        dbg!(NaiveDate::from_ymd_opt(self.year(), self.month(), 1).unwrap());
        NaiveDate::from_ymd_opt(self.year(), self.month(), 1).unwrap()
    }

    fn to_first_day_of_next_month(&self) -> Self {
        NaiveDate::from_ymd_opt(self.year(), self.month() + 1, 1)
            .unwrap_or(NaiveDate::from_ymd_opt(self.year() + 1, 1, 1).unwrap())
    }

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        let mut date = self.clone();
        while date.weekday() != *weekday {
            date = date.succ_opt().unwrap();
        }
        date
    }

    fn to_weekday_ocurring(&self, weekday: &Weekday, occurence: u8) -> Self {
        let date = self.succ_opt().unwrap().to_weekday(weekday);
        date + Duration::days(7 * (occurence - 1) as i64)
    }

    fn to_months_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        let mut date = self.to_first_day_of_month().to_weekday(weekday);
        for _ in 1..occurence {
            date = date + Duration::days(7);
        }
        if self.month() == date.month() {
            Some(date)
        } else {
            None
        }
    }

    fn to_prev_weekday(&self, weekday: &Weekday) -> Self {
        let mut date = self.clone();
        while date.weekday() != *weekday {
            date = date.pred_opt().unwrap();
        }
        date
    }

    fn to_months_last_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        let mut date = self.to_last_day_of_month().to_prev_weekday(weekday);
        for _ in 1..occurence {
            date = date - Duration::days(7);
        }
        if self.month() == date.month() {
            Some(date)
        } else {
            None
        }
    }
}

impl DateLikeUtils for NaiveDateTime {
    fn to_last_day_of_month(&self) -> Self {
        NaiveDateTime::new(self.date().to_last_day_of_month(), self.time())
    }

    fn to_first_day_of_month(&self) -> Self {
        NaiveDateTime::new(self.date().to_first_day_of_month(), self.time())
    }

    fn to_first_day_of_next_month(&self) -> Self {
        NaiveDateTime::new(self.date().to_first_day_of_next_month(), self.time())
    }

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        NaiveDateTime::new(self.date().to_weekday(weekday), self.time())
    }

    fn to_weekday_ocurring(&self, weekday: &Weekday, occurence: u8) -> Self {
        NaiveDateTime::new(
            self.date().to_weekday_ocurring(weekday, occurence),
            self.time(),
        )
    }

    fn to_months_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        self.date()
            .to_months_weekday(weekday, occurence)
            .map(|date| NaiveDateTime::new(date, self.time()))
    }

    fn to_prev_weekday(&self, weekday: &Weekday) -> Self {
        NaiveDateTime::new(self.date().to_prev_weekday(weekday), self.time())
    }

    fn to_months_last_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        self.date()
            .to_months_last_weekday(weekday, occurence)
            .map(|date| NaiveDateTime::new(date, self.time()))
    }
}

impl<Tz: TimeZone> DateLikeUtils for DateTime<Tz> {
    fn to_last_day_of_month(&self) -> Self {
        DateTime::<Tz>::from(W((
            self.timezone(),
            self.naive_local().to_last_day_of_month(),
        )))
    }

    fn to_first_day_of_month(&self) -> Self {
        DateTime::<Tz>::from(W((
            self.timezone(),
            self.naive_local().to_first_day_of_month(),
        )))
    }

    fn to_first_day_of_next_month(&self) -> Self {
        DateTime::<Tz>::from(W((
            self.timezone(),
            self.naive_local().to_first_day_of_next_month(),
        )))
    }

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        DateTime::<Tz>::from(W((self.timezone(), self.naive_local().to_weekday(weekday))))
    }

    fn to_weekday_ocurring(&self, weekday: &Weekday, occurence: u8) -> Self {
        DateTime::<Tz>::from(W((
            self.timezone(),
            self.naive_local().to_weekday_ocurring(weekday, occurence),
        )))
    }

    fn to_months_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        self.naive_local()
            .to_months_weekday(weekday, occurence)
            .map(|date| DateTime::<Tz>::from(W((self.timezone(), date))))
    }

    fn to_prev_weekday(&self, weekday: &Weekday) -> Self {
        DateTime::<Tz>::from(W((
            self.timezone(),
            self.naive_local().to_prev_weekday(weekday),
        )))
    }

    fn to_months_last_weekday(&self, weekday: &Weekday, occurence: u8) -> Option<Self> {
        self.naive_local()
            .to_months_last_weekday(weekday, occurence)
            .map(|date| DateTime::<Tz>::from(W((self.timezone(), date))))
    }
}

pub fn naive_date_with_last_day_of_month_in_year(year: i32, month: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(year, month + 1, 1)
        .unwrap_or(NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap())
        .pred_opt()
        .unwrap()
}
