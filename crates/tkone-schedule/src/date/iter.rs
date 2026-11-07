use super::{engine, spec::{DayCycle, Spec}};
use crate::limits::{skip_through, Limits};
use crate::utils::next_result_to_tz;
use crate::{biz_day::BizDayProcessor, prelude::*, DstPolicy, Occurrence};
use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use std::marker::PhantomData;

// --- Builder type-state markers ---
// These zero-sized types encode the builder's configuration at compile time so
// that only valid combinations (e.g. you must supply a start before you can
// supply an end) are callable.
pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

/// Fluent, type-state builder for [`SpecIterator`] and [`NaiveSpecIterator`].
///
/// The builder uses phantom type parameters to enforce at *compile time* that
/// you call methods in a valid order:
///
/// - Every builder can be `.build()`-ed immediately.
/// - `with_end` is available after `new_after` / `resume_after` and after
///   `new_with_start`; `with_end_spec` only after `new_with_start` (an end
///   spec is resolved from the start).
///
/// # Construction variants
///
/// | Constructor | First result | Use when… |
/// |-------------|-------------|-----------|
/// | `new(spec, bdp, tz)` | Next occurrence after `Utc::now()` | open-ended schedule from now |
/// | `new_after(spec, bdp, dtm)` | First occurrence **after** `dtm` | schedule from a known cursor |
/// | `new_with_start(spec, bdp, start)` | `start` itself is the first item | anchor to a fixed start date |
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::biz_day::WeekendSkipper;
/// use tkone_schedule::date::SpecIteratorBuilder;
/// use tkone_schedule::Occurrence;
/// use chrono::{TimeZone, Utc, Duration};
/// use fallible_iterator::FallibleIterator;
///
/// let bdp   = WeekendSkipper::new();
/// let start = Utc.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap();
/// let end   = Utc.with_ymd_and_hms(2024, 6, 30, 0, 0, 0).unwrap();
///
/// // Last day of every month, starting on 2024-01-31, bounded until end of June
/// let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
///     .with_end(end)
///     .build()
///     .unwrap();
///
/// let dates: Vec<_> = iter.collect().unwrap();
/// // → 2024-01-31, 2024-02-29, 2024-03-31, 2024-04-30, 2024-05-31, 2024-06-30 (end is inclusive)
/// ```
pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    bd_processor: BDP,
    end: END,
    timezone: Tz,
    dst_policy: DstPolicy,
    /// Wall-clock cursor given to `resume_after`; `None` means `dtm`.
    local: Option<NaiveDateTime>,
    /// Wall-clock start given to `resume_with_start`.
    anchor: Option<NaiveDateTime>,
    limits: Limits,
    /// Date specs given to `with_exclusions`.
    exclusions: Vec<String>,
    marker_sealed: PhantomData<S>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor, START, END, S>
    SpecIteratorBuilder<Tz, BDP, START, END, S>
{
    /// Override the DST resolution policy for this iterator.
    ///
    /// Defaults to [`DstPolicy::Adjust`]. Set to [`DstPolicy::Strict`] to
    /// receive [`Error::AmbiguousLocalTime`] whenever a scheduled datetime
    /// falls in a DST gap or overlap instead of being silently adjusted.
    pub fn with_dst_policy(mut self, policy: DstPolicy) -> Self {
        self.dst_policy = policy;
        self
    }

    /// Skip the dates that match `spec`, another date spec, by actual date:
    /// `.with_exclusions("YY-12-25")`. Call it again to skip the dates of
    /// more specs. Cadences in `spec` (`3M`) are counted from the start; day
    /// steps (`7D`) are not allowed.
    pub fn with_exclusions(mut self, spec: impl Into<String>) -> Self {
        self.exclusions.push(spec.into());
        self
    }

    /// Stop after `n` results, counted from the start (a `new_with_start`
    /// start is the first). Excluded dates are not counted.
    ///
    /// A counted schedule is resumed with `resume_with_start`, which replays
    /// it from its start to keep the count; `resume_after` returns
    /// [`Error::UnsupportedResume`].
    pub fn with_count(mut self, n: usize) -> Self {
        self.limits.count = Some(n);
        self
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    /// Create an iterator that produces occurrences starting **from `Utc::now()`**
    /// in timezone `tz`. The current instant is excluded; the first result is the
    /// earliest occurrence strictly after now.
    pub fn new(
        spec: &str,
        bdp: BDP,
        tz: Tz,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder::new_after(spec, bdp, Utc::now().with_timezone(&tz))
    }

    /// Create an iterator that produces occurrences strictly **after `dtm`**.
    ///
    /// `dtm` itself is excluded. Use [`new_with_start`](SpecIteratorBuilder::new_with_start)
    /// if you want `dtm` to be the first yielded item.
    pub fn new_after(
        spec: &str,
        bdp: BDP,
        dtm: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            timezone: dtm.timezone(),
            dtm,
            start: NoStart,
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            dst_policy: DstPolicy::default(),
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
    /// moved by a DST gap (`2025-03-09T02:30:00-05:00`), which a plain
    /// instant would lose. See the crate docs on
    /// [persisting and resuming](crate#persisting-and-resuming-a-schedule).
    pub fn resume_after(
        spec: &str,
        bdp: BDP,
        tz: Tz,
        last_actual: DateTime<FixedOffset>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
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
    /// datetimes as stored, with their UTC offsets.
    pub fn resume_with_start(
        spec: &str,
        bdp: BDP,
        tz: Tz,
        start: DateTime<FixedOffset>,
        last_actual: DateTime<FixedOffset>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        let mut builder = Self::resume_after(spec, bdp, tz, last_actual);
        builder.anchor = Some(start.naive_local());
        builder.limits.resume_start = Some(start);
        builder
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let (spec, bdp, end) = (&self.spec, self.bd_processor, BuilderEnd::None);
        assemble(spec, bdp, self.dtm, self.local, false, self.anchor, end, &self.limits, &self.exclusions, self.dst_policy)
    }
}

/// Whether resuming `spec` from a stored occurrence needs the schedule's
/// start: a cycle with positions (see [`Spec::needs_start`]).
pub(crate) fn needs_start(spec: &str) -> Result<bool> {
    Ok(spec.parse::<Spec>()?.needs_start())
}

pub(crate) fn resume_needs_start() -> Error {
    Error::UnsupportedResume(
        "a cycle with positions (`/4W[1,2]`, `8D[1-4]`) must be resumed with `resume_with_start`".into(),
    )
}

/// An end given to the builder.
enum BuilderEnd<Tz: TimeZone> {
    None,
    At(DateTime<Tz>),
    Spec(String),
}

fn actual_instant<Tz: TimeZone>(occurrence: &Occurrence<DateTime<Tz>>) -> DateTime<Tz> {
    occurrence.actual().clone()
}

/// Build the iterator for every builder.
///
/// - The start is the builder's (`new_with_start`, yielded as-is first;
///   `resume_with_start`), else none (cadences count from the cursor).
/// - The end is the builder's (`with_end`, `with_end_spec`), if any.
/// - With `with_count`, a resumed schedule is replayed from its start.
#[allow(clippy::too_many_arguments)]
fn assemble<Tz: TimeZone, BDP: BizDayProcessor>(
    spec: &str,
    bdp: BDP,
    dtm: DateTime<Tz>,
    local: Option<NaiveDateTime>,
    given_start: bool,
    anchor: Option<NaiveDateTime>,
    end: BuilderEnd<Tz>,
    limits: &Limits,
    exclusions: &[String],
    dst_policy: DstPolicy,
) -> Result<SpecIterator<Tz, BDP>> {
    let tz = dtm.timezone();
    let wrap = |naive_spec_iter, last| SpecIterator {
        tz: tz.clone(),
        dst_policy,
        naive_spec_iter,
        last,
        remaining: limits.count,
    };
    let end = match end {
        BuilderEnd::At(end) => Some(end.with_timezone(&tz).naive_local()),
        BuilderEnd::Spec(end_spec) => {
            // A start-inclusive iterator would yield the start itself.
            let end = NaiveSpecIterator::new_after(&end_spec, bdp.clone(), dtm.naive_local())?
                .next()?
                .ok_or(Error::InvalidEndSpec)?;
            Some(*end.observed())
        }
        BuilderEnd::None => None,
    };
    let cursor = local.unwrap_or_else(|| dtm.naive_local());
    // Cadences in exclusions count from the start, else the cursor.
    let from = match (given_start, anchor) {
        (true, _) => dtm.naive_local(),
        (false, Some(start)) => start,
        (false, None) => cursor,
    };
    let excluding = |iter: NaiveSpecIterator<BDP>| iter.excluding(exclusions, from);

    if given_start {
        let start = dtm.naive_local();
        return Ok(wrap(excluding(NaiveSpecIterator::new(spec, bdp, start, start, Some(start), end)?)?, None));
    }
    if let Some(start) = anchor {
        // `resume_with_start`: positioned as if iterating from `start`.
        if limits.count.is_some() {
            let continuous = NaiveSpecIterator::new(spec, bdp, start, start, Some(start), end)?;
            return skip_through(wrap(excluding(continuous)?, None), &dtm, actual_instant);
        }
        let resumed = NaiveSpecIterator::resumed(spec, bdp, start, cursor, end, true)?;
        return Ok(wrap(excluding(resumed)?, Some(dtm)));
    }
    if local.is_some() && needs_start(spec)? {
        return Err(resume_needs_start());
    }
    limits.replay_from(local)?;
    Ok(wrap(excluding(NaiveSpecIterator::new(spec, bdp, cursor, cursor, None, end)?)?, Some(dtm)))
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    /// Bound the iterator by an explicit end datetime (inclusive).
    ///
    /// Useful with [`resume_after`](Self::resume_after) for a stored schedule
    /// that has an end.
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            dst_policy: self.dst_policy,
            local: self.local,
            anchor: self.anchor,
            limits: self.limits,
            exclusions: self.exclusions,
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, NoStart, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let (spec, bdp, end) = (&self.spec, self.bd_processor, BuilderEnd::At(self.end.0));
        assemble(spec, bdp, self.dtm, self.local, false, self.anchor, end, &self.limits, &self.exclusions, self.dst_policy)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let (spec, bdp, end) = (&self.spec, self.bd_processor, BuilderEnd::At(self.end.0));
        assemble(spec, bdp, self.dtm, None, true, None, end, &self.limits, &self.exclusions, self.dst_policy)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let (spec, bdp, end) = (&self.spec, self.bd_processor, BuilderEnd::Spec(self.end.0));
        assemble(spec, bdp, self.dtm, None, true, None, end, &self.limits, &self.exclusions, self.dst_policy)
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed>
{
    /// Create an iterator that includes `start` as its **first yielded item**.
    ///
    /// This is the right choice when you have already resolved a concrete start
    /// date (e.g. by calling `new_after(...).build()?.next()?`) and want the
    /// series to begin exactly there.
    ///
    /// The iterator alignment (for `NextNth` cycles) is anchored to `start`,
    /// so "every 3 months from `start`" always lands on the same day-of-month
    /// as `start`.
    ///
    /// # Example — derive start from spec, then iterate
    ///
    /// ```rust
    /// use tkone_schedule::biz_day::WeekendSkipper;
    /// use tkone_schedule::date::SpecIteratorBuilder;
    /// use tkone_schedule::Occurrence;
    /// use chrono::{TimeZone, Utc};
    /// use fallible_iterator::FallibleIterator;
    ///
    /// let bdp = WeekendSkipper::new();
    /// let now = Utc::now();
    ///
    /// // 1. Find the next occurrence to use as the start date
    /// let start = SpecIteratorBuilder::new_after("YY-1M-L", bdp.clone(), now)
    ///     .build()
    ///     .unwrap()
    ///     .next()
    ///     .unwrap()           // Result<Option<…>>
    ///     .unwrap()           // Option<…>
    ///     .observed()         // settlement date (post biz-day adjustment)
    ///     .clone();
    ///
    /// // 2. Build the recurring series anchored to that start date
    /// let iter = SpecIteratorBuilder::new_with_start("YY-1M-L", bdp, start)
    ///     .build()
    ///     .unwrap();
    ///
    /// let dates: Vec<_> = iter.take(3).collect().unwrap();
    /// assert_eq!(dates[0].observed(), &start);
    /// ```
    pub fn new_with_start(
        spec: &str,
        bdp: BDP,
        start: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, NoEnd, NotSealed> {
        SpecIteratorBuilder {
            dtm: start.clone(),
            timezone: start.timezone(),
            start: StartDateTime(start),
            spec: spec.to_string(),
            bd_processor: bdp,
            end: NoEnd,
            dst_policy: DstPolicy::default(),
            local: None,
            anchor: None,
            limits: Limits::default(),
            exclusions: Vec::new(),
            marker_sealed: PhantomData,
        }
    }

    /// Bound the iterator by another spec string.
    ///
    /// The end is the first occurrence of `end_spec` strictly after `start`
    /// (its observed date), and is inclusive.
    ///
    /// This is useful when the end boundary is itself expressed as a recurrence
    /// rule (e.g. "stop at the end of the current quarter").
    pub fn with_end_spec(
        self,
        end_spec: impl Into<String>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndSpec(end_spec.into()),
            dst_policy: self.dst_policy,
            local: self.local,
            anchor: self.anchor,
            limits: self.limits,
            exclusions: self.exclusions,
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

    /// Bound the iterator by an explicit end datetime.
    ///
    /// The end is inclusive: occurrences up to and including `end` are
    /// yielded. The iterator stops at the first occurrence whose *actual*
    /// (unadjusted) or *observed* (adjusted) date is after `end`.
    pub fn with_end(
        self,
        end: DateTime<Tz>,
    ) -> SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
        SpecIteratorBuilder {
            dtm: self.dtm,
            start: self.start,
            spec: self.spec,
            bd_processor: self.bd_processor,
            end: EndDateTime(end),
            dst_policy: self.dst_policy,
            local: self.local,
            anchor: self.anchor,
            limits: self.limits,
            exclusions: self.exclusions,
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let (spec, bdp, end) = (&self.spec, self.bd_processor, BuilderEnd::None);
        assemble(spec, bdp, self.dtm, None, true, None, end, &self.limits, &self.exclusions, self.dst_policy)
    }
}

/// Timezone-aware calendar-day recurrence iterator.
///
/// Yields [`Occurrence<DateTime<Tz>>`](crate::Occurrence) values so that
/// business-day adjustments are visible to the caller (see
/// [`Occurrence::actual`] vs [`Occurrence::observed`]).
///
/// Construct via [`SpecIteratorBuilder`]. Implements
/// [`fallible_iterator::FallibleIterator`].
///
/// # Example
///
/// ```rust
/// use tkone_schedule::biz_day::WeekendSkipper;
/// use tkone_schedule::date::SpecIteratorBuilder;
/// use tkone_schedule::Occurrence;
/// use chrono::{offset::TimeZone, DateTime};
/// use chrono_tz::America::New_York;
/// use fallible_iterator::FallibleIterator;
///
/// // Last day of every month, starting 2024-01-31 (EST)
/// let start = New_York.with_ymd_and_hms(2024, 1, 31, 0, 0, 0).unwrap();
/// let iter  = SpecIteratorBuilder::new_with_start(
///     "YY-1M-L", WeekendSkipper::new(), start,
/// )
/// .build()
/// .unwrap();
///
/// let dates: Vec<_> = iter.take(3).collect().unwrap();
/// // → 2024-01-31, 2024-02-29, 2024-03-31
/// for r in &dates {
///     // r.observed() is the settlement date
///     // r.actual()   is the raw calendar date (differs when biz-day adj fires)
///     println!("{}", r.observed());
/// }
/// ```
///
/// # See Also
///
/// - [`SpecIteratorBuilder`]
/// - [`NaiveSpecIterator`]
#[derive(Debug, Clone)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    dst_policy: DstPolicy,
    naive_spec_iter: NaiveSpecIterator<BDP>,
    /// Actual instant of the last yielded occurrence. Distinct local times
    /// can map to one instant across a DST gap; later duplicates are dropped
    /// so instants strictly increase.
    last: Option<DateTime<Tz>>,
    /// `with_count`: results left.
    remaining: Option<usize>,
}

impl<Tz: TimeZone, BDM: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDM> {
    type Item = Occurrence<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        if self.remaining == Some(0) {
            return Ok(None);
        }
        loop {
            let Some(next) = self.naive_spec_iter.next()? else {
                return Ok(None);
            };
            let next = next_result_to_tz(&self.tz, next, self.dst_policy)?;
            if self.last.as_ref().is_some_and(|last| next.actual() <= last) {
                continue;
            }
            self.last = Some(next.actual().clone());
            if let Some(n) = &mut self.remaining {
                *n -= 1;
            }
            return Ok(Some(next));
        }
    }
}

/// Non-timezone-aware calendar-day recurrence iterator.
///
/// The timezone-aware [`SpecIterator`] delegates all date arithmetic to this
/// type, and so does the datetime iterator. Use it directly when you do not
/// need timezone conversion.
///
/// # Construction
///
/// `NaiveSpecIterator` is not constructed through the builder; use its
/// inherent constructors, which mirror the builder's:
///
/// | Method | First result |
/// |--------|-------------|
/// | `new_after(spec, bdp, dtm)` | First occurrence **after** `dtm` |
/// | `new_with_start(spec, bdp, start)` | `start` itself is the first item |
/// | `new_with_end(spec, bdp, start, end)` | As above, up to and including `end` |
/// | `new_with_end_spec(spec, bdp, start, end_spec)` | As above; `end` is the first occurrence of `end_spec` after `start` |
///
/// # Example
///
/// ```rust
/// use tkone_schedule::biz_day::WeekendSkipper;
/// use tkone_schedule::date::NaiveSpecIterator;
/// use chrono::NaiveDate;
/// use fallible_iterator::FallibleIterator;
///
/// let start = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap().and_hms_opt(0, 0, 0).unwrap();
/// let iter = NaiveSpecIterator::new_with_start("YY-1M-L", WeekendSkipper::new(), start).unwrap();
/// let dates: Vec<_> = iter.take(3).map(|o| Ok(o.observed().date())).collect().unwrap();
/// assert_eq!(dates[1], NaiveDate::from_ymd_opt(2025, 2, 28).unwrap());
/// ```
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    spec: Spec,
    dtm: NaiveDateTime,
    context: engine::IterContext<BDP>,
    index: usize,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
    /// Exclusion specs: occurrences whose actual date matches one are
    /// skipped.
    exclusions: Vec<NaiveSpecIterator<BDP>>,
    /// Apply the `~` adjustment to `start` (a step rule's first date from an
    /// anchoring start) instead of yielding it as-is.
    adjust_start: bool,
}

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    fn new(
        spec: &str,
        bdp: BDP,
        dtm: NaiveDateTime,
        anchor: NaiveDateTime,
        start: Option<NaiveDateTime>,
        end: Option<NaiveDateTime>,
    ) -> Result<Self> {
        Ok(Self {
            spec: spec.parse()?,
            dtm,
            context: engine::IterContext {
                anchor,
                bd_processor: bdp,
            },
            index: 0,
            start,
            end,
            exclusions: Vec::new(),
            adjust_start: false,
        })
    }

    /// Skip the dates of `specs`, with cadences counted from `anchor`.
    fn excluding(mut self, specs: &[String], anchor: NaiveDateTime) -> Result<Self> {
        for spec in specs {
            let bdp = self.context.bd_processor.clone();
            self.exclusions.push(Self::exclusion(spec, bdp, anchor)?);
        }
        Ok(self)
    }

    /// An exclusion spec, with cadences counted from `anchor`. Day steps are
    /// rejected: whether a date matches would depend on where the steps
    /// started.
    pub(crate) fn exclusion(spec: &str, bdp: BDP, anchor: NaiveDateTime) -> Result<Self> {
        let iter = Self::new(spec, bdp, anchor, anchor, None, None)?;
        if matches!(iter.spec.days, DayCycle::NextNth(..)) {
            return Err(Error::InvalidDateSpec(format!(
                "{spec}: an exclusion spec cannot use a day step (`nD`, `nBD`, `nWD`)"
            )));
        }
        Ok(iter)
    }

    /// Whether `date` is a raw (unadjusted) date of this spec.
    pub(crate) fn matches_date(&self, date: NaiveDate) -> Result<bool> {
        let before = date.and_time(NaiveTime::MIN) - Duration::nanoseconds(1);
        Ok(engine::next_after(&self.spec, before, &self.context)?
            .is_some_and(|occurrence| occurrence.actual().date() == date))
    }

    /// Occurrences strictly after `last`, positioned as if iterating from
    /// `start` (which is yielded first if it is after `last`) up to `end`.
    ///
    /// With `passthrough` (a builder start) `start` itself is yielded first,
    /// as-is; without it (an anchoring start, as a schedule's) results are
    /// the occurrences at or after `start`.
    pub(crate) fn resumed(
        spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        last: NaiveDateTime,
        end: Option<NaiveDateTime>,
        passthrough: bool,
    ) -> Result<Self> {
        let mut iter = Self::new(spec, bdp, start, start, Some(start), end)?;
        if !passthrough {
            if !matches!(iter.spec.days, DayCycle::NextNth(..)) {
                // Calendar rules: the first date after the day before the
                // start (or after `last`), at the start's time of day.
                let from = last.max(start - Duration::nanoseconds(1));
                let day = if from.time() < start.time() { from.date().pred_opt().unwrap_or(from.date()) } else { from.date() };
                iter.index = 1;
                iter.dtm = day.and_time(start.time());
                return Ok(iter);
            }
            // Step rules begin on the start (or the first valid day after it).
            match engine::first_step(&iter.spec, start, &iter.context)? {
                Some(first) => {
                    iter.start = Some(first);
                    iter.dtm = first;
                    iter.adjust_start = true;
                }
                None => {
                    iter.start = None;
                    iter.index = 1;
                }
            }
            if last < start {
                return Ok(iter);
            }
        }
        if last < start {
            // The start itself is still ahead (a datetime resumed on its
            // start's date): yield it first, as a continuous iterator would.
            return Ok(iter);
        }
        if !matches!(iter.spec.days, DayCycle::NextNth(..)) {
            // Calendar rules depend only on the position and the anchor.
            iter.index = 1;
            iter.dtm = last;
            return Ok(iter);
        }
        // Step rules advance from the previous occurrence: replay from the
        // start, then step back so the first result after `last` is next.
        loop {
            // Step back fully, including whether the start has been yielded.
            let (before, index) = (iter.dtm, iter.index);
            match iter.next()? {
                Some(occurrence) if *occurrence.actual() <= last => continue,
                Some(_) => {
                    iter.dtm = before;
                    iter.index = index;
                    return Ok(iter);
                }
                None => return Ok(iter),
            }
        }
    }

    /// Iterate occurrences strictly after `dtm`; `nY` / `nM` cadences align to `dtm`.
    pub fn new_after(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        Self::new(spec, bdp, dtm, dtm, None, None)
    }

    /// Like [`new_after`](Self::new_after), but aligns `nY` / `nM` cadences to
    /// `anchor` instead of `dtm`.
    pub(crate) fn new_after_anchored(
        spec: &str,
        bdp: BDP,
        dtm: NaiveDateTime,
        anchor: NaiveDateTime,
    ) -> Result<Self> {
        Self::new(spec, bdp, dtm, anchor, None, None)
    }

    /// Yield `start` first (unchanged, as `Exact`), then iterate forward.
    pub fn new_with_start(spec: &str, bdp: BDP, start: NaiveDateTime) -> Result<Self> {
        Self::new(spec, bdp, start, start, Some(start), None)
    }

    /// Like [`new_with_start`](Self::new_with_start), up to and including `end`.
    pub fn new_with_end(
        spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> Result<Self> {
        Self::new(spec, bdp, start, start, Some(start), Some(end))
    }

    /// Like [`new_with_end`](Self::new_with_end), with `end` set to the first
    /// occurrence of `end_spec` strictly after `start` (its observed date).
    pub fn new_with_end_spec(
        spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        end_spec: &str,
    ) -> Result<Self> {
        // A start-inclusive iterator would yield `start` itself and leave an
        // empty range.
        let end = Self::new_after(end_spec, bdp.clone(), start)?
            .next()?
            .ok_or(Error::InvalidEndSpec)?;
        Self::new_with_end(spec, bdp, start, *end.observed())
    }
}

impl<BDP: BizDayProcessor + Clone> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = Occurrence<NaiveDateTime>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        loop {
            let Some(next) = self.next_unexcluded()? else {
                return Ok(None);
            };
            if self.excluded(next.actual().date())? {
                continue;
            }
            return Ok(Some(next));
        }
    }
}

impl<BDP: BizDayProcessor + Clone> NaiveSpecIterator<BDP> {
    fn excluded(&self, date: NaiveDate) -> Result<bool> {
        for exclusion in &self.exclusions {
            if exclusion.matches_date(date)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// The next result before exclusions.
    fn next_unexcluded(&mut self) -> Result<Option<Occurrence<NaiveDateTime>>> {
        if let Some(end) = &self.end {
            if &self.dtm >= end {
                return Ok(None);
            }
        }

        if self.index == 0 {
            if let Some(start) = &self.start {
                if &self.dtm <= start {
                    self.dtm = *start;
                    self.index += 1;
                    if !self.adjust_start {
                        return Ok(Some(Occurrence::Exact(*start)));
                    }
                    let start = Occurrence::Exact(*start);
                    if !engine::skipped(&self.spec, &start, &self.context)? {
                        return Ok(Some(engine::adjust(&self.spec, start, &self.context)?));
                    }
                }
            }
        }

        // `~SB` can drop dates; a calendar with no business days at all
        // fails instead of looping.
        let mut raw = None;
        for _ in 0..10_000 {
            let Some(next) = engine::next_after(&self.spec, self.dtm, &self.context)? else {
                return Ok(None);
            };
            if !engine::skipped(&self.spec, &next, &self.context)? {
                raw = Some(next);
                break;
            }
            if self.end.is_some_and(|end| *next.actual() > end) {
                return Ok(None);
            }
            self.dtm = *next.actual();
        }
        let raw = raw.ok_or(Error::IteratorNotConverged)?;
        let next_result = engine::adjust(&self.spec, raw, &self.context)?;

        if let Some(end) = &self.end {
            // The end is inclusive. Stop when either the actual or the
            // observed (adjusted) date passes it, so a biz-day adjustment that
            // pushes into the next month does not produce a settlement date
            // beyond the caller's stated upper bound.
            if next_result.actual() > end || next_result.observed() > end {
                self.dtm = *end;
                return Ok(None);
            }
        };

        self.index += 1;
        self.dtm = *next_result.actual();
        Ok(Some(next_result))
    }
}

#[cfg(test)]
mod tests {
    use crate::biz_day::WeekendSkipper;

    use super::*;
    use chrono_tz::America::New_York;

    #[test]
    fn test_with_start() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 11, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter = SpecIteratorBuilder::new_with_start("YY-1M-DD", WeekendSkipper::new(), dtm)
            .build()
            .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<Occurrence<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 31, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_with_start("YY-1M-31N", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<Occurrence<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter_multiples() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2023, 1, 31, 23, 0, 0).unwrap();

        dbg!(&dtm);
        let spec_iter =
            SpecIteratorBuilder::new_after("YY-[02]-[01,02,03]", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        dbg!(spec_iter
            .take(15)
            .collect::<Vec<Occurrence<DateTime<_>>>>()
            .unwrap());
    }

    #[test]
    fn test_spec_iter_multiples_first_of_month() {
        // US Eastern Time (EST/EDT)
        let est = New_York;
        // Before DST starts (Standard Time)
        let dtm = est.with_ymd_and_hms(2024, 12, 1, 23, 0, 0).unwrap();

        let spec_iter =
            SpecIteratorBuilder::new_after("[2025,2026]-MM-01", WeekendSkipper::new(), dtm)
                .build()
                .unwrap();
        let results = spec_iter
            .take(3)
            .collect::<Vec<Occurrence<DateTime<_>>>>()
            .unwrap();

        dbg!(&results);

        // let expected = vec![
        //     Occurrence::Exact(est.with_ymd_and_hms(2025, 1, 1, 23, 0, 0).unwrap()),
        //     Occurrence::Exact(est.with_ymd_and_hms(2025, 2, 1, 23, 0, 0).unwrap()),
        //     Occurrence::Exact(est.with_ymd_and_hms(2025, 3, 1, 23, 0, 0).unwrap()),
        // ];
        // assert_eq!(results, expected);
    }
}
