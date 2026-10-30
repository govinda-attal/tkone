use std::marker::PhantomData;

use chrono::{DateTime, Duration, FixedOffset, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};

use fallible_iterator::FallibleIterator;

use super::{engine, spec::Spec};
use crate::merge::{union_parts, Merge};
use crate::utils::resolve_local;
use crate::{prelude::*, DstPolicy, IntervalMode, TimeAnchor};

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
/// Any builder may be bounded with `.with_end(end)` (inclusive). After
/// `new_with_start` you may instead call `.with_end_spec(end_spec)` to bound
/// it by another time spec.
///
/// Any builder can also take `.with_dst_policy(…)` and `.with_interval_mode(…)`
/// to control behaviour across DST changes (see [`crate::IntervalMode`]), and
/// `.with_time_anchor(TimeAnchor::Midnight)` to align steps to the clock
/// instead of the start (see [`crate::TimeAnchor`]).
pub struct SpecIteratorBuilder<Tz: TimeZone, START, END, S> {
    timezone: Tz,
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    end: END,
    dst_policy: DstPolicy,
    interval_mode: IntervalMode,
    time_anchor: TimeAnchor,
    /// Wall-clock cursor given to `resume_after`; `None` means `dtm`.
    local: Option<NaiveDateTime>,
    /// Wall-clock start given to `resume_with_start`.
    anchor: Option<NaiveDateTime>,
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

    /// Choose whether steps (`nH`, `nM`, `nS`) move the wall clock or measure
    /// elapsed time across DST changes.
    ///
    /// Defaults to [`IntervalMode::WallClock`].
    pub fn with_interval_mode(mut self, mode: IntervalMode) -> Self {
        self.interval_mode = mode;
        self
    }

    /// Choose where steps and carried fields are counted from: the start
    /// (default) or each day's midnight, which aligns steps to the clock
    /// (`HH:15M:00` at :00, :15, :30, :45 whatever the start).
    pub fn with_time_anchor(mut self, anchor: TimeAnchor) -> Self {
        self.time_anchor = anchor;
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
            interval_mode: IntervalMode::default(),
            time_anchor: TimeAnchor::default(),
            local: None,
            anchor: None,
            marker_sealed: PhantomData,
        }
    }

    /// Resume a stored schedule: times strictly after `last`, the last time
    /// you handled, in timezone `tz`.
    ///
    /// Store `last` with its UTC offset (e.g. RFC 3339) and pass it back here,
    /// with the same options as the original schedule. See the crate docs on
    /// [persisting and resuming](crate#persisting-and-resuming-a-schedule).
    pub fn resume_after(
        spec: &str,
        tz: Tz,
        last: DateTime<FixedOffset>,
    ) -> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
        let mut builder = Self::new_after(spec, last.with_timezone(&tz));
        builder.local = Some(last.naive_local());
        builder
    }

    /// Resume a stored schedule from its original `start`: times strictly
    /// after `last`, on the grid counted from `start`.
    ///
    /// Required for a union spec (`A | B`), whose parts would otherwise count
    /// their steps from another part's time. For a plain spec it gives the
    /// same results as [`resume_after`](Self::resume_after). A union in
    /// [`IntervalMode::Elapsed`] cannot be resumed
    /// ([`Error::UnsupportedResume`]).
    pub fn resume_with_start(
        spec: &str,
        tz: Tz,
        start: DateTime<FixedOffset>,
        last: DateTime<FixedOffset>,
    ) -> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
        let mut builder = Self::resume_after(spec, tz, last);
        builder.anchor = Some(start.naive_local());
        builder
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        let iter = SpecIterator::new_after(&self.spec, self.dtm, self.dst_policy)?;
        resumed(iter, &self.spec, self.interval_mode, self.time_anchor, self.local, self.anchor)
    }

    /// Bound the iterator by an explicit end datetime (inclusive).
    ///
    /// Useful with [`resume_after`](Self::resume_after) for a stored schedule
    /// that has an end.
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, NoStart, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            timezone: self.timezone,
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            end: EndDateTime(end),
            dst_policy: self.dst_policy,
            interval_mode: self.interval_mode,
            time_anchor: self.time_anchor,
            local: self.local,
            anchor: self.anchor,
            marker_sealed: PhantomData,
        }
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, NoStart, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        let end = self.end.0.with_timezone(&self.dtm.timezone());
        let iter = SpecIterator::new(&self.spec, self.dtm, None, Some(end), self.dst_policy)?;
        resumed(iter, &self.spec, self.interval_mode, self.time_anchor, self.local, self.anchor)
    }
}

/// Apply the options and any `resume_after` / `resume_with_start` cursor.
fn resumed<Tz: TimeZone>(
    iter: SpecIterator<Tz>,
    spec: &str,
    mode: IntervalMode,
    time_anchor: TimeAnchor,
    local: Option<NaiveDateTime>,
    anchor: Option<NaiveDateTime>,
) -> Result<SpecIterator<Tz>> {
    let union = union_parts(spec).is_some();
    match (anchor, local) {
        (None, Some(_)) if union => {
            return Err(Error::UnsupportedResume(
                "a union spec must be resumed with `resume_with_start`".into(),
            ))
        }
        (Some(_), _) if union && mode == IntervalMode::Elapsed => {
            return Err(Error::UnsupportedResume("a union spec in `IntervalMode::Elapsed`".into()))
        }
        _ => {}
    }
    let mut iter = iter.with_interval_mode(mode).with_time_anchor(time_anchor);
    // Elapsed steps continue from the instant, so they keep its own local
    // time; wall-clock steps continue from the stored wall clock, on the grid
    // counted from the stored start if there is one.
    if mode == IntervalMode::WallClock {
        iter.each_part(|part| {
            if let Some(local) = local {
                part.local = local;
            }
            part.anchor = anchor;
        });
    }
    Ok(iter)
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
            interval_mode: IntervalMode::default(),
            time_anchor: TimeAnchor::default(),
            local: None,
            anchor: None,
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
            interval_mode: self.interval_mode,
            time_anchor: self.time_anchor,
            local: self.local,
            anchor: self.anchor,
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
            interval_mode: self.interval_mode,
            time_anchor: self.time_anchor,
            local: self.local,
            anchor: self.anchor,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        Ok(SpecIterator::new_with_start(&self.spec, self.dtm, self.dst_policy)?
            .with_interval_mode(self.interval_mode)
            .with_time_anchor(self.time_anchor))
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        Ok(SpecIterator::new_with_end(&self.spec, self.dtm, self.end.0, self.dst_policy)?
            .with_interval_mode(self.interval_mode)
            .with_time_anchor(self.time_anchor))
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        Ok(
            SpecIterator::new_with_end_spec(&self.spec, self.dtm, &self.end.0, self.dst_policy)?
                .with_interval_mode(self.interval_mode)
            .with_time_anchor(self.time_anchor),
        )
    }
}

/// ## SpecIterator
/// An iterator for generating recurring timezone aware datetimes as per time based specifications.
/// ### Examples
///
/// ```rust
/// use tkone_schedule::time::SpecIteratorBuilder;
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
    spec: Spec,
    dst_policy: DstPolicy,
    interval_mode: IntervalMode,
    time_anchor: TimeAnchor,
    /// Local time of the last result (or the initial cursor). The next result
    /// is searched for after it, and it anchors the grid's phase unless
    /// `anchor` is set.
    local: NaiveDateTime,
    /// The stored start of a `resume_with_start` schedule: the grid's phase.
    anchor: Option<NaiveDateTime>,
    /// The last yielded instant, or the initial cursor. Results must be
    /// strictly later.
    cursor: DateTime<Tz>,
    /// Yielded first, then cleared.
    start: Option<DateTime<Tz>>,
    /// Inclusive upper bound.
    end: Option<DateTime<Tz>>,
    done: bool,
    /// For a union spec (`A | B`), its parts; the other fields are unused.
    union: Option<Box<Merge<SpecIterator<Tz>, DateTime<Tz>>>>,
    /// A union part: fixed times of day recur every day instead of only on
    /// the current day.
    daily: bool,
}

fn instant<Tz: TimeZone>(t: &DateTime<Tz>) -> DateTime<Tz> {
    t.clone()
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    fn new(
        spec: &str,
        cursor: DateTime<Tz>,
        start: Option<DateTime<Tz>>,
        end: Option<DateTime<Tz>>,
        dst_policy: DstPolicy,
    ) -> Result<Self> {
        if let Some(parts) = union_parts(spec) {
            let parts = parts
                .into_iter()
                .map(|part| {
                    let part = Self::new(part, cursor.clone(), start.clone(), end.clone(), dst_policy)?;
                    Ok(Self { daily: true, ..part })
                })
                .collect::<Result<Vec<_>>>()?;
            let mut outer = parts[0].clone();
            outer.union = Some(Box::new(Merge::new(parts, instant)));
            return Ok(outer);
        }
        Ok(Self {
            tz: cursor.timezone(),
            spec: spec.parse()?,
            dst_policy,
            interval_mode: IntervalMode::default(),
            time_anchor: TimeAnchor::default(),
            local: cursor.naive_local(),
            anchor: None,
            cursor,
            start,
            end,
            done: false,
            union: None,
            daily: false,
        })
    }

    fn new_after(spec: &str, dtm: DateTime<Tz>, dst_policy: DstPolicy) -> Result<Self> {
        Self::new(spec, dtm, None, None, dst_policy)
    }

    fn new_with_start(spec: &str, start: DateTime<Tz>, dst_policy: DstPolicy) -> Result<Self> {
        Self::new(spec, start.clone(), Some(start), None, dst_policy)
    }

    fn new_with_end(
        spec: &str,
        start: DateTime<Tz>,
        end: DateTime<Tz>,
        dst_policy: DstPolicy,
    ) -> Result<Self> {
        let end = end.with_timezone(&start.timezone());
        Self::new(spec, start.clone(), Some(start), Some(end), dst_policy)
    }

    /// The end is the first occurrence of `end_spec` after `start`, on any
    /// day: `13:00:00` from a 14:00 start is 13:00 the next day.
    fn new_with_end_spec(
        spec: &str,
        start: DateTime<Tz>,
        end_spec: &str,
        dst_policy: DstPolicy,
    ) -> Result<Self> {
        let local = start.naive_local();
        let mut end = None;
        for part in union_parts(end_spec).unwrap_or_else(|| vec![end_spec]) {
            let next = engine::next_after(&part.parse()?, local, local).ok_or(Error::InvalidEndSpec)?;
            end = Some(end.map_or(next, |e: NaiveDateTime| e.min(next)));
        }
        let end = end.ok_or(Error::InvalidEndSpec)?;
        let end = resolve_local(&start.timezone(), end, dst_policy)?;
        Self::new(spec, start.clone(), Some(start), Some(end), dst_policy)
    }

    /// Apply `f` to this iterator, or to each part of a union.
    fn each_part(&mut self, f: impl Fn(&mut Self)) {
        match &mut self.union {
            Some(union) => union.parts_mut().iter_mut().for_each(f),
            None => f(self),
        }
    }

    fn with_interval_mode(mut self, mode: IntervalMode) -> Self {
        self.each_part(|part| part.interval_mode = mode);
        self
    }

    fn with_time_anchor(mut self, anchor: TimeAnchor) -> Self {
        self.each_part(|part| part.time_anchor = anchor);
        self
    }

    /// The next wall-clock time after `self.local`. With `TimeAnchor::Midnight`
    /// each day's times come from a grid that starts at that day's midnight.
    fn next_local(&self) -> Option<NaiveDateTime> {
        if self.time_anchor == TimeAnchor::Start {
            return engine::next_after(&self.spec, self.anchor.unwrap_or(self.local), self.local);
        }
        let (mut day, mut after) = (self.local.date(), self.local);
        // Every day has the same grid, so a day without a time means none ever.
        for _ in 0..2 {
            let next = engine::next_after(&self.spec, day.and_time(NaiveTime::MIN), after)?;
            if next.date() == day {
                return Some(next);
            }
            day = day.succ_opt()?;
            after = day.and_time(NaiveTime::MIN) - Duration::nanoseconds(1);
        }
        None
    }
}

impl<Tz: TimeZone> FallibleIterator for SpecIterator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if let Some(union) = &mut self.union {
            return union.next();
        }
        if self.done {
            return Ok(None);
        }
        if let Some(start) = self.start.take() {
            if self.end.as_ref().is_some_and(|end| start > *end) {
                self.done = true;
                return Ok(None);
            }
            return Ok(Some(start));
        }

        loop {
            let Some(next) = self.next_local() else {
                self.done = true;
                return Ok(None);
            };
            // A spec without a step names fixed times of day; on its own it
            // yields them on the current day only. In a union they recur.
            if !self.daily && !engine::has_step(&self.spec) && next.date() != self.local.date() {
                self.done = true;
                return Ok(None);
            }
            let instant = match self.interval_mode {
                IntervalMode::WallClock => {
                    self.local = next;
                    resolve_local(&self.tz, next, self.dst_policy)?
                }
                IntervalMode::Elapsed => {
                    // `next` was computed in the previous occurrence's
                    // offset, so the step measures real elapsed time.
                    let offset = self.cursor.offset().fix().local_minus_utc();
                    let instant = self
                        .tz
                        .from_utc_datetime(&(next - Duration::seconds(i64::from(offset))));
                    self.local = instant.naive_local();
                    instant
                }
            };
            // Distinct wall-clock times can land on one instant (or go
            // backwards) across a spring-forward gap; skip them.
            if instant <= self.cursor {
                continue;
            }
            if self.end.as_ref().is_some_and(|end| instant > *end) {
                self.done = true;
                return Ok(None);
            }
            self.cursor = instant.clone();
            return Ok(Some(instant));
        }
    }
}

/// ## NaiveSpecIterator
/// An iterator for generating recurring naive datetimes as per time based specifications.
///
/// Behaves like [`SpecIterator`] in UTC: no DST, so steps are plain durations.
///
/// ### Examples
///
/// ```rust
/// use tkone_schedule::time::NaiveSpecIterator;
/// use chrono::{NaiveDate, NaiveDateTime, Duration};
/// use fallible_iterator::FallibleIterator;
///
/// let start = NaiveDate::from_ymd_opt(2024, 3, 31).unwrap().and_hms_opt(10, 0, 0).unwrap();
/// let iter = NaiveSpecIterator::new_with_start("1H:00:00", start.clone()).unwrap();
/// let occurrences = iter.take(3).collect::<Vec<NaiveDateTime>>().unwrap();
///
/// assert_eq!(occurrences, vec![start, start + Duration::hours(1), start + Duration::hours(2)]);
/// ```
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator(SpecIterator<Utc>);

impl NaiveSpecIterator {
    pub fn new_after(spec: &str, dtm: NaiveDateTime) -> Result<Self> {
        Ok(Self(SpecIterator::new_after(spec, dtm.and_utc(), DstPolicy::Adjust)?))
    }

    pub fn new_with_start(spec: &str, start: NaiveDateTime) -> Result<Self> {
        Ok(Self(SpecIterator::new_with_start(spec, start.and_utc(), DstPolicy::Adjust)?))
    }

    pub fn new_with_end(spec: &str, start: NaiveDateTime, end: NaiveDateTime) -> Result<Self> {
        Ok(Self(SpecIterator::new_with_end(
            spec,
            start.and_utc(),
            end.and_utc(),
            DstPolicy::Adjust,
        )?))
    }

    pub fn new_with_end_spec(spec: &str, start: NaiveDateTime, end_spec: &str) -> Result<Self> {
        Ok(Self(SpecIterator::new_with_end_spec(
            spec,
            start.and_utc(),
            end_spec,
            DstPolicy::Adjust,
        )?))
    }
}

impl FallibleIterator for NaiveSpecIterator {
    type Item = NaiveDateTime;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        Ok(self.0.next()?.map(|d| d.naive_utc()))
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
