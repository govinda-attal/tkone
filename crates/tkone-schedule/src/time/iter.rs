use std::marker::PhantomData;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Offset, TimeZone, Utc};

use fallible_iterator::FallibleIterator;

use super::{engine, spec::Spec};
use crate::bounds::{just_before, split_bounds};
use crate::limits::{skip_through, Limits};
use crate::merge::{add_exclusions, split_exclusions, union_parts, Merge};
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
    limits: Limits,
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

    /// Skip the times that match `spec`, another time spec (a union is
    /// allowed): `.with_exclusions("12:MM:SS")` skips the 12:00 hour. The
    /// same as writing `! spec` at the end of the schedule's spec, and
    /// combined with any exclusions already there. Steps in `spec` are
    /// counted from the start.
    pub fn with_exclusions(mut self, spec: impl AsRef<str>) -> Self {
        self.spec = add_exclusions(&self.spec, spec.as_ref());
        self
    }

    /// Stop after `n` results, counted from the start (a `new_with_start`
    /// start is the first). Excluded times are not counted.
    ///
    /// A counted schedule is resumed with `resume_with_start`, which replays
    /// it from its start to keep the count; `resume_after` returns
    /// [`Error::UnsupportedResume`].
    pub fn with_count(mut self, n: usize) -> Self {
        self.limits.count = Some(n);
        self
    }

    /// Build the iterator for every builder, from `dtm` (the start when
    /// `given_start`).
    ///
    /// - The start is the builder's (`new_with_start`, yielded as-is first;
    ///   `resume_with_start`), else one written in the spec (`@…`: the
    ///   grid's anchor and an inclusive lower bound), else none.
    /// - The end is the builder's (`with_end`, `with_end_spec`), else one
    ///   written in the spec (`..…`).
    /// - With `with_count`, a resumed schedule is replayed from its start.
    fn assemble(&self, given_start: bool, end: BuilderEnd<Tz>) -> Result<SpecIterator<Tz>> {
        let (body, bounds) = split_bounds(&self.spec)?;
        let tz = &self.timezone;
        let policy = self.dst_policy;
        let end = match end {
            BuilderEnd::At(end) => Some(end.with_timezone(tz)),
            BuilderEnd::Spec(end_spec) => Some(end_from_spec(&end_spec, &self.dtm, policy)?),
            BuilderEnd::None => bounds.end_in(tz, policy)?,
        };
        // A builder count (`with_count`) takes precedence over one written (`xN`).
        let limits = Limits { count: self.limits.count.or(bounds.count()), ..self.limits.clone() };
        let count = limits.count;
        let configured = |iter: SpecIterator<Tz>| {
            let mut iter = iter.with_interval_mode(self.interval_mode).with_time_anchor(self.time_anchor);
            iter.remaining = count;
            iter
        };
        if given_start {
            let start = Some(self.dtm.clone());
            return Ok(configured(SpecIterator::new(body, self.dtm.clone(), start, end, policy)?));
        }
        if let Some(start) = limits.replay_from(self.local)? {
            // A counted `resume_with_start`: replay from the start.
            let start = start.with_timezone(tz);
            let continuous = configured(SpecIterator::new(body, start.clone(), Some(start), end, policy)?);
            return skip_through(continuous, &self.dtm, instant);
        }
        if self.anchor.is_none() {
            if let Some(start) = bounds.start_in(tz, policy)? {
                return self.with_text_start(body, start, end, count);
            }
        }
        let iter = configured(SpecIterator::new(body, self.dtm.clone(), None, end, policy)?);
        let mut iter = resumed(iter, body, self.interval_mode, self.local, self.anchor)?;
        // Steps in exclusion specs count from the stored start, if any.
        if let Some(anchor) = self.anchor {
            iter.exclusion_anchor = anchor;
        }
        Ok(iter)
    }

    /// A start written in the spec: the grid's anchor, and results at or
    /// after it.
    fn with_text_start(
        &self,
        body: &str,
        start: DateTime<Tz>,
        end: Option<DateTime<Tz>>,
        count: Option<usize>,
    ) -> Result<SpecIterator<Tz>> {
        let anchor = start.naive_local();
        let policy = self.dst_policy;
        let fresh = |cursor: DateTime<Tz>| -> Result<SpecIterator<Tz>> {
            let mut iter = SpecIterator::new(body, cursor, None, end.clone(), policy)?
                .with_interval_mode(self.interval_mode)
                .with_time_anchor(self.time_anchor);
            iter.remaining = count;
            iter.exclusion_anchor = anchor;
            iter.each_part(|part| {
                part.anchor = Some(anchor);
                part.today = Some(anchor.date());
                // In `Elapsed`, later steps measure real time from each result.
                part.anchor_once = self.interval_mode == IntervalMode::Elapsed;
            });
            Ok(iter)
        };
        if count.is_some() {
            return skip_through(fresh(just_before(&start))?, &self.dtm, instant);
        }
        if self.dtm < start {
            return fresh(just_before(&start));
        }
        // After the start: resume on the grid counted from it.
        let iter = SpecIterator::new(body, self.dtm.clone(), None, end, policy)?
            .with_interval_mode(self.interval_mode)
            .with_time_anchor(self.time_anchor);
        let mut iter = resumed(iter, body, self.interval_mode, self.local, Some(anchor))?;
        iter.exclusion_anchor = anchor;
        iter.remaining = count;
        Ok(iter)
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
            limits: Limits::default(),
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
        builder.limits.resume_start = Some(start);
        builder
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        self.assemble(false, BuilderEnd::None)
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
            limits: self.limits,
            marker_sealed: PhantomData,
        }
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, NoStart, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        self.assemble(false, BuilderEnd::At(self.end.0.clone()))
    }
}

impl SpecIteratorBuilder<chrono_tz::Tz, NoStart, NoEnd, NotSealed> {
    /// Build from the spec's own text alone: its start (`@…`), end (`..…`)
    /// and time zone (`[Europe/London]`, UTC if none is named). Without a
    /// start, results begin after now.
    ///
    /// ```rust
    /// use tkone_schedule::time::SpecIteratorBuilder;
    /// use fallible_iterator::FallibleIterator;
    ///
    /// let times: Vec<_> = SpecIteratorBuilder::from_spec("2H:00:00 @2025-01-06T09:00:00[Europe/London] ..2025-01-06T15:00")?
    ///     .build()?
    ///     .map(|t| Ok(t.format("%H:%M %Z").to_string()))
    ///     .collect()?;
    /// assert_eq!(times, ["09:00 GMT", "11:00 GMT", "13:00 GMT", "15:00 GMT"]);
    /// # Ok::<(), tkone_schedule::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if the start, end or time zone is malformed.
    pub fn from_spec(spec: &str) -> Result<Self> {
        let (_, bounds) = split_bounds(spec)?;
        Ok(Self::new_after(spec, bounds.cursor()?))
    }
}

/// An end given to the builder.
enum BuilderEnd<Tz: TimeZone> {
    None,
    At(DateTime<Tz>),
    Spec(String),
}

/// The first time of `end_spec` (a union is allowed) after `start`, on any day:
/// `13:00:00` from a 14:00 start is 13:00 the next day.
fn end_from_spec<Tz: TimeZone>(end_spec: &str, start: &DateTime<Tz>, policy: DstPolicy) -> Result<DateTime<Tz>> {
    let local = start.naive_local();
    let mut end = None;
    for part in union_parts(end_spec).unwrap_or_else(|| vec![end_spec]) {
        let next = engine::next_after(&part.parse()?, local, local).ok_or(Error::InvalidEndSpec)?;
        end = Some(end.map_or(next, |e: NaiveDateTime| e.min(next)));
    }
    resolve_local(&start.timezone(), end.ok_or(Error::InvalidEndSpec)?, policy)
}

/// Apply any `resume_after` / `resume_with_start` cursor.
fn resumed<Tz: TimeZone>(
    iter: SpecIterator<Tz>,
    spec: &str,
    mode: IntervalMode,
    local: Option<NaiveDateTime>,
    anchor: Option<NaiveDateTime>,
) -> Result<SpecIterator<Tz>> {
    let union = union_parts(split_exclusions(spec).0).is_some();
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
    let mut iter = iter;
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
            limits: Limits::default(),
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
            limits: self.limits,
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
            limits: self.limits,
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz>> {
        self.assemble(true, BuilderEnd::None)
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        self.assemble(true, BuilderEnd::At(self.end.0.clone()))
    }
}

impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
    pub fn build(self) -> Result<SpecIterator<Tz>> {
        self.assemble(true, BuilderEnd::Spec(self.end.0.clone()))
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
    /// The exclusion specs after a `!`: matching times are skipped.
    exclusions: Vec<Spec>,
    /// Where steps in the exclusion specs are counted from.
    exclusion_anchor: NaiveDateTime,
    /// `with_count`: results left.
    remaining: Option<usize>,
    /// The day whose fixed times a spec without a step yields (the start's
    /// day for a start written in the spec); `None` means the cursor's day.
    today: Option<NaiveDate>,
    /// Use `anchor` for the first result only (`Elapsed` from a written start).
    anchor_once: bool,
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
        if let (spec, Some(exclusions)) = split_exclusions(spec) {
            let mut iter = Self::new(spec, cursor, start, end, dst_policy)?;
            iter.exclusions = union_parts(exclusions)
                .unwrap_or_else(|| vec![exclusions])
                .into_iter()
                .map(str::parse)
                .collect::<Result<Vec<Spec>>>()?;
            return Ok(iter);
        }
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
        let cursor_local = cursor.naive_local();
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
            exclusions: Vec::new(),
            exclusion_anchor: cursor_local,
            remaining: None,
            today: None,
            anchor_once: false,
        })
    }

    fn is_excluded(&self, t: &DateTime<Tz>) -> bool {
        let local = t.naive_local();
        let before = local - Duration::nanoseconds(1);
        self.exclusions
            .iter()
            .any(|spec| engine::next_after(spec, self.exclusion_anchor, before) == Some(local))
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
        if self.remaining == Some(0) {
            return Ok(None);
        }
        loop {
            let Some(t) = self.next_unlimited()? else {
                return Ok(None);
            };
            if self.is_excluded(&t) {
                continue;
            }
            if let Some(n) = &mut self.remaining {
                *n -= 1;
            }
            return Ok(Some(t));
        }
    }
}

impl<Tz: TimeZone> SpecIterator<Tz> {
    /// The next result before exclusions and `with_count`.
    fn next_unlimited(&mut self) -> Result<Option<DateTime<Tz>>> {
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
            let today = self.today.unwrap_or(self.local.date());
            if !self.daily && !engine::has_step(&self.spec) && next.date() != today {
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
            if self.anchor_once {
                self.anchor = None;
                self.anchor_once = false;
            }
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
        SpecIteratorBuilder::new_after(spec, dtm.and_utc()).build().map(Self)
    }

    pub fn new_with_start(spec: &str, start: NaiveDateTime) -> Result<Self> {
        SpecIteratorBuilder::new_with_start(spec, start.and_utc()).build().map(Self)
    }

    pub fn new_with_end(spec: &str, start: NaiveDateTime, end: NaiveDateTime) -> Result<Self> {
        SpecIteratorBuilder::new_with_start(spec, start.and_utc()).with_end(end.and_utc()).build().map(Self)
    }

    pub fn new_with_end_spec(spec: &str, start: NaiveDateTime, end_spec: &str) -> Result<Self> {
        SpecIteratorBuilder::new_with_start(spec, start.and_utc()).with_end_spec(end_spec).build().map(Self)
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
        let spec_ter = SpecIteratorBuilder::new_after("HH:30M:00", dt).build().unwrap();
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
        let spec_ter = SpecIteratorBuilder::new_after("1H:MM:00", dtm).build().unwrap();
        dbg!(spec_ter.take(5).collect::<Vec<DateTime<_>>>().unwrap());
    }

    #[test]
    fn test_time_spec_with_end_spec() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 3, 11, 23, 0, 0).unwrap();
        dbg!(&dtm);

        let spec_iter = SpecIteratorBuilder::new_with_start("3H:00:00", dtm).with_end_spec("15H:00:00").build().unwrap();

        let tmp = spec_iter.collect::<Vec<DateTime<_>>>().unwrap();
        dbg!(tmp);
    }

    #[test]
    fn test_time_spec_with_utc() {
        let start = Utc.with_ymd_and_hms(2024, 3, 31, 10, 0, 0).unwrap();
        let iter = SpecIteratorBuilder::new_with_start("1H:00:00", start).build().unwrap();
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
        let iter = SpecIteratorBuilder::new_with_start("HH:MM:SS", start).build().unwrap();
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
        let iter = SpecIteratorBuilder::new_with_start("HH:MM:00", start).build().unwrap();
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
        let iter = SpecIteratorBuilder::new_with_start("HH:00:00", start).build().unwrap();
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
        let iter = SpecIteratorBuilder::new_with_start("1H:MM:SS", start).build().unwrap();
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
