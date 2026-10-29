use super::{engine, spec::Spec};
use crate::utils::next_result_to_tz;
use crate::{biz_day::BizDayProcessor, prelude::*, DstPolicy, Occurrence};
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
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
/// - `with_end` / `with_end_spec` are only available after `new_with_start`.
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
/// // → 2024-01-31, 2024-02-29, 2024-03-31, 2024-04-30, 2024-05-31
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
    /// receive [`Error::AmbiguousLocalTime`] whenever a scheduled datetime
    /// falls in a DST gap or overlap instead of being silently adjusted.
    pub fn with_dst_policy(mut self, policy: DstPolicy) -> Self {
        self.dst_policy = policy;
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
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator {
            tz: self.dtm.timezone(),
            last: None,
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_after(
                &self.spec,
                self.bd_processor,
                self.dtm.naive_local(),
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndDateTime<Tz>, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        Ok(SpecIterator {
            tz: start.timezone(),
            last: None,
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                &self.spec,
                self.bd_processor,
                start.naive_local(),
                self.end.0.with_timezone(&start.timezone()).naive_local(),
            )?,
        })
    }
}

impl<Tz: TimeZone, BDP: BizDayProcessor>
    SpecIteratorBuilder<Tz, BDP, StartDateTime<Tz>, EndSpec, Sealed>
{
    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        let start = self.start.0;
        Ok(SpecIterator {
            tz: start.timezone(),
            last: None,
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_end_spec(
                &self.spec,
                start.naive_local(),
                self.bd_processor,
                &self.end.0,
            )?,
        })
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
            marker_sealed: PhantomData,
        }
    }

    /// Bound the iterator by another spec string.
    ///
    /// The end datetime is resolved by running `end_spec` as its own single-shot
    /// iterator starting from `start`. The first occurrence of `end_spec` becomes
    /// the exclusive upper bound.
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
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

    /// Bound the iterator by an explicit end datetime.
    ///
    /// Occurrences whose *actual* (unadjusted) date or *observed* (adjusted)
    /// date exceeds `end` are suppressed. When the boundary is hit the iterator
    /// emits `end` itself as a terminal `Single` result, then stops.
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
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::<Tz, BDP> {
            tz: self.start.0.timezone(),
            last: None,
            dst_policy: self.dst_policy,
            naive_spec_iter: NaiveSpecIterator::new_with_start(
                &self.spec,
                self.bd_processor,
                self.start.0.naive_local(),
            )?,
        })
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
#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    dst_policy: DstPolicy,
    naive_spec_iter: NaiveSpecIterator<BDP>,
    /// Actual instant of the last yielded occurrence. Distinct local times
    /// can map to one instant across a DST gap; later duplicates are dropped
    /// so instants strictly increase.
    last: Option<DateTime<Tz>>,
}

impl<Tz: TimeZone, BDM: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDM> {
    type Item = Occurrence<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        loop {
            let Some(next) = self.naive_spec_iter.next()? else {
                return Ok(None);
            };
            let next = next_result_to_tz(&self.tz, next, self.dst_policy)?;
            if self.last.as_ref().is_some_and(|last| next.actual() <= last) {
                continue;
            }
            self.last = Some(next.actual().clone());
            return Ok(Some(next));
        }
    }
}

impl<Tz: TimeZone, BDM: BizDayProcessor> SpecIterator<Tz, BDM> {
    #[allow(dead_code)]
    pub(crate) fn update_cursor(&mut self, dtm: DateTime<Tz>) {
        self.naive_spec_iter.update_cursor(dtm.naive_local());
    }
}

/// Non-timezone-aware calendar-day recurrence iterator.
///
/// The timezone-aware [`SpecIterator`] delegates all date arithmetic to this
/// type. You can use `NaiveSpecIterator` directly when you do not need
/// timezone conversion, or when you are composing it with other naive
/// iterators (as [`crate::datetime::NaiveSpecIterator`] does internally).
///
/// # Construction
///
/// `NaiveSpecIterator` is not constructed through the builder; use its
/// inherent constructors:
///
/// | Method | First result |
/// |--------|-------------|
/// | `new_after(spec, bdp, dtm)` | First occurrence **after** `dtm` |
/// | `new_with_start(spec, bdp, start)` | `start` itself is the first item |
/// | `new_with_end(spec, bdp, start, end)` | As above, bounded by `end` |
/// | `new_with_end_spec(spec, start, bdp, end_spec)` | As above; end resolved from `end_spec` |
#[derive(Debug, Clone)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    spec: Spec,
    dtm: NaiveDateTime,
    context: engine::IterContext<BDP>,
    index: usize,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
}

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    pub(crate) fn new_after(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        Self::new_after_anchored(spec, bdp, dtm, dtm)
    }

    /// Like [`new_after`](Self::new_after), but aligns `nY` / `nM` cadences to
    /// `anchor` instead of `dtm`.
    pub(crate) fn new_after_anchored(
        spec: &str,
        bdp: BDP,
        dtm: NaiveDateTime,
        anchor: NaiveDateTime,
    ) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            context: engine::IterContext {
                anchor,
                bd_processor: bdp,
            },
            index: 0,
            start: None,
            end: None,
        })
    }

    fn new_with_start(spec: &str, bdp: BDP, start: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            context: engine::IterContext {
                anchor: start,
                bd_processor: bdp,
            },
            index: 0,
            start: Some(start),
            end: None,
        })
    }

    fn new_with_end(
        spec: &str,
        bdp: BDP,
        start: NaiveDateTime,
        end: NaiveDateTime,
    ) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            context: engine::IterContext {
                anchor: start,
                bd_processor: bdp,
            },
            index: 0,
            start: Some(start),
            end: Some(end),
        })
    }

    fn new_with_end_spec(
        spec: &str,
        start: NaiveDateTime,
        bdp: BDP,
        end_spec: &str,
    ) -> Result<Self> {
        let spec = spec.parse()?;
        // First occurrence strictly after `start`; a start-inclusive iterator
        // would yield `start` itself and leave an empty range.
        let end = Self::new_after(end_spec, bdp.clone(), start)?
            .next()?
            .ok_or(Error::InvalidEndSpec)?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            context: engine::IterContext {
                anchor: start,
                bd_processor: bdp,
            },
            index: 0,
            start: Some(start),
            end: Some(end.observed().clone()),
        })
    }

    pub(crate) fn update_cursor(&mut self, dtm: NaiveDateTime) {
        self.dtm = dtm;
        self.start = None;
        self.index = 0;
    }
}

impl<BDP: BizDayProcessor + Clone> FallibleIterator for NaiveSpecIterator<BDP> {
    type Item = Occurrence<NaiveDateTime>;
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
                    return Ok(Some(Occurrence::Exact(start.clone())));
                }
            }
        }

        let Some(raw) = engine::next_after(&self.spec, self.dtm, &self.context)? else {
            return Ok(None);
        };
        let next_result = engine::adjust(&self.spec, raw, &self.context)?;

        if let Some(end) = &self.end {
            // Filter when the actual date exceeds the end boundary.
            // Also filter when the observed (adjusted) date exceeds the end,
            // so that e.g. a biz-day adjustment that pushes into the next
            // month does not produce a result whose settlement date is beyond
            // the caller's stated upper bound.
            if next_result.actual() > &end || next_result.observed() > &end {
                self.dtm = end.clone();
                self.index += 1;
                return Ok(Some(Occurrence::Exact(end.clone())));
            }
        };

        self.index += 1;
        self.dtm = next_result.actual().clone();
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
