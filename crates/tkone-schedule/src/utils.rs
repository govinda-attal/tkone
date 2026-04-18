use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Weekday};

use crate::{prelude::*, DstPolicy, NextResult};

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

/// Convert a `NaiveDateTime` to `DateTime<Tz>` according to `policy`.
///
/// - `Adjust` (default): silently resolves gaps (spring-forward) and overlaps
///   (fall-back) using the same heuristics as the internal `From<W<…>>` impl.
/// - `Strict`: returns [`Error::AmbiguousLocalTime`] for any non-unique mapping.
pub(crate) fn resolve_local<Tz: TimeZone>(
    tz: &Tz,
    dtm: NaiveDateTime,
    policy: DstPolicy,
) -> Result<DateTime<Tz>> {
    match tz.from_local_datetime(&dtm) {
        LocalResult::Single(dt) => Ok(dt),
        LocalResult::None => match policy {
            DstPolicy::Adjust => Ok(tz
                .from_local_datetime(&(dtm + Duration::hours(1)))
                .latest()
                .unwrap()),
            DstPolicy::Strict => Err(Error::AmbiguousLocalTime(format!(
                "{dtm} does not exist in timezone (DST spring-forward gap)"
            ))),
        },
        LocalResult::Ambiguous(_, _) => match policy {
            DstPolicy::Adjust => Ok(tz.from_local_datetime(&dtm).earliest().unwrap()),
            DstPolicy::Strict => Err(Error::AmbiguousLocalTime(format!(
                "{dtm} is ambiguous in timezone (DST fall-back overlap)"
            ))),
        },
    }
}

/// Convert a `NextResult<NaiveDateTime>` to `NextResult<DateTime<Tz>>`,
/// applying `policy` to each embedded naive datetime.
pub(crate) fn next_result_to_tz<Tz: TimeZone>(
    tz: &Tz,
    next: NextResult<NaiveDateTime>,
    policy: DstPolicy,
) -> Result<NextResult<DateTime<Tz>>> {
    match next {
        NextResult::Single(dtm) => Ok(NextResult::Single(resolve_local(tz, dtm, policy)?)),
        NextResult::AdjustedEarlier(actual, adjusted) => Ok(NextResult::AdjustedEarlier(
            resolve_local(tz, actual, policy)?,
            resolve_local(tz, adjusted, policy)?,
        )),
        NextResult::AdjustedLater(actual, adjusted) => Ok(NextResult::AdjustedLater(
            resolve_local(tz, actual, policy)?,
            resolve_local(tz, adjusted, policy)?,
        )),
    }
}

pub trait DateLikeUtils: Datelike {
    fn to_last_day_of_month(&self) -> Self;
    fn to_first_day_of_month(&self) -> Self;
    fn to_weekday(&self, weekday: &Weekday) -> Self;
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
        NaiveDate::from_ymd_opt(self.year(), self.month(), 1).unwrap()
    }

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        let mut date = self.clone();
        while date.weekday() != *weekday {
            date = date.succ_opt().unwrap();
        }
        date
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

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        NaiveDateTime::new(self.date().to_weekday(weekday), self.time())
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

    fn to_weekday(&self, weekday: &Weekday) -> Self {
        DateTime::<Tz>::from(W((self.timezone(), self.naive_local().to_weekday(weekday))))
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

#[derive(Debug, Clone, Copy, Eq, Hash)]
pub struct WeekdayStartingMonday(pub Weekday);

impl PartialEq for WeekdayStartingMonday {
    fn eq(&self, Self(other): &Self) -> bool {
        self.0.num_days_from_monday() == other.num_days_from_monday()
    }
}

impl Ord for WeekdayStartingMonday {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare based on 0 (Mon) to 6 (Sun)
        self.0
            .num_days_from_monday()
            .cmp(&other.0.num_days_from_monday())
    }
}

impl PartialOrd for WeekdayStartingMonday {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
