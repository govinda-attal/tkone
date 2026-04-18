use super::spec::Spec;
use crate::biz_day::BizDayProcessor;
use crate::date::NaiveSpecIterator as DateNaiveSpecIterator;
use crate::prelude::*;
use crate::time::{Cycle as TimeCycle, Spec as TimeSpec};
use crate::utils::next_result_to_tz;
use crate::{DstPolicy, Occurrence};
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, TimeZone, Timelike, Utc};
use fallible_iterator::FallibleIterator;
use std::marker::PhantomData;
use std::str::FromStr;

// --- Builder type-state markers (same pattern as date/time) ---
pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

/// Fluent, type-state builder for the combined datetime [`SpecIterator`].
///
/// See the [module documentation](crate::datetime) for the spec format.
///
/// # Construction variants
///
/// | Constructor | First result | Use when… |
/// |-------------|-------------|-----------|
/// | `new(spec, bdp, tz)` | First occurrence after `Utc::now()` | open-ended schedule from now |
/// | `new_after(spec, bdp, dtm)` | First occurrence **after** `dtm` | schedule from a known cursor |
/// | `new_with_start(spec, bdp, start)` | `start` itself is the first item | anchor to a fixed start datetime |
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::biz_day::WeekendSkipper;
/// use tkone_schedule::datetime::SpecIteratorBuilder;
/// use tkone_schedule::Occurrence;
/// use chrono::{TimeZone, Utc};
/// use fallible_iterator::FallibleIterator;
///
/// let bdp   = WeekendSkipper::new();
/// let start = Utc.with_ymd_and_hms(2024, 1, 31, 11, 0, 0).unwrap();
/// let end   = Utc.with_ymd_and_hms(2024, 4, 30, 11, 0, 0).unwrap();
///
/// // Last day of every month at 11:00 UTC, bounded until end of April
/// let iter = SpecIteratorBuilder::new_with_start(
///     "YY-1M-L~WT11:00:00", bdp, start,
/// )
/// .with_end(end)
/// .build()
/// .unwrap();
///
/// let occurrences: Vec<_> = iter.collect().unwrap();
/// // → 2024-01-31T11:00, 2024-02-29T11:00, 2024-03-29T11:00 (Fri adj), 2024-04-30T11:00
/// ```
pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    bd_processor: BDP,
    end: END,
    timezone: Tz,
    dst_policy: DstPolicy,
    marker_sealed: PhantomData<S>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor, START, END, S>
    SpecIteratorBuilder<Tz, BDP, START, END, S>
{
    /// Override the DST resolution policy for this iterator.
    ///
    /// Defaults to [`DstPolicy::Adjust`]. Set to [`DstPolicy::Strict`] to
    /// receive [`Error::AmbiguousLocalTime`] instead of silent adjustment.
    pub fn with_dst_policy(mut self, policy: DstPolicy) -> Self {
        self.dst_policy = policy;
        self
    }
}

// --- no-start, no-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    /// Create an iterator from `Utc::now()` in timezone `tz`. The current
    /// instant is excluded; the first result is strictly after now.
    pub fn new(spec: &str, bdp: BDP, tz: Tz) -> Self {
        Self::new_after(spec, bdp, Utc::now().with_timezone(&tz))
    }

    /// Create an iterator that produces occurrences strictly **after `dtm`**.
    pub fn new_after(spec: &str, bdp: BDP, dtm: DateTime<Tz>) -> Self {
        SpecIteratorBuilder {
            timezone: dtm.timezone(),
            dtm: dtm.clone(),
            start: NoStart,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            dst_policy: DstPolicy::default(),
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let spec = Spec::from_str(&self.spec)?;
        Ok(SpecIterator {
            tz: self.dtm.timezone(),
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_after(
                &spec.date_spec,
                &spec.time_spec,
                self.bd_processor,
                self.dtm.naive_local(),
            )?,
        })
    }
}

// --- with-start, no-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    /// Create an iterator where `start` is the **first yielded item**.
    ///
    /// Use this after you have resolved the start datetime from the spec:
    ///
    /// ```rust
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::datetime::SpecIteratorBuilder;
    /// use tkone_schedule::Occurrence;
    /// use chrono::Utc;
    /// use fallible_iterator::FallibleIterator;
    ///
    /// let bdp = WeekendSkipper::new();
    /// let now = Utc::now();
    ///
    /// // Step 1: resolve start datetime from the spec
    /// let start = SpecIteratorBuilder::new_after("YY-1M-L~WT11:00:00", bdp.clone(), now)
    ///     .build().unwrap()
    ///     .next().unwrap().unwrap()
    ///     .observed().clone();
    ///
    /// // Step 2: anchor the recurrence to that start datetime
    /// let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~WT11:00:00", bdp, start)
    ///     .build().unwrap();
    ///
    /// let results: Vec<_> = iter.take(3).collect().unwrap();
    /// assert_eq!(results[0].observed(), &start);
    /// ```
    pub fn new_with_start(spec: &str, bdp: BDP, start: DateTime<Tz>) -> Self {
        SpecIteratorBuilder {
            timezone: start.timezone(),
            dtm: start.clone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            dst_policy: DstPolicy::default(),
            marker_sealed: PhantomData,
        }
    }

    /// Bound the iterator by an explicit end datetime (exclusive upper bound).
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            timezone: self.timezone,
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            dst_policy: self.dst_policy,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let spec = Spec::from_str(&self.spec)?;
        let start = self.start.0;
        Ok(SpecIterator {
            tz: start.timezone(),
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_start(
                &spec.date_spec,
                &spec.time_spec,
                self.bd_processor,
                start.naive_local(),
            )?,
        })
    }
}

// --- with-start, with-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let spec = Spec::from_str(&self.spec)?;
        let start = self.start.0;
        Ok(SpecIterator {
            tz: start.timezone(),
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                &spec.date_spec,
                &spec.time_spec,
                self.bd_processor,
                start.naive_local(),
                self.end.0.naive_local(),
            )?,
        })
    }
}

// ---------------------------------------------------------------------------
// SpecIterator (timezone-aware wrapper)
// ---------------------------------------------------------------------------

/// Timezone-aware datetime recurrence iterator combining a date spec and a time spec.
///
/// Use [`SpecIteratorBuilder`] to construct one.
#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    dst_policy: DstPolicy,
    naive_spec_iter: NaiveSpecIterator<BDP>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor + Clone> FallibleIterator for SpecIterator<Tz, BDP> {
    type Item = Occurrence<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let next = self.naive_spec_iter.next()?;
        let Some(next) = next else {
            return Ok(None);
        };
        Ok(Some(next_result_to_tz(&self.tz, next, self.dst_policy)?))
    }
}

// ---------------------------------------------------------------------------
// NaiveSpecIterator
// ---------------------------------------------------------------------------

/// Naive (non-timezone-aware) datetime recurrence iterator.
///
/// Combines a date spec (which calendar days to visit) with a time spec
/// (which times within each day to emit), using a **date-first** strategy:
///
/// 1. Find the next valid calendar date via the date spec.
/// 2. Within that date, emit every matching time produced by the time spec.
/// 3. When times are exhausted for that date, advance to the next valid date.
///
/// This correctly handles sub-daily recurrence on specific dates, e.g.
/// "every 30 minutes on the last business day of each month".
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    date_iter: DateNaiveSpecIterator<BDP>,
    time_spec: TimeSpec,
    /// Exclusive end of the current date window (next midnight after the date
    /// we are currently emitting times for).  `None` until the first date is
    /// entered.
    current_date_end: Option<NaiveDateTime>,
    /// Set to `true` the first time we successfully consume a result from
    /// `date_iter`.  Until then we must NOT call `date_iter.update_cursor`,
    /// because the iterator is already pre-positioned before the starting date
    /// and advancing it would make `NextNth(n)` skip the first eligible period.
    date_iter_started: bool,
    /// Cursor to use when advancing the date iterator to the next period.
    /// Stored as `actual_date 23:59:59` (the *unadjusted* date) rather than
    /// `observed_date 23:59:59`, so that business-day adjustments that push
    /// the observed date into the next month do not cause that month's natural
    /// occurrence to be skipped.  For `Single` results actual == observed, so
    /// there is no behavioural difference in the common case.
    next_period_cursor: Option<NaiveDateTime>,
    /// Moving cursor — the last datetime returned (or the initial value).
    dtm: NaiveDateTime,
    /// The very first cursor value supplied by the caller, used to restrict
    /// which times are eligible on the first day.
    initial_dtm: NaiveDateTime,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    index: usize,
}

impl<BDP: BizDayProcessor + Clone> NaiveSpecIterator<BDP> {
    /// Iterate occurrences strictly after `dtm`.
    pub(crate) fn new_after(
        date_spec: &str,
        time_spec: &str,
        bdp: BDP,
        dtm: NaiveDateTime,
    ) -> Result<Self> {
        // Position the date iterator just before today so that today is a
        // candidate on the first `date_iter.next()` call.
        let before_today = dtm.date().and_hms_opt(0, 0, 0).unwrap() - Duration::seconds(1);
        Ok(Self {
            date_iter: DateNaiveSpecIterator::new_after(date_spec, bdp, before_today)?,
            time_spec: time_spec.parse()?,
            current_date_end: None,
            date_iter_started: false,
            next_period_cursor: None,
            dtm,
            initial_dtm: dtm,
            start: None,
            end: None,
            index: 0,
        })
    }

    /// Include `start` as the first result, then iterate forward.
    pub(crate) fn new_with_start(
        date_spec: &str,
        time_spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
    ) -> Result<Self> {
        let before_today = start.date().and_hms_opt(0, 0, 0).unwrap() - Duration::seconds(1);
        Ok(Self {
            date_iter: DateNaiveSpecIterator::new_after(date_spec, bdp, before_today)?,
            time_spec: time_spec.parse()?,
            current_date_end: None,
            date_iter_started: false,
            next_period_cursor: None,
            dtm: start,
            initial_dtm: start,
            start: Some(start),
            end: None,
            index: 0,
        })
    }

    /// Like [`new_with_start`] but stop at `end` (inclusive).
    pub(crate) fn new_with_end(
        date_spec: &str,
        time_spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> Result<Self> {
        let mut s = Self::new_with_start(date_spec, time_spec, bdp, start)?;
        s.end = Some(end);
        Ok(s)
    }
}

impl<BDP: BizDayProcessor + Clone> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = Occurrence<NaiveDateTime>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        // ── global end guard ──────────────────────────────────────────────
        if let Some(end) = self.end {
            if self.dtm >= end {
                return Ok(None);
            }
        }

        // ── index-0 start passthrough ─────────────────────────────────────
        if self.index == 0 {
            if let Some(start) = self.start {
                if self.dtm <= start {
                    self.dtm = start;
                    self.index += 1;
                    self.current_date_end = Some(midnight_next(start.date()));
                    // Prime the date iterator so that when the time loop for
                    // this day exhausts, update_cursor advances past start's
                    // date rather than re-emitting it from midnight.
                    self.date_iter_started = true;
                    self.next_period_cursor = Some(start.date().and_hms_opt(23, 59, 59).unwrap());
                    return Ok(Some(Occurrence::Exact(start)));
                }
            }
        }

        // ── try next time within the current date window ──────────────────
        if let Some(date_end) = self.current_date_end {
            let candidate = apply_time_spec(&self.time_spec, self.dtm);
            if candidate > self.dtm && candidate < date_end {
                if let Some(end) = self.end {
                    if candidate > end {
                        return Ok(None);
                    }
                }
                self.dtm = candidate;
                self.index += 1;
                return Ok(Some(Occurrence::Exact(candidate)));
            }
        }

        // ── advance to the next valid date ────────────────────────────────
        //
        // Prefer `next_period_cursor` (actual_date 23:59:59) over the plain
        // `date_end - 1s` fallback.  When a business-day adjustment pushes the
        // observed date into the next month (AdjustedLater), using the *actual*
        // (unadjusted) date as the cursor keeps the month-counting epoch correct
        // and prevents that next month's natural occurrence from being skipped.
        //
        // We only call update_cursor once the date iterator has been used at
        // least once (`date_iter_started`).  Before that first use the iterator
        // is already pre-positioned before the starting date; advancing it
        // further would cause `NextNth(n)` month cycles to skip the first
        // eligible period (e.g. May → June when we want May 31).
        if self.date_iter_started {
            if let Some(cursor) = self.next_period_cursor.take() {
                self.date_iter.update_cursor(cursor);
            } else if let Some(date_end) = self.current_date_end {
                self.date_iter
                    .update_cursor(date_end - Duration::seconds(1));
            }
        }

        loop {
            let next_date = self.date_iter.next()?;
            let Some(next_date) = next_date else {
                return Ok(None);
            };
            self.date_iter_started = true;

            let observed_date = next_date.observed().date();
            let date_midnight = observed_date.and_hms_opt(0, 0, 0).unwrap();
            let date_end = midnight_next(observed_date);

            // Always use the *actual* (unadjusted) date for the next period
            // cursor so that month-boundary adjustments don't shift the epoch.
            let next_period_cursor = next_date.actual().date().and_hms_opt(23, 59, 59).unwrap();

            // On the very first date entry (current_date_end is still None),
            // if the date returned is today, use `initial_dtm` as the time
            // cursor so we only emit times *after* the caller's starting point.
            // On any subsequent date, start from midnight to get all times.
            let is_initial_day =
                self.current_date_end.is_none() && observed_date == self.initial_dtm.date();

            // On the initial day use the caller's cursor so only times *after*
            // the start are emitted.  On any subsequent date, step back by the
            // spec's natural driving period from midnight and apply once: this
            // lands exactly at midnight when midnight is a natural boundary
            // (e.g. `1H:00:00` → back 1 h → 23:00 → +1 h → 00:00).  If the
            // candidate falls before midnight (e.g. an `At`-only spec such as
            // `11:00:00` gives `11:00` of the previous day), fall back to
            // applying the spec from midnight itself.
            let first_time = if is_initial_day {
                apply_time_spec(&self.time_spec, self.initial_dtm)
            } else {
                let delta = spec_delta(&self.time_spec);
                let candidate = apply_time_spec(&self.time_spec, date_midnight - delta);
                if candidate >= date_midnight {
                    candidate
                } else {
                    apply_time_spec(&self.time_spec, date_midnight)
                }
            };

            let is_valid = if is_initial_day {
                // Must be strictly after the initial cursor and within today
                first_time > self.initial_dtm && first_time < date_end
            } else {
                // Must be within the day window (>= midnight is guaranteed;
                // only fail if Every-cycle pushes past midnight)
                first_time < date_end
            };

            if !is_valid {
                // No eligible time on this date — skip it and try the next.
                self.current_date_end = Some(date_end);
                self.date_iter.update_cursor(next_period_cursor);
                continue;
            }

            // End-of-range check
            if let Some(end) = self.end {
                if first_time > end {
                    return Ok(None);
                }
            }

            self.current_date_end = Some(date_end);
            self.next_period_cursor = Some(next_period_cursor);
            self.dtm = first_time;
            self.index += 1;

            // Propagate business-day adjustment info from the date result.
            let result = match next_date {
                Occurrence::Exact(_) => Occurrence::Exact(first_time),
                Occurrence::AdjustedEarlier(actual, _) => Occurrence::AdjustedEarlier(
                    actual.date().and_time(first_time.time()),
                    first_time,
                ),
                Occurrence::AdjustedLater(actual, _) => {
                    Occurrence::AdjustedLater(actual.date().and_time(first_time.time()), first_time)
                }
            };

            return Ok(Some(result));
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Apply a time spec to a cursor datetime, mirroring the time iterator logic:
/// seconds → minutes → hours, each either `At(n)` (set absolute) or `Every(n)` (add delta).
/// `ForEach` on the finest non-At component acts as `Every(1)` when no explicit `Every` exists.
/// `AsIs` is a true no-op — it always carries the current value.
fn apply_time_spec(spec: &TimeSpec, cursor: NaiveDateTime) -> NaiveDateTime {
    let has_any_every = matches!(spec.seconds, TimeCycle::Every(_))
        || matches!(spec.minutes, TimeCycle::Every(_))
        || matches!(spec.hours, TimeCycle::Every(_));
    let seconds_is_foreach = matches!(spec.seconds, TimeCycle::ForEach);
    let minutes_is_foreach = matches!(spec.minutes, TimeCycle::ForEach);

    let next = cursor;
    let next = match &spec.seconds {
        TimeCycle::At(s) => next.with_second(*s as u32).unwrap(),
        TimeCycle::Every(s) => next + Duration::seconds(*s as i64),
        TimeCycle::ForEach if !has_any_every => next + Duration::seconds(1),
        TimeCycle::ForEach | TimeCycle::AsIs => next,
    };
    let next = match &spec.minutes {
        TimeCycle::At(m) => next.with_minute(*m as u32).unwrap(),
        TimeCycle::Every(m) => next + Duration::minutes(*m as i64),
        TimeCycle::ForEach if !has_any_every && !seconds_is_foreach => next + Duration::minutes(1),
        TimeCycle::ForEach | TimeCycle::AsIs => next,
    };
    match &spec.hours {
        TimeCycle::At(h) => next.with_hour(*h as u32).unwrap(),
        TimeCycle::Every(h) => next + Duration::hours(*h as i64),
        TimeCycle::ForEach if !has_any_every && !seconds_is_foreach && !minutes_is_foreach => {
            next + Duration::hours(1)
        }
        TimeCycle::ForEach | TimeCycle::AsIs => next,
    }
}

/// Return the natural step size of the driving component of a time spec.
///
/// Used by the new-day first-tick calculation: stepping back one period from
/// midnight and applying the spec once produces midnight itself whenever
/// midnight is a natural boundary of the cycle.
///
/// Rules (coarsest-to-finest, first match wins):
/// - `Every(n)` on seconds  → `n` seconds
/// - `ForEach` on seconds (and no `Every` anywhere) → 1 second
/// - `Every(n)` on minutes  → `n` minutes
/// - `ForEach` on minutes (and no `Every` anywhere) → 1 minute
/// - `Every(n)` on hours    → `n` hours
/// - `ForEach` on hours (and no `Every` anywhere) → 1 hour
/// - All `At` / `AsIs`      → 1 second (safe fallback; result will be < midnight,
///                             triggering the `apply_time_spec(midnight)` fallback)
fn spec_delta(spec: &TimeSpec) -> Duration {
    let has_any_every = matches!(spec.seconds, TimeCycle::Every(_))
        || matches!(spec.minutes, TimeCycle::Every(_))
        || matches!(spec.hours, TimeCycle::Every(_));

    match &spec.seconds {
        TimeCycle::Every(n) => Duration::seconds(*n as i64),
        TimeCycle::ForEach if !has_any_every => Duration::seconds(1),
        _ => match &spec.minutes {
            TimeCycle::Every(n) => Duration::minutes(*n as i64),
            TimeCycle::ForEach if !has_any_every => Duration::minutes(1),
            _ => match &spec.hours {
                TimeCycle::Every(n) => Duration::hours(*n as i64),
                TimeCycle::ForEach if !has_any_every => Duration::hours(1),
                _ => Duration::seconds(1),
            },
        },
    }
}

/// Returns midnight at the start of the *next* day after `date`.
fn midnight_next(date: NaiveDate) -> NaiveDateTime {
    (date + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap()
}

