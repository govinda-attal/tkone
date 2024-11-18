use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Timelike};

use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::prelude::*;

/// ## SpecIterator
/// An iterator for generating recurring timezone aware datetimes as per time based specifications.
/// ### Examples
///
/// ```rust
/// use lib_schedule::time::SpecIterator;
/// use chrono::{DateTime, TimeZone, Utc, Duration};
/// use fallible_iterator::FallibleIterator;
///
/// let start = Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap();
/// let iter = SpecIterator::new_with_start("1H:00:00", start.clone()).unwrap();
/// let occurrences = iter.take(3).collect::<Vec<DateTime<_>>>().unwrap();
///        
/// assert_eq!(occurrences, vec![
///     start,
///     start + Duration::hours(1),
///     start + Duration::hours(2),
/// ]);
///
/// ```
#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator,
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    pub fn new(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new(spec, start.naive_local())?,
        })
    }

    pub fn new_with_start(spec: &str, start: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_start(spec, start.naive_local())?,
        })
    }

    pub fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                spec,
                start.naive_local(),
                end.naive_local(),
            )?,
        })
    }

    pub fn new_with_end_spec(spec: &str, start: DateTime<Tz>, end_spec: &str) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
                spec,
                start.naive_local(),
                end_spec,
            )?,
        })
    }

    pub(crate) fn update_cursor(&mut self, dtm: DateTime<Tz>) {
        self.naive_spec_iter.update_cursor(dtm.naive_local());
    }
}

/// ## NaiveSpecIterator
/// An iterator for generating recurring naive datetimes as per time based specifications.
/// ### Examples
///
/// ```rust
/// use lib_schedule::time::NaiveSpecIterator;
/// use chrono::{NaiveDate, NaiveDateTime, Duration};
/// use fallible_iterator::FallibleIterator;
///
/// let start = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap().and_hms_opt(10, 0, 0).unwrap();
/// let iter = NaiveSpecIterator::new_with_start("1H:00:00", start.clone()).unwrap();
/// let occurrences = iter.take(3).collect::<Vec<NaiveDateTime>>().unwrap();
///        
/// assert_eq!(occurrences, vec![start, start + Duration::hours(1), start + Duration::hours(2)]);
///
/// ```
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator {
    spec: Spec,
    end: Option<NaiveDateTime>,
    dtm: NaiveDateTime,
    start: Option<NaiveDateTime>,
    index: usize,
}

impl NaiveSpecIterator {
    pub fn new(spec: &str, dtm: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm,
            spec,
            end: None,
            start: None,
            index: 0,
        })
    }

    pub fn new_with_start(spec: &str, start: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start.clone(),
            spec,
            end: None,
            start: Some(start),
            index: 0,
        })
    }

    pub fn new_with_end(spec: &str, start: NaiveDateTime, end: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            dtm: start.clone(),
            end: Some(end),
            spec,
            start: Some(start),
            index: 0,
        })
    }

    pub fn new_with_end_spec(spec: &str, start: NaiveDateTime, end_spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        let end = Self::new(end_spec, start.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            end: Some(end),
            spec,
            dtm: start.clone(),
            start: Some(start),
            index: 0,
        })
    }

    pub(crate) fn update_cursor(&mut self, dtm: NaiveDateTime) {
        self.dtm = dtm;
    }
}

/// Advances the iterator and returns the next `NaiveDateTime` value or `None` if the end is reached.
///
/// # Returns
///
/// - `Ok(Some(NaiveDateTime))` if the next date-time value is successfully computed.
/// - `Ok(None)` if the iterator has reached the end date-time specified by `self.end`.
/// - `Err(Error)` if an error occurs during the computation.
///
/// The function computes next naive datetime based on the specified cycles for seconds, minutes, and hours.
/// - If `Cycle::At` is specified, it sets the corresponding field to the specified value.
/// - If `Cycle::Every` is specified, it increments the corresponding field by the specified duration.
///
/// ### Example 1:
/// If the current date-time is 2024-11-01 12:30:45 and the specification is HH:MM:15S then it adds seconds every iteration,
/// the `next` method will produce the following sequence:
///
/// 1. 2024-11-01 12:31:00
/// 2. 2024-11-01 12:31:15
/// 3. 2024-11-01 12:31:30
/// 4. ...
///
/// This continues until the end condition is met, if specified.
///
/// ### Example 2:
/// If the current date-time is 2024-11-01 00:30:00 and the specification is 1H:00:00 then it adds hours every iteration with minutes and seconds set to 00,
/// the `next` method will produce the following sequence:
///
/// 1. 2024-11-01 13:00:00
/// 2. 2024-11-01 14:30:45
/// 3. 2024-11-01 15:30:45
/// 4. ...
///
/// This continues until the end condition is met, if specified.
impl FallibleIterator for NaiveSpecIterator {
    type Item = NaiveDateTime;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if let Some(end) = &self.end {
            if &self.dtm >= end {
                return Ok(None);
            }
        }

        if self.index == 0 {
            if let Some(start) = &self.start {
                if &self.dtm <= start {
                    self.dtm = start.clone();
                    self.index += 1;
                    return Ok(Some(start.clone()));
                }
            }
        }

        let next = self.dtm.clone();

        let next = match &self.spec.seconds {
            Cycle::At(s) => next.with_second(*s as u32).unwrap(),
            Cycle::Every(s) => next + Duration::seconds(*s as i64),
            _ => next,
        };

        let next = match &self.spec.minutes {
            Cycle::At(m) => next.with_minute(*m as u32).unwrap(),
            Cycle::Every(m) => next + Duration::minutes(*m as i64),
            _ => next,
        };

        let next = match &self.spec.hours {
            Cycle::At(h) => next.with_hour(*h as u32).unwrap(),
            Cycle::Every(h) => next + Duration::hours(*h as i64),
            _ => next,
        };

        self.dtm = next;

        Ok(Some(self.dtm.clone()))
    }
}

impl<Tz: TimeZone> FallibleIterator for SpecIterator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let item = self.naive_spec_iter.next()?;
        let Some(next) = item else {
            return Ok(None);
        };
        Ok(Some(Self::Item::from(W((self.tz.clone(), next.clone())))))
    }
}

#[cfg(test)]
mod tests {

    use chrono::Utc;
    use chrono_tz::{America::New_York, Europe::London};

    use super::*;

    #[test]
    fn test_time_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm.to_rfc3339());
        let dt = DateTime::parse_from_rfc3339("2023-03-11T23:00:00-05:00").unwrap();
        let spec_ter = SpecIterator::new("HH:30M:00", dt.with_timezone(&New_York)).unwrap();
        dbg!(spec_ter.take(6).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_month_end() {
        // US Eastern Time (EST/EDT)
        let london = London;
        // Before DST starts (Standard Time)
        let dtm = london.with_ymd_and_hms(2021, 10, 31, 00, 30, 0).unwrap();
        dbg!(&dtm);
        // let dt = DateTime::parse_from_rfc3339("2023-03-11T23:00:00-05:00").unwrap();
        let spec_ter = SpecIterator::new("1H:MM:00", dtm).unwrap();
        dbg!(spec_ter.take(5).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_end_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);

        let spec_iter = SpecIterator::new_with_end_spec("3H:00:00", dtm, "15H:00:00").unwrap();

        let tmp = spec_iter.collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(tmp);
    }

    #[test]
    fn test_time_spec_with_utc() {
        let start = Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap();
        let iter = SpecIterator::new_with_start("1H:00:00", start).unwrap();
        let occurrences = iter.take(3).collect::<Vec<DateTime<_>>>().unwrap();

        assert_eq!(
            occurrences,
            vec![
                Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 3, 31, 11, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2024, 3, 31, 12, 0, 0).unwrap(),
            ]
        );
    }
}
