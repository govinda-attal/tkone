use super::spec::Spec;
use crate::limits::{skip_through, Limits};
use crate::merge::{add_exclusions, split_exclusions, union_parts, Merge};
use crate::biz_day::BizDayProcessor;
use crate::date::NaiveSpecIterator as DateNaiveSpecIterator;
use crate::prelude::*;
use crate::time::Spec as TimeSpec;
use crate::utils::{next_result_to_tz, resolve_local};
use crate::{DstPolicy, IntervalMode, Occurrence, TimeAnchor};
use chrono::{
    DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc,
};
use fallible_iterator::FallibleIterator;
use std::marker::PhantomData;
use std::str::FromStr;

// --- Builder type-state markers (same pattern as date/time) ---
pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
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
/// Any builder may be bounded with `.with_end(end)`; after `new_with_start`
/// also with `.with_end_spec(end_spec)` (both inclusive). Any builder also takes
/// `.with_dst_policy(…)`, `.with_interval_mode(…)` and `.with_time_anchor(…)`.
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
/// // Last day of every month at 11:00 UTC, up to and including end of April
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
    options: Options,
    /// Wall-clock cursor given to `resume_after`; `None` means `dtm`.
    local: Option<NaiveDateTime>,
    /// Wall-clock start given to `resume_with_start`.
    anchor: Option<NaiveDateTime>,
    limits: Limits,
    marker_sealed: PhantomData<(Tz, S)>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Options {
    dst_policy: DstPolicy,
    interval_mode: IntervalMode,
    time_anchor: TimeAnchor,
}

impl<Tz: TimeZone, BDP: BizDayProcessor, START, END, S>
    SpecIteratorBuilder<Tz, BDP, START, END, S>
{
    /// Override the DST resolution policy for this iterator.
    ///
    /// Defaults to [`DstPolicy::Adjust`]. Set to [`DstPolicy::Strict`] to
    /// receive [`Error::AmbiguousLocalTime`] instead of silent adjustment.
    pub fn with_dst_policy(mut self, policy: DstPolicy) -> Self {
        self.options.dst_policy = policy;
        self
    }

    /// Choose whether time-spec steps (`nH`, `nM`, `nS`) move the wall clock
    /// or measure elapsed time across DST changes.
    ///
    /// Defaults to [`IntervalMode::WallClock`]. With `Elapsed`, the first time
    /// on each date still comes from the wall clock; later times that day are
    /// whole steps of real time after the previous one.
    pub fn with_interval_mode(mut self, mode: IntervalMode) -> Self {
        self.options.interval_mode = mode;
        self
    }

    /// Choose which times each date gets: following the start's time of day,
    /// or restarting from midnight.
    ///
    /// Defaults to [`TimeAnchor::Start`].
    pub fn with_time_anchor(mut self, anchor: TimeAnchor) -> Self {
        self.options.time_anchor = anchor;
        self
    }

    /// Skip the occurrences whose actual date matches `spec`, a date spec (a
    /// union is allowed): `.with_exclusions("YY-12-25 | YY-01-01")` skips every
    /// time on 25 December and 1 January. The same as writing `! spec` at the
    /// end of the schedule's spec, and combined with any exclusions already
    /// there. Cadences in `spec` are counted from the start; day steps (`7D`)
    /// are not allowed.
    pub fn with_exclusions(mut self, spec: impl AsRef<str>) -> Self {
        self.spec = add_exclusions(&self.spec, spec.as_ref());
        self
    }

    /// Stop after `n` results, counted from the start (a `new_with_start`
    /// start is the first). Excluded occurrences are not counted.
    ///
    /// A counted schedule is resumed with `resume_with_start`, which replays
    /// it from its start to keep the count; `resume_after` returns
    /// [`Error::UnsupportedResume`].
    pub fn with_count(mut self, n: usize) -> Self {
        self.limits.count = Some(n);
        self
    }

    /// Build the iterator from `dtm` (the start when `include_start`), with
    /// `with_count` applied. A resumed counted schedule is replayed from its
    /// stored start instead.
    fn finish(self, include_start: bool, end: Option<DateTime<Tz>>) -> Result<SpecIterator<Tz, BDP>> {
        let bdp = self.bd_processor;
        let count = self.limits.count;
        if let Some(start) = self.limits.replay_from(self.local)? {
            let start = start.with_timezone(&self.dtm.timezone());
            let mut continuous = SpecIterator::new(&self.spec, bdp, start, None, None, true, end, self.options)?;
            continuous.remaining = count;
            return skip_through(continuous, &self.dtm, actual);
        }
        let mut iter = SpecIterator::new(&self.spec, bdp, self.dtm.clone(), self.local, self.anchor, include_start, end, self.options)?;
        iter.remaining = count;
        Ok(iter)
    }

    fn sealed<E, S2>(self, end: E) -> SpecIteratorBuilder<Tz, BDP, START, E, S2> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end,
            options: self.options,
            local: self.local,
            anchor: self.anchor,
            limits: self.limits,
            marker_sealed: PhantomData,
        }
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
            dtm,
            start: NoStart,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            options: Options::default(),
            local: None,
            anchor: None,
            limits: Limits::default(),
            marker_sealed: PhantomData,
        }
    }

    /// Resume a stored schedule: occurrences strictly after `last_actual`, the
    /// [`actual`](crate::Occurrence::actual) of the last occurrence you
    /// handled, in timezone `tz`.
    ///
    /// Store `actual` with its UTC offset (e.g. RFC 3339) and pass it back
    /// here. The offset keeps the scheduled wall-clock time of an occurrence
    /// moved by a DST gap, and tells fall-back's two 01:00s apart. Rebuild
    /// with the same options (`with_time_anchor`, `with_interval_mode`, …) as
    /// the original schedule. See the crate docs on
    /// [persisting and resuming](crate#persisting-and-resuming-a-schedule).
    pub fn resume_after(spec: &str, bdp: BDP, tz: Tz, last_actual: DateTime<FixedOffset>) -> Self {
        let mut builder = Self::new_after(spec, bdp, last_actual.with_timezone(&tz));
        builder.local = Some(last_actual.naive_local());
        builder
    }

    /// Resume a stored schedule from its original `start`: occurrences
    /// strictly after `last_actual`, positioned as if iterating from `start`.
    ///
    /// Required for a union spec (`A | B`), whose parts would otherwise take
    /// their dates or times from another part's occurrence. For a plain spec
    /// it gives the same results as [`resume_after`](Self::resume_after). A
    /// union in [`IntervalMode::Elapsed`] cannot be resumed
    /// ([`Error::UnsupportedResume`]). Pass both datetimes as stored, with
    /// their UTC offsets, and rebuild with the same options.
    pub fn resume_with_start(
        spec: &str,
        bdp: BDP,
        tz: Tz,
        start: DateTime<FixedOffset>,
        last_actual: DateTime<FixedOffset>,
    ) -> Self {
        let mut builder = Self::resume_after(spec, bdp, tz, last_actual);
        builder.anchor = Some(start.naive_local());
        builder.limits.resume_start = Some(start);
        builder
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        self.finish(false, None)
    }
}

// --- no-start, with-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    /// Bound the iterator by an explicit end datetime (inclusive).
    ///
    /// Useful with [`resume_after`](Self::resume_after) for a stored schedule
    /// that has an end.
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, EndDateTime<Tz>, Sealed> {
        self.sealed(EndDateTime(end))
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let end = self.end.0.with_timezone(&self.dtm.timezone());
        self.finish(false, Some(end))
    }
}

// --- with-start, no-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    /// Create an iterator where `start` is the **first yielded item**.
    ///
    /// `start` is yielded as-is, as [`Occurrence::Exact`], even when it does
    /// not match the spec or would be adjusted by it. This lets you feed back
    /// an already-adjusted (observed) date:
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
            dtm: start.clone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            options: Options::default(),
            local: None,
            anchor: None,
            limits: Limits::default(),
            marker_sealed: PhantomData,
        }
    }

    /// Bound the iterator by an explicit end datetime (inclusive).
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        self.sealed(EndDateTime(end))
    }

    /// Bound the iterator by another datetime spec.
    ///
    /// The end is the first occurrence of `end_spec` strictly after `start`
    /// (its observed time), and is inclusive. For example
    /// `.with_end_spec("YY-MM-LT17:00:00")` stops at 17:00 on the last day of
    /// the start's month.
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
        self.sealed(EndSpec(end_spec.into()))
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        self.finish(true, None)
    }
}

// --- with-start, with-end ---
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let end = self.end.0.with_timezone(&self.dtm.timezone());
        self.finish(true, Some(end))
    }
}

// --- with-start, with-end-spec ---
impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let bdp = self.bd_processor.clone();
        let end = SpecIterator::new(&self.end.0, bdp, self.dtm.clone(), None, None, false, None, self.options)?
            .next()?
            .ok_or(Error::InvalidEndSpec)?
            .observed()
            .clone();
        self.finish(true, Some(end))
    }
}

// ---------------------------------------------------------------------------
// SpecIterator
// ---------------------------------------------------------------------------

/// Timezone-aware datetime recurrence iterator combining a date spec and a time spec.
///
/// Visits each date the date spec produces, in order, and yields every time
/// the time spec produces on that date (on the *observed* date when a `~`
/// adjustment moved it). Every time on an adjusted date reports the
/// adjustment: `actual` is the raw date at that time, `observed` the moved one.
///
/// Use [`SpecIteratorBuilder`] to construct one.
#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    dates: DateNaiveSpecIterator<BDP>,
    time_spec: TimeSpec,
    options: Options,
    /// Time of day of the start (or `new_after` cursor); the phase for
    /// [`TimeAnchor::Start`].
    start_time: NaiveTime,
    /// Wall-clock time of the start or `new_after` / `resume_after` cursor.
    initial_local: NaiveDateTime,
    /// The date whose times are being yielded.
    day: Option<Day<Tz>>,
    /// Whether a date has been taken from `dates` yet.
    first_date_taken: bool,
    /// Actual instant of the last result, or the initial cursor. Results must
    /// be strictly later, so DST collisions are dropped.
    cursor: DateTime<Tz>,
    /// Yielded first, then cleared.
    start: Option<DateTime<Tz>>,
    /// Inclusive upper bound.
    end: Option<DateTime<Tz>>,
    done: bool,
    /// For a union spec (`A | B`), its parts; the other fields are unused.
    union: Option<Box<Parts<Tz, BDP>>>,
    /// The exclusion spec after a `!`: occurrences whose actual date matches
    /// are skipped.
    exclusions: Option<DateNaiveSpecIterator<BDP>>,
    /// `with_count`: results left.
    remaining: Option<usize>,
}

/// The parts of a union spec, merged by actual time.
type Parts<Tz, BDP> = Merge<SpecIterator<Tz, BDP>, DateTime<Tz>>;

fn actual<Tz: TimeZone>(occurrence: &Occurrence<DateTime<Tz>>) -> DateTime<Tz> {
    occurrence.actual().clone()
}

#[derive(Debug, Clone)]
struct Day<Tz: TimeZone> {
    /// The date-level occurrence (raw date, observed date).
    occurrence: Occurrence<NaiveDateTime>,
    /// The observed date: times are yielded on it.
    date: NaiveDate,
    /// Local time of the last time searched on this date.
    local: NaiveDateTime,
    /// Observed instant of the last time yielded on this date.
    prev: Option<DateTime<Tz>>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIterator<Tz, BDP> {
    /// `cursor` is the start when `include_start`, else the `new_after` point.
    /// `local` overrides the cursor's wall-clock time (see `resume_after`);
    /// `anchor` is a stored start to position dates and times from (see
    /// `resume_with_start`).
    #[allow(clippy::too_many_arguments)]
    fn new(
        spec: &str,
        bdp: BDP,
        cursor: DateTime<Tz>,
        local: Option<NaiveDateTime>,
        anchor: Option<NaiveDateTime>,
        include_start: bool,
        end: Option<DateTime<Tz>>,
        options: Options,
    ) -> Result<Self> {
        if let (spec, Some(exclusions)) = split_exclusions(spec) {
            let mut iter = Self::new(spec, bdp.clone(), cursor.clone(), local, anchor, include_start, end, options)?;
            // Cadences in the exclusion spec count from the start.
            let from = anchor.unwrap_or_else(|| local.unwrap_or_else(|| cursor.naive_local()));
            iter.exclusions = Some(DateNaiveSpecIterator::exclusion(exclusions, bdp, from)?);
            return Ok(iter);
        }
        if let Some(parts) = union_parts(spec) {
            match anchor {
                None if local.is_some() => {
                    return Err(Error::UnsupportedResume(
                        "a union spec must be resumed with `resume_with_start`".into(),
                    ))
                }
                Some(_) if options.interval_mode == IntervalMode::Elapsed => {
                    return Err(Error::UnsupportedResume("a union spec in `IntervalMode::Elapsed`".into()))
                }
                _ => {}
            }
            let parts = parts
                .into_iter()
                .map(|part| {
                    let (bdp, cursor, end) = (bdp.clone(), cursor.clone(), end.clone());
                    Self::new(part, bdp, cursor, local, anchor, include_start, end, options)
                })
                .collect::<Result<Vec<_>>>()?;
            let mut outer = parts[0].clone();
            outer.union = Some(Box::new(Merge::new(parts, actual)));
            return Ok(outer);
        }
        let spec = Spec::from_str(spec)?;
        let local = local.unwrap_or_else(|| cursor.naive_local());
        // With a start, its date is already in progress, so the date
        // iterator resumes after it; otherwise the cursor's date is still a
        // candidate. Cadences (`nY`, `nM`) align to the cursor's date either way.
        let dates_after = if include_start {
            local.date().and_hms_nano_opt(23, 59, 59, 999_999_999)
        } else {
            local.date().and_hms_opt(0, 0, 0).map(|m| m - Duration::nanoseconds(1))
        }
        .ok_or(Error::IteratorNotConverged)?;

        let day = include_start.then(|| Day {
            occurrence: Occurrence::Exact(local),
            date: local.date(),
            local,
            prev: Some(cursor.clone()),
        });
        let dates = match anchor {
            // Dates from the stored start's cadence, from the cursor's date on.
            Some(start) => DateNaiveSpecIterator::resumed(&spec.date_spec, bdp, start, dates_after, None)?,
            None => DateNaiveSpecIterator::new_after_anchored(&spec.date_spec, bdp, dates_after, local)?,
        };
        Ok(Self {
            tz: cursor.timezone(),
            dates,
            time_spec: spec.time_spec.parse()?,
            options,
            start_time: anchor.unwrap_or(local).time(),
            initial_local: local,
            day,
            first_date_taken: include_start,
            start: include_start.then(|| cursor.clone()),
            cursor,
            end,
            done: false,
            union: None,
            exclusions: None,
            remaining: None,
        })
    }



    /// The next time on the current date, or `None` once the date is exhausted.
    fn next_on_day(&mut self) -> Result<Option<Occurrence<DateTime<Tz>>>> {
        let Some(day) = self.day.as_mut() else {
            return Ok(None);
        };
        let policy = self.options.dst_policy;

        if let (IntervalMode::Elapsed, Some(prev)) = (self.options.interval_mode, &day.prev) {
            // Step in the previous time's offset so the step is real time.
            let local = prev.naive_local();
            let offset = Duration::seconds(i64::from(prev.offset().fix().local_minus_utc()));
            let Some(next) = crate::time::next_after(&self.time_spec, local, local) else {
                return Ok(None);
            };
            let instant = self.tz.from_utc_datetime(&(next - offset));
            if instant.naive_local().date() != day.date {
                return Ok(None);
            }
            day.prev = Some(instant.clone());
            let occurrence = match tick(&day.occurrence, instant.naive_local()) {
                Occurrence::Exact(_) => Occurrence::Exact(instant),
                Occurrence::AdjustedLater(actual, _) => {
                    Occurrence::AdjustedLater(resolve_local(&self.tz, actual, policy)?, instant)
                }
                Occurrence::AdjustedEarlier(actual, _) => {
                    Occurrence::AdjustedEarlier(resolve_local(&self.tz, actual, policy)?, instant)
                }
            };
            return Ok(Some(occurrence));
        }

        let anchor = match self.options.time_anchor {
            TimeAnchor::Start => day.date.and_time(self.start_time),
            TimeAnchor::Midnight => day.date.and_time(NaiveTime::MIN),
        };
        let Some(next) = crate::time::next_after(&self.time_spec, anchor, day.local) else {
            return Ok(None);
        };
        if next.date() != day.date {
            return Ok(None);
        }
        day.local = next;
        let occurrence = next_result_to_tz(&self.tz, tick(&day.occurrence, next), policy)?;
        day.prev = Some(occurrence.observed().clone());
        Ok(Some(occurrence))
    }

    /// Move to the next date from the date spec; `false` when there are none.
    fn next_day(&mut self) -> Result<bool> {
        let Some(occurrence) = self.dates.next()? else {
            return Ok(false);
        };
        let date = occurrence.observed().date();
        let midnight = date.and_time(NaiveTime::MIN);
        // On the `new_after` date only times after the cursor are candidates,
        // and `Elapsed` steps continue from the cursor itself.
        let on_cursor_date = !self.first_date_taken && date == self.initial_local.date();
        let (local, prev) = if on_cursor_date {
            (self.initial_local, Some(self.cursor.clone()))
        } else {
            (midnight - Duration::nanoseconds(1), None)
        };
        self.first_date_taken = true;
        self.day = Some(Day { occurrence, date, local, prev });
        Ok(true)
    }
}

/// The date-level occurrence carried to one time `t` on its observed date.
fn tick(day: &Occurrence<NaiveDateTime>, t: NaiveDateTime) -> Occurrence<NaiveDateTime> {
    match day {
        Occurrence::Exact(_) => Occurrence::Exact(t),
        Occurrence::AdjustedLater(actual, _) => {
            Occurrence::AdjustedLater(actual.date().and_time(t.time()), t)
        }
        Occurrence::AdjustedEarlier(actual, _) => {
            Occurrence::AdjustedEarlier(actual.date().and_time(t.time()), t)
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDP> {
    type Item = Occurrence<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if self.remaining == Some(0) {
            return Ok(None);
        }
        loop {
            let Some(occurrence) = self.next_unlimited()? else {
                return Ok(None);
            };
            if let Some(exclusions) = &self.exclusions {
                if exclusions.matches_date(occurrence.actual().naive_local().date())? {
                    continue;
                }
            }
            if let Some(n) = &mut self.remaining {
                *n -= 1;
            }
            return Ok(Some(occurrence));
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIterator<Tz, BDP> {
    /// The next result before exclusions and `with_count`.
    fn next_unlimited(&mut self) -> Result<Option<Occurrence<DateTime<Tz>>>> {
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
            return Ok(Some(Occurrence::Exact(start)));
        }

        loop {
            let Some(occurrence) = self.next_on_day()? else {
                if !self.next_day()? {
                    self.done = true;
                    return Ok(None);
                }
                continue;
            };
            if *occurrence.actual() <= self.cursor {
                continue;
            }
            if let Some(end) = &self.end {
                if occurrence.actual() > end || occurrence.observed() > end {
                    self.done = true;
                    return Ok(None);
                }
            }
            self.cursor = occurrence.actual().clone();
            return Ok(Some(occurrence));
        }
    }
}

// ---------------------------------------------------------------------------
// NaiveSpecIterator
// ---------------------------------------------------------------------------

/// Naive (non-timezone-aware) datetime recurrence iterator.
///
/// Behaves like [`SpecIterator`] in UTC, with default options.
#[derive(Debug)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor>(SpecIterator<Utc, BDP>);

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    /// Iterate occurrences strictly after `dtm`.
    pub fn new_after(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        SpecIteratorBuilder::new_after(spec, bdp, dtm.and_utc()).build().map(Self)
    }

    /// Yield `start` first, then iterate forward.
    pub fn new_with_start(spec: &str, bdp: BDP, start: NaiveDateTime) -> Result<Self> {
        SpecIteratorBuilder::new_with_start(spec, bdp, start.and_utc()).build().map(Self)
    }

    /// Like [`new_with_start`](Self::new_with_start), up to and including `end`.
    pub fn new_with_end(spec: &str, bdp: BDP, start: NaiveDateTime, end: NaiveDateTime) -> Result<Self> {
        SpecIteratorBuilder::new_with_start(spec, bdp, start.and_utc())
            .with_end(end.and_utc())
            .build()
            .map(Self)
    }
}

impl<BDP: BizDayProcessor> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = Occurrence<NaiveDateTime>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        Ok(self.0.next()?.map(|occ| match occ {
            Occurrence::Exact(t) => Occurrence::Exact(t.naive_utc()),
            Occurrence::AdjustedLater(a, o) => Occurrence::AdjustedLater(a.naive_utc(), o.naive_utc()),
            Occurrence::AdjustedEarlier(a, o) => Occurrence::AdjustedEarlier(a.naive_utc(), o.naive_utc()),
        }))
    }
}
