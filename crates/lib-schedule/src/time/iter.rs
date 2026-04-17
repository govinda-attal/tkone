use std::marker::PhantomData;

use chrono::{DateTime, Duration, NaiveDateTime, TimeZone, Timelike, Utc};

use fallible_iterator::FallibleIterator;

use super::spec::{Cycle, Spec};
use crate::utils::resolve_local;
use crate::{prelude::*, DstPolicy};

pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

/// Fluent, type-state builder for the time-only [`SpecIterator`] and
/// [`NaiveSpecIterator`].
///
/// # Construction variants
///
/// | Constructor | First result |
/// |-------------|-------------|
/// | `new(spec, tz)` | First occurrence after `Utc::now()` |
/// | `new_after(spec, dtm)` | First occurrence strictly **after** `dtm` |
/// | `new_with_start(spec, start)` | `start` itself is the first item |
///
/// After `new_with_start` you may optionally call:
/// - `.with_end(end)` — bound by an explicit datetime
/// - `.with_end_spec(end_spec)` — bound by another time spec
pub struct SpecIteratorBuilder<Tz: TimeZone, START, END, S> {
    timezone: Tz,
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    end: END,
    dst_policy: DstPolicy,
    marker_sealed: PhantomData<S>,
}

impl<Tz: TimeZone, START, END, S> SpecIteratorBuilder<Tz, START, END, S> {
    /// Override the DST resolution policy for this iterator.
    ///
    /// Defaults to [`DstPolicy::Adjust`]. Set to [`DstPolicy::Strict`] to
    /// receive [`Error::AmbiguousLocalTime`] instead of silent adjustment.
    pub fn with_dst_policy(mut self, policy: DstPolicy) -> Self {
        self.dst_policy = policy;
        self
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
    /// Create an iterator from `Utc::now()` in timezone `tz`.
    pub fn new(spec: &str, tz: Tz) -> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder::new_after(spec, Utc::now().with_timezone(&tz))
    }

    /// Create an iterator that produces occurrences strictly **after `dtm`**.
    pub fn new_after(
        spec: &str,
        dtm: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
        Self {
            timezone: dtm.timezone(),
            dtm,
            start: NoStart,
            spec: spec.to_string(),
            end: NoEnd,
            dst_policy: DstPolicy::default(),
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        SpecIterator::new_after(&self.spec, self.dtm, self.dst_policy)
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, NoEnd, NotSealed> {
    /// Create an iterator where `start` is the **first yielded item**.
    pub fn new_with_start(
        spec: &str,
        start: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, NoEnd, NotSealed> {
        let timezone = start.timezone();
        Self {
            timezone,
            dtm: start.clone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            end: NoEnd,
            dst_policy: DstPolicy::default(),
            marker_sealed: PhantomData,
        }
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, NoEnd, NotSealed> {
    /// Bound the iterator by an explicit end datetime.
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            timezone: self.timezone,
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            end: EndDateTime(end),
            dst_policy: self.dst_policy,
            marker_sealed: PhantomData,
        }
    }

    /// Bound the iterator by a second time spec.
    ///
    /// The end time is resolved as the first occurrence of `end_spec` starting
    /// from the iterator's start datetime.
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
        SpecIteratorBuilder {
            timezone: self.timezone,
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            end: EndSpec(end_spec.into()),
            dst_policy: self.dst_policy,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        SpecIterator::new_with_start(&self.spec, self.dtm, self.dst_policy)
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        SpecIterator::new_with_end(&self.spec, self.dtm, self.end.0, self.dst_policy)
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        SpecIterator::new_with_end_spec(&self.spec, self.dtm, &self.end.0, self.dst_policy)
    }
}

/// ## SpecIterator
/// An iterator for generating recurring timezone aware datetimes as per time based specifications.
/// ### Examples
///
/// ```rust
/// use lib_schedule::time::SpecIteratorBuilder;
/// use chrono::{DateTime, TimeZone, Utc, Duration};
/// use fallible_iterator::FallibleIterator;
///
/// let start = Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap();
/// let iter = SpecIteratorBuilder::new_with_start("1H:00:00", start.clone()).build().unwrap();
/// let occurrences = iter.take(3).collect::<Vec<DateTime<_>>>().unwrap();
///        
/// assert_eq!(occurrences, vec![
///     start,
///     start + Duration::hours(1),
///     start + Duration::hours(2),
/// ]);
///
/// ```
///
/// ### See Also
/// - [NaiveSpecIterator](crate::time::NaiveSpecIterator)
/// - [SpecIteratorBuilder](crate::time::SpecIteratorBuilder)
/// - [Spec](crate::time::Spec)
///
#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone> {
    tz: Tz,
    dst_policy: DstPolicy,
    naive_spec_iter: NaiveSpecIterator,
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    fn new_after(spec: &str, dtm: DateTime<Tz>, dst_policy: DstPolicy) -> Result<Self> {
        Ok(Self {
            tz: dtm.timezone(),
            dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_after(spec, dtm.naive_local())?,
        })
    }

    fn new_with_start(spec: &str, start: DateTime<Tz>, dst_policy: DstPolicy) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_start(spec, start.naive_local())?,
        })
    }

    fn new_with_end(spec: &str, start: DateTime<Tz>, end: DateTime<Tz>, dst_policy: DstPolicy) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                spec,
                start.naive_local(),
                end.naive_local(),
            )?,
        })
    }

    fn new_with_end_spec(spec: &str, start: DateTime<Tz>, end_spec: &str, dst_policy: DstPolicy) -> Result<Self> {
        Ok(Self {
            tz: start.timezone(),
            dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
                spec,
                start.naive_local(),
                end_spec,
            )?,
        })
    }

    #[allow(dead_code)]
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
    pub fn new_after(spec: &str, dtm: NaiveDateTime) -> Result<Self> {
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
        let end = Self::new_after(end_spec, start.clone())?
            .next()?
            .ok_or(Error::InvalidEndSpec)?;
        Ok(Self {
            end: Some(end),
            spec,
            dtm: start.clone(),
            start: Some(start),
            index: 0,
        })
    }

    #[allow(dead_code)]
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

        // When no explicit Every cadence exists, the finest ForEach component
        // acts as Every(1) for its own unit (seconds > minutes > hours).
        // Coarser ForEach components carry the value forward unchanged.
        // AsIs is a true no-op — it always carries the current value.
        let has_any_every = matches!(self.spec.seconds, Cycle::Every(_))
            || matches!(self.spec.minutes, Cycle::Every(_))
            || matches!(self.spec.hours, Cycle::Every(_));
        let seconds_is_foreach = matches!(self.spec.seconds, Cycle::ForEach);
        let minutes_is_foreach = matches!(self.spec.minutes, Cycle::ForEach);

        let next = match &self.spec.seconds {
            Cycle::At(s) => next.with_second(*s as u32).unwrap(),
            Cycle::Every(s) => next + Duration::seconds(*s as i64),
            Cycle::ForEach if !has_any_every => next + Duration::seconds(1),
            Cycle::ForEach | Cycle::AsIs => next,
        };

        let next = match &self.spec.minutes {
            Cycle::At(m) => next.with_minute(*m as u32).unwrap(),
            Cycle::Every(m) => next + Duration::minutes(*m as i64),
            Cycle::ForEach if !has_any_every && !seconds_is_foreach => next + Duration::minutes(1),
            Cycle::ForEach | Cycle::AsIs => next,
        };

        let next = match &self.spec.hours {
            Cycle::At(h) => next.with_hour(*h as u32).unwrap(),
            Cycle::Every(h) => next + Duration::hours(*h as i64),
            Cycle::ForEach if !has_any_every && !seconds_is_foreach && !minutes_is_foreach => {
                next + Duration::hours(1)
            }
            Cycle::ForEach | Cycle::AsIs => next,
        };

        // No-progress guard: all-At and all-AsIs specs produce next == self.dtm,
        // which would loop forever without this check.
        if next <= self.dtm {
            return Ok(None);
        }

        if let Some(end) = &self.end {
            if &next > end {
                self.dtm = end.clone();
                self.index += 1;
                return Ok(Some(end.clone()));
            }
        };

        self.dtm = next;
        self.index += 1;
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
        Ok(Some(resolve_local(&self.tz, next, self.dst_policy)?))
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
        let spec_ter = SpecIterator::new_after("HH:30M:00", dt, DstPolicy::default()).unwrap();
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
        let spec_ter = SpecIterator::new_after("1H:MM:00", dtm, DstPolicy::default()).unwrap();
        dbg!(spec_ter.take(5).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_end_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);

        let spec_iter = SpecIterator::new_with_end_spec("3H:00:00", dtm, "15H:00:00", DstPolicy::default()).unwrap();

        let tmp = spec_iter.collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(tmp);
    }

    #[test]
    fn test_time_spec_with_utc() {
        let start = Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap();
        let iter = SpecIterator::new_with_start("1H:00:00", start, DstPolicy::default()).unwrap();
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

    // HH:MM:SS — all ForEach → every second (finest ForEach drives)
    #[test]
    fn test_foreach_seconds_drives() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let iter = SpecIterator::new_with_start("HH:MM:SS", start, DstPolicy::default()).unwrap();
        let results = iter.take(4).collect::<Vec<DateTime<_>>>().unwrap();
        assert_eq!(
            results,
            vec![
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 1).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 2).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 3).unwrap(),
            ]
        );
    }

    // HH:MM:00 — ForEach H, ForEach M, At(0) S → every minute at :00 second
    #[test]
    fn test_foreach_minutes_drives() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let iter = SpecIterator::new_with_start("HH:MM:00", start, DstPolicy::default()).unwrap();
        let results = iter.take(4).collect::<Vec<DateTime<_>>>().unwrap();
        assert_eq!(
            results,
            vec![
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 1, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 2, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 3, 0).unwrap(),
            ]
        );
    }

    // HH:00:00 — ForEach H, At(0) M, At(0) S → every hour at :00:00
    #[test]
    fn test_foreach_hours_drives() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap();
        let iter = SpecIterator::new_with_start("HH:00:00", start, DstPolicy::default()).unwrap();
        let results = iter.take(4).collect::<Vec<DateTime<_>>>().unwrap();
        assert_eq!(
            results,
            vec![
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 10, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 11, 0, 0).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 12, 0, 0).unwrap(),
            ]
        );
    }

    // 1H:MM:SS — Every H with ForEach M and S → H drives, M and S carry (unchanged)
    #[test]
    fn test_every_h_foreach_carry() {
        let start = Utc.with_ymd_and_hms(2025, 1, 1, 9, 22, 45).unwrap();
        let iter = SpecIterator::new_with_start("1H:MM:SS", start, DstPolicy::default()).unwrap();
        let results = iter.take(3).collect::<Vec<DateTime<_>>>().unwrap();
        assert_eq!(
            results,
            vec![
                Utc.with_ymd_and_hms(2025, 1, 1, 9, 22, 45).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 10, 22, 45).unwrap(),
                Utc.with_ymd_and_hms(2025, 1, 1, 11, 22, 45).unwrap(),
            ]
        );
    }
}
