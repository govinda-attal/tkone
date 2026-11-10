use super::spec::Spec;
use crate::biz_day::BizDayProcessor;
use crate::date::NaiveSpecIterator as DateNaiveSpecIterator;
use crate::limits::{skip_through, Limits};
use crate::prelude::*;
use crate::time::Spec as TimeSpec;
use crate::utils::{just_before, next_result_to_tz, resolve_local};
use crate::{DstPolicy, IntervalMode, Occurrence, TimeAnchor};
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};
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
/// | `new_with_start(spec, bdp, start)` | The occurrences from `start` (which anchors cadences and time steps) | anchor to a fixed start datetime |
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
    /// Date specs given to `with_exclusions`.
    exclusions: Vec<String>,
    marker_sealed: PhantomData<(Tz, S)>,
}

#[derive(Debug, Clone, Copy, Default)]
struct Options {
    dst_policy: DstPolicy,
    interval_mode: IntervalMode,
    time_anchor: TimeAnchor,
    /// The cursor is just before an anchoring start, not a stored occurrence.
    fresh_start: bool,
}

impl<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> SpecIteratorBuilder<Tz, BDP, START, END, S> {
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

    /// Skip the occurrences whose actual date matches `spec`, a date spec:
    /// `.with_exclusions("YY-12-25")` skips every time on 25 December. Call
    /// it again to skip the dates of more specs. Cadences in `spec` are
    /// counted from the start; day steps (`7D`) are not allowed.
    pub fn with_exclusions(mut self, spec: impl Into<String>) -> Self {
        self.exclusions.push(spec.into());
        self
    }

    /// Stop after `n` results, counted from the start. Excluded occurrences
    /// are not counted.
    ///
    /// A counted schedule is resumed with `resume_with_start`, which replays
    /// it from its start to keep the count; `resume_after` returns
    /// [`Error::UnsupportedResume`].
    pub fn with_count(mut self, n: usize) -> Self {
        self.limits.count = Some(n);
        self
    }

    /// Build the iterator from `dtm` (the start when `include_start`), with
    /// `with_count` and `with_exclusions` applied. A resumed counted schedule
    /// is replayed from its stored start instead.
    fn finish(self, include_start: bool, end: Option<DateTime<Tz>>) -> Result<SpecIterator<Tz, BDP>> {
        let bdp = self.bd_processor;
        // A start anchors the rule: results are its occurrences at or after
        // the start, not the start itself as-is.
        let options = self.options;
        let count = self.limits.count;
        let body = self.spec.as_str();
        // Cadences in exclusions count from the start, else the cursor.
        let from = self.anchor.unwrap_or_else(|| self.local.unwrap_or_else(|| self.dtm.naive_local()));
        let exclusions = self
            .exclusions
            .iter()
            .map(|spec| DateNaiveSpecIterator::exclusion(spec, bdp.clone(), from))
            .collect::<Result<Vec<_>>>()?;
        let finished = |mut iter: SpecIterator<Tz, BDP>| {
            iter.remaining = count;
            iter.exclusions = exclusions.clone();
            iter
        };
        let from_start = |start: DateTime<Tz>, bdp: BDP| {
            let options = Options { fresh_start: true, ..options };
            SpecIterator::new(
                body,
                bdp,
                just_before(&start),
                None,
                Some(start.naive_local()),
                false,
                end.clone(),
                options,
            )
        };
        if let Some(start) = self.limits.replay_from(self.local)? {
            let continuous = from_start(start.with_timezone(&self.dtm.timezone()), bdp)?;
            return skip_through(finished(continuous), &self.dtm, actual);
        }
        if include_start {
            return Ok(finished(from_start(self.dtm.clone(), bdp)?));
        }
        let iter = SpecIterator::new(body, bdp, self.dtm.clone(), self.local, self.anchor, false, end, options)?;
        Ok(finished(iter))
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
            exclusions: self.exclusions,
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
            exclusions: Vec::new(),
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
    /// Required for a cycle with positions (`/4W[1,2]`, `8D[1-4]`) and for a
    /// schedule with [`with_count`](Self::with_count); otherwise it gives the
    /// same results as [`resume_after`](Self::resume_after). Pass both
    /// datetimes as stored, with their UTC offsets, and rebuild with the same
    /// options.
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
    pub fn with_end(self, end: DateTime<Tz>) -> SpecIteratorBuilder<Tz, BDP, NoStart, EndDateTime<Tz>, Sealed> {
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
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
    /// Create an iterator of the occurrences from `start`.
    ///
    /// `start` anchors the rule: cadences (`3M`, `2W`) count from it, time
    /// steps take their phase from it, and it is the earliest a result's
    /// *actual* date can be. It is a result only if the rule gives it,
    /// adjusted like any other: `YY-MM-DD~NB` from a Saturday gives the
    /// Saturday moved to Monday, and `YY-MM-15` from the 3rd gives the 15th.
    ///
    /// To continue from an occurrence found with `new_after`, pass its
    /// `actual()` date: the series then starts with that same occurrence,
    /// whichever way its adjustment moved it.:
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
    /// // Step 1: the next occurrence after now
    /// let first = SpecIteratorBuilder::new_after("YY-1M-L~WT11:00:00", bdp.clone(), now)
    ///     .build().unwrap()
    ///     .next().unwrap().unwrap();
    ///
    /// // Step 2: the series from its actual date starts with that occurrence,
    /// // whether `~W` moved it earlier (Saturday) or later (Sunday).
    /// let iter = SpecIteratorBuilder::new_with_start("YY-1M-L~WT11:00:00", bdp, *first.actual())
    ///     .build().unwrap();
    ///
    /// let results: Vec<_> = iter.take(3).collect().unwrap();
    /// assert_eq!(results[0], first);
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
            exclusions: Vec::new(),
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
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let end = self.end.0.with_timezone(&self.dtm.timezone());
        self.finish(true, Some(end))
    }
}

// --- with-start, with-end-spec ---
impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
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
    /// The `with_exclusions` specs: occurrences whose actual date matches one
    /// are skipped.
    exclusions: Vec<DateNaiveSpecIterator<BDP>>,
    /// `with_count`: results left.
    remaining: Option<usize>,
    /// The business-day processor's code and time zone, to check that each
    /// result's date is the same there (see [`BizDayProcessor::time_zone`]).
    calendar: Option<(String, chrono_tz::Tz)>,
}

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
    /// A rule anchored at `start`: cadences and the time of day count from
    /// `start`, which is a result only if the rule produces it. Results have actual times at or after
    /// `start` and strictly after `after`. For [`crate::schedule`].
    pub(crate) fn anchored(
        rule: &str,
        bdp: BDP,
        start: &DateTime<Tz>,
        after: &DateTime<Tz>,
        options: (DstPolicy, IntervalMode, TimeAnchor),
    ) -> Result<Self> {
        let (dst_policy, interval_mode, time_anchor) = options;
        let fresh = after < start;
        let options = Options { dst_policy, interval_mode, time_anchor, fresh_start: fresh };
        let cursor = if fresh { just_before(start) } else { after.clone() };
        Self::new(rule, bdp, cursor, None, Some(start.naive_local()), false, None, options)
    }

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
        let calendar = bdp.time_zone().map(|zone| (bdp.code().unwrap_or_else(|| "the calendar".into()), zone));
        let spec = Spec::from_str(spec)?;
        if local.is_some() && anchor.is_none() && crate::date::needs_start(&spec.date_spec)? {
            return Err(crate::date::resume_needs_start());
        }
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
            exclusions: Vec::new(),
            remaining: None,
            calendar,
        })
    }

    /// Fails if `occurrence` falls on another date in the calendar's own time
    /// zone: its business day would be judged on the wrong day.
    fn check_calendar_date(&self, occurrence: &Occurrence<DateTime<Tz>>) -> Result<()> {
        match &self.calendar {
            Some((code, zone)) => check_calendar_date(code, zone, occurrence.observed()),
            None => Ok(()),
        }
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
        // and `Elapsed` steps continue from the cursor itself. Just before an
        // anchoring start there is no previous occurrence, and every time of
        // the date is a candidate: those whose actual time is before the
        // start are dropped below, but one moved there from a later date is
        // kept, so a schedule can report it (`moved_before_start`).
        let on_cursor_date = !self.first_date_taken && !self.options.fresh_start && date == self.initial_local.date();
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

/// Fails if `at` is on another date in `zone`, the time zone `code`'s
/// business days are counted in: they would be judged on the wrong day.
pub(crate) fn check_calendar_date<T: TimeZone>(code: &str, zone: &chrono_tz::Tz, at: &DateTime<T>) -> Result<()> {
    let there = at.with_timezone(zone);
    if there.date_naive() == at.date_naive() {
        return Ok(());
    }
    Err(Error::Calendar(format!(
        "{} is {} in {}, where {code}'s business days are counted; read the schedule in that time zone",
        at.naive_local().format("%Y-%m-%d %H:%M"),
        there.format("%Y-%m-%d %H:%M"),
        zone.name(),
    )))
}

/// The date-level occurrence carried to one time `t` on its observed date.
fn tick(day: &Occurrence<NaiveDateTime>, t: NaiveDateTime) -> Occurrence<NaiveDateTime> {
    match day {
        Occurrence::Exact(_) => Occurrence::Exact(t),
        Occurrence::AdjustedLater(actual, _) => Occurrence::AdjustedLater(actual.date().and_time(t.time()), t),
        Occurrence::AdjustedEarlier(actual, _) => Occurrence::AdjustedEarlier(actual.date().and_time(t.time()), t),
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
            if self.excluded(occurrence.actual().naive_local().date())? {
                continue;
            }
            self.check_calendar_date(&occurrence)?;
            if let Some(n) = &mut self.remaining {
                *n -= 1;
            }
            return Ok(Some(occurrence));
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIterator<Tz, BDP> {
    fn excluded(&self, date: NaiveDate) -> Result<bool> {
        for exclusion in &self.exclusions {
            if exclusion.matches_date(date)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The next result before exclusions and `with_count`.
    fn next_unlimited(&mut self) -> Result<Option<Occurrence<DateTime<Tz>>>> {
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
        SpecIteratorBuilder::new_with_start(spec, bdp, start.and_utc()).with_end(end.and_utc()).build().map(Self)
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
