use super::{
    component::{self, DateComponent},
    spec::{BizDayAdjustment, Cycle, DayCycle, LastDayOption, NextNthDayOption, Spec},
};
use crate::biz_day::WeekendSkipper;
use crate::{biz_day::BizDayProcessor, prelude::*, NextResult};
use chrono::{DateTime, Datelike, NaiveDateTime, TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use std::{marker::PhantomData, sync::LazyLock};

pub struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct NoStart;
pub struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
pub struct EndSpec(String);
pub struct NoEnd;
pub struct Sealed;
pub struct NotSealed;

pub struct SpecIteratorBuilder<Tz: TimeZone, BDP: BizDayProcessor, START, END, S> {
    dtm: DateTime<Tz>,
    start: START,
    spec: String,
    bd_processor: BDP,
    end: END,
    timezone: Tz,
    marker_sealed: PhantomData<S>,
}

impl<Tz: TimeZone, BDP: BizDayProcessor> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
    pub fn new(
        spec: &str,
        bdp: BDP,
        tz: Tz,
    ) -> SpecIteratorBuilder<Tz, BDP, NoStart, NoEnd, NotSealed> {
        SpecIteratorBuilder::new_after(spec, bdp, Utc::now().with_timezone(&tz))
    }

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
            marker_sealed: PhantomData,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator {
            tz: self.dtm.timezone(),
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
            naive_spec_iter: NaiveSpecIterator::new_with_end(
                &self.spec,
                self.bd_processor,
                start.naive_local(),
                self.end.0.naive_local(),
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
            marker_sealed: PhantomData,
        }
    }

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
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

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
            marker_sealed: PhantomData,
            timezone: self.timezone,
        }
    }

    pub fn build(self) -> Result<SpecIterator<Tz, BDP>> {
        Ok(SpecIterator::<Tz, BDP> {
            tz: self.start.0.timezone(),
            naive_spec_iter: NaiveSpecIterator::new_with_start(
                &self.spec,
                self.bd_processor,
                self.start.0.naive_local(),
            )?,
        })
    }
}

static WEEKEND_SKIPPER: LazyLock<WeekendSkipper> = LazyLock::new(|| WeekendSkipper::new());

/// # SpecIterator
/// datetime::SpecIterator is an iterator that combines a date and time specification to generate a sequence of date-times.
/// This iterator is created using the SpecIteratorBuilder.
///
/// ## Example
/// ```rust
/// use lib_schedule::biz_day::WeekendSkipper;
/// use lib_schedule::date::SpecIteratorBuilder;
/// use chrono_tz::America::New_York;
/// use fallible_iterator::FallibleIterator;
/// use chrono::{offset::TimeZone, DateTime};
/// use lib_schedule::NextResult;
/// use chrono::Duration;
///
/// let start = New_York.with_ymd_and_hms(2024, 11, 30, 11, 0, 0).unwrap();
/// let iter = SpecIteratorBuilder::new_with_start("YY-1M-31L", WeekendSkipper::new(), start).build().unwrap();
/// let occurrences = iter.take(4).collect::<Vec<NextResult<DateTime<_>>>>().unwrap();
/// assert_eq!(occurrences, vec![
///     NextResult::Single(start.clone()), // 2024-11-30
///     NextResult::Single(start + Duration::days(31)), // 2024-12-31
///     NextResult::Single(start + Duration::days(62)), // 2025-01-31
///     NextResult::Single(start + Duration::days(90)), // 2025-02-28
/// ]);
/// ```
///
/// ## See Also
/// - [SpecIteratorBuilder](crate::date::SpecIteratorBuilder)
/// - [SPEC_EXPR](crate::date::SPEC_EXPR)
/// - [NaiveSpecIterator](crate::date::NaiveSpecIterator)
#[derive(Debug)]
pub struct SpecIterator<Tz: TimeZone, BDP: BizDayProcessor> {
    tz: Tz,
    naive_spec_iter: NaiveSpecIterator<BDP>,
}

impl<Tz: TimeZone, BDM: BizDayProcessor> FallibleIterator for SpecIterator<Tz, BDM> {
    type Item = NextResult<DateTime<Tz>>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        let next = self.naive_spec_iter.next()?;
        let Some(next) = next else {
            return Ok(None);
        };
        Ok(Some(Self::Item::from(W((self.tz.clone(), next)))))
    }
}

impl<Tz: TimeZone, BDM: BizDayProcessor> SpecIterator<Tz, BDM> {
    #[allow(dead_code)]
    pub(crate) fn update_cursor(&mut self, dtm: DateTime<Tz>) {
        self.naive_spec_iter.update_cursor(dtm.naive_local());
    }
}

#[derive(Debug, Clone)]
pub struct NaiveSpecIterator<BDP: BizDayProcessor> {
    spec: Spec,
    dtm: NaiveDateTime,
    context: component::IterContext<BDP>,
    index: usize,
    start: Option<NaiveDateTime>,
    end: Option<NaiveDateTime>,
}

impl<BDP: BizDayProcessor> NaiveSpecIterator<BDP> {
    pub(crate) fn new_after(spec: &str, bdp: BDP, dtm: NaiveDateTime) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            context: component::IterContext {
                start_dt: dtm,
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
            context: component::IterContext {
                start_dt: start,
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
            context: component::IterContext {
                start_dt: start,
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
        let end = Self::new_with_start(end_spec, bdp.clone(), start.clone())?
            .next()?
            .ok_or(Error::Custom("invalid end spec"))?;
        Ok(Self {
            spec,
            dtm: start.clone(),
            context: component::IterContext {
                start_dt: start,
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
    type Item = NextResult<NaiveDateTime>;
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
                    return Ok(Some(NextResult::Single(start.clone())));
                }
            }
        }

        let mut candidate = self.dtm;
        let mut iterations = 0u32;
        loop {
            iterations += 1;
            if iterations > 10_000 {
                return Err(Error::Custom("schedule iterator did not converge"));
            }

            // ── NextNth year + Values months: intra-year month enumeration ──
            //
            // When the year spec is NextNth and the month spec is a fixed Values
            // set, each valid year must visit *all* months in the set before the
            // year advances.  The NextNth year component normally preserves the
            // current month when jumping to the next valid year (to support the
            // combined `1Y-3M-15` clock), which would cause months earlier in the
            // set to be skipped.
            //
            // Two adjustments address this:
            //
            // 1. Suppress year advance while more Values months remain in the
            //    current year by passing first-of-next-month to the year
            //    component.  Its `remainder==0 && day==1` guard then fires and
            //    keeps us in the same year.
            //
            // 2. After the year does advance, reset year_candidate to the first
            //    valid month in the new year (instead of whatever month the year
            //    component preserved), so the month component starts scanning
            //    from the beginning of the year.
            let (candidate_for_year, reset_month_on_advance) =
                if let (Cycle::NextNth(n), Cycle::Values(month_vals)) =
                    (&self.spec.years, &self.spec.months)
                {
                    let diff = candidate.year() - self.context.start_dt.year();
                    let year_is_valid = diff >= 0 && diff % (*n as i32) == 0;
                    if year_is_valid && !month_vals.is_empty() {
                        if matches!(&self.spec.days, DayCycle::NextNth(..)) {
                            // For relative day specs: always stabilize the year by
                            // passing first-of-current-month (day==1 guard prevents
                            // year from advancing). Month and year transitions happen
                            // naturally through day overflow + month revalidation, so
                            // reset_month_on_advance is not needed here.
                            let current_mo_first = chrono::NaiveDate::from_ymd_opt(
                                candidate.year(),
                                candidate.month(),
                                1,
                            )
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                            (current_mo_first, false)
                        } else {
                            let has_more_months =
                                month_vals.iter().any(|&m| m > candidate.month());
                            if has_more_months {
                                // For fixed-day specs with remaining months: suppress
                                // year advance by passing first-of-next-month so the
                                // NextNth `day==1` guard fires and stays.
                                let next_mo = chrono::NaiveDate::from_ymd_opt(
                                    candidate.year(),
                                    candidate.month() + 1,
                                    1,
                                )
                                .unwrap_or_else(|| {
                                    chrono::NaiveDate::from_ymd_opt(candidate.year() + 1, 1, 1)
                                        .unwrap()
                                })
                                .and_hms_opt(0, 0, 0)
                                .unwrap();
                                (next_mo, false)
                            } else {
                                // Fixed-day specs with no remaining months: let year
                                // advance and reset to first valid month.
                                (candidate, true)
                            }
                        }
                    } else {
                        (candidate, false)
                    }
                } else {
                    (candidate, false)
                };

            let year_candidate = self
                .spec
                .years
                .next_date(&candidate_for_year, &self.context);
            let Some(year_candidate) = year_candidate else {
                return Ok(None);
            };

            // After year advancement with Values months: reset to the first
            // valid month in the new year.
            let year_candidate = if reset_month_on_advance
                && year_candidate.year() != candidate.year()
            {
                if let Cycle::Values(month_vals) = &self.spec.months {
                    month_vals
                        .iter()
                        .next()
                        .and_then(|&m| chrono::NaiveDate::from_ymd_opt(year_candidate.year(), m, 1))
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                        .unwrap_or(year_candidate)
                } else {
                    year_candidate
                }
            } else {
                year_candidate
            };

            // Whether the year component advanced us to a new calendar year.
            let year_advanced = year_candidate.year() != candidate.year();

            // Relative-advance day specs (NextNth, ForEach) must always push the
            // month forward; fixed-day specs (OnDays, OnWeekDays) may stay in the
            // current aligned month so we don't skip dates within that month.
            // For Values months after year advancement: year_candidate is already
            // at the first valid month — don't force-advance past it.
            let day_is_relative =
                matches!(&self.spec.days, DayCycle::NextNth(..) | DayCycle::ForEach);
            let month_must_advance = (year_advanced
                && !matches!(&self.spec.months, Cycle::Values(_)))
                || day_is_relative;

            let month_candidate = component::find_next_in_month_cycle(
                &self.spec.months,
                &year_candidate,
                &self.context,
                month_must_advance,
            );
            let Some(month_candidate) = month_candidate else {
                return Ok(None);
            };

            // When the month cycle has jumped to a new month, OnDays needs to
            // search from the last day of the preceding month so that
            // `day > prev_last_day` correctly rolls into the first valid
            // day-of-month in the new period.
            // For NextNth day specs we preserve the day-of-month position from
            // `candidate` so the full "month + days" offset accumulates (e.g.
            // "1M-7D" advances by 1 month AND 7 days each tick).
            let month_advanced = month_candidate.month() != year_candidate.month()
                || month_candidate.year() != year_candidate.year();
            // A "period reset" occurs when either the month advanced within the year,
            // OR the year advanced into a new cycle (resetting to the first valid month).
            // Both cases require the day sequence to restart from the beginning of the
            // new month period.
            let period_reset =
                month_advanced || (year_advanced && matches!(&self.spec.months, Cycle::Values(_)));
            let day_cursor = if period_reset {
                match &self.spec.days {
                    DayCycle::OnDays { .. } if period_reset => {
                        (month_candidate.date().pred_opt().unwrap())
                            .and_hms_opt(0, 0, 0)
                            .unwrap()
                    }
                    DayCycle::NextNth(n, NextNthDayOption::Regular)
                        if matches!(&self.spec.months, Cycle::Values(_)) =>
                    {
                        // For Values months + regular NextNth days: set cursor to
                        // (month_start - n) so that next_date lands exactly on
                        // month_start, restarting the sequence from day 1.
                        month_candidate - chrono::Duration::days(*n as i64)
                    }
                    DayCycle::NextNth(..) if !matches!(&self.spec.months, Cycle::Values(_)) => {
                        // Preserve same day-of-month only for relative month cycles
                        // (NextNth, ForEach, AsIs) so combined specs like "1M-7D"
                        // accumulate month + days together.
                        chrono::NaiveDate::from_ymd_opt(
                            month_candidate.year(),
                            month_candidate.month(),
                            candidate.day(),
                        )
                        .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                        .unwrap_or(month_candidate)
                    }
                    DayCycle::NextNth(..) => {
                        // For BizDay/WeekDay variants with Values months: step begins
                        // from month_start.
                        month_candidate
                    }
                    _ => month_candidate,
                }
            } else {
                // No period reset: for relative day specs with Values months, use the
                // actual candidate position so next_date advances from the right place.
                // For OnDays specs, if every day in the set is ≤ month_candidate's
                // day-of-month, the day component would find nothing in the current
                // month and roll over (e.g. day={1} when month_candidate is the 1st
                // of a 6-month aligned period).  Step back one day so the component
                // can find the first valid day in the current period.
                match &self.spec.days {
                    DayCycle::NextNth(..) if matches!(&self.spec.months, Cycle::Values(_)) => {
                        candidate
                    }
                    DayCycle::OnDays { days, .. }
                        if !days.is_empty()
                            && days.iter().all(|&d| d <= month_candidate.day()) =>
                    {
                        month_candidate
                            .date()
                            .pred_opt()
                            .map(|d| d.and_time(month_candidate.time()))
                            .unwrap_or(month_candidate)
                    }
                    _ => month_candidate,
                }
            };

            let day_candidate = self.spec.days.next_date(&day_cursor, &self.context);
            let Some(day_candidate) = day_candidate else {
                // This month has no valid days; advance to the next month and retry.
                let (y, m) = component::ffwd_months(&month_candidate, 1);
                candidate = chrono::NaiveDate::from_ymd_opt(y as i32, m, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap();
                continue;
            };

            if day_candidate > self.dtm {
                // --- Year re-validation ---
                // Only necessary when the year spec restricts to a finite set of
                // years (Values) or a sparse cadence (NextNth with n>1 effectively
                // restricts years).  For ForEach/AsIs every year is valid.
                // For NextNth, check alignment explicitly instead of calling
                // next_date (which would always return the *next* aligned year,
                // triggering a false mismatch for the current valid year).
                let year_is_valid = match &self.spec.years {
                    Cycle::AsIs | Cycle::ForEach => true,
                    Cycle::Values(values) => values.contains(&(day_candidate.year() as u32)),
                    Cycle::NextNth(n) => {
                        let diff = day_candidate.year() - self.context.start_dt.year();
                        diff >= 0 && diff % (*n as i32) == 0
                    }
                };
                if !year_is_valid {
                    let year_recheck = self.spec.years.next_date(&day_candidate, &self.context);
                    match year_recheck {
                        None => return Ok(None),
                        Some(ref y) => {
                            candidate =
                                (y.date().pred_opt().unwrap()).and_hms_opt(0, 0, 0).unwrap();
                            continue;
                        }
                    }
                }

                // --- Month re-validation ---
                // For Values months: the day component may have overflowed into
                // a month outside the allowed set.
                if let Cycle::Values(_) = &self.spec.months {
                    let month_recheck = component::find_next_in_month_cycle(
                        &self.spec.months,
                        &day_candidate,
                        &self.context,
                        true,
                    );
                    match month_recheck {
                        None => return Ok(None),
                        Some(ref m)
                            if m.month() != day_candidate.month()
                                || m.year() != day_candidate.year() =>
                        {
                            candidate =
                                (m.date().pred_opt().unwrap()).and_hms_opt(0, 0, 0).unwrap();
                            continue;
                        }
                        _ => {}
                    }
                }

                // For NextNth months: the day component may have rolled into a
                // month that is not in the N-month cadence.  Re-align if needed.
                if let Cycle::NextNth(n) = &self.spec.months {
                    let start_month = self.context.start_dt.month() as i32;
                    let start_year = self.context.start_dt.year();
                    let total = (day_candidate.year() - start_year) * 12
                        + (day_candidate.month() as i32 - start_month);
                    if total.rem_euclid(*n as i32) != 0 {
                        let rem = total.rem_euclid(*n as i32);
                        let months_to_add = *n - rem as u32;
                        let (ny, nm) = component::ffwd_months(&day_candidate, months_to_add);
                        candidate = chrono::NaiveDate::from_ymd_opt(ny as i32, nm, 1)
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                        continue;
                    }
                }

                candidate = day_candidate;
                break;
            } else {
                // No progress: if the pipeline returned the same value it
                // started with, force a one-day advance to avoid an infinite
                // loop (can happen when all three components are AsIs).
                candidate = if day_candidate == candidate {
                    candidate + chrono::Duration::days(1)
                } else {
                    day_candidate
                };
            }
        }

        // --- Apply NextMonthFirstDay / NextMonthOverflow wrapping ---
        // These options mean: if the target day doesn't exist in this month,
        // return the last day as `actual` and wrap into the next month as
        // `observed`.  The component already clamped to last-day-of-month;
        // here we detect that clamping and build the AdjustedLater result.
        let next_result = if let DayCycle::OnDays { days, option } = &self.spec.days {
            match option {
                LastDayOption::NextMonthFirstDay | LastDayOption::NextMonthOverflow => {
                    let last_day =
                        component::last_day_of_month(candidate.year(), candidate.month());
                    // Clamped when the returned day equals the month's last day AND
                    // some day in the set is strictly larger (meaning it overflowed).
                    let target_day = days.iter().copied().find(|&d| d > last_day.day());
                    if candidate.day() == last_day.day() && target_day.is_some() {
                        let (ny, nm) = component::ffwd_months(&candidate, 1);
                        let first_of_next = chrono::NaiveDate::from_ymd_opt(ny as i32, nm, 1)
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap();
                        let observed = match option {
                            LastDayOption::NextMonthFirstDay => first_of_next,
                            LastDayOption::NextMonthOverflow => {
                                let overflow = target_day.unwrap() - last_day.day(); // days past month end
                                first_of_next + chrono::Duration::days(overflow as i64 - 1)
                            }
                            _ => unreachable!(),
                        };
                        NextResult::AdjustedLater(candidate, observed)
                    } else {
                        NextResult::Single(candidate)
                    }
                }
                _ => NextResult::Single(candidate),
            }
        } else {
            NextResult::Single(candidate)
        };

        // --- Apply BizDay adjustment ---
        // Prev(n) / Next(n) are unconditional offsets; the directional variants
        // (BizDay / Weekday) only apply when the actual date is not a biz day.
        let next_result = if let Some(biz_day_adj) = &self.spec.biz_day_adj {
            let (actual, observed) = next_result.as_tuple();
            match biz_day_adj {
                BizDayAdjustment::Prev(num) => NextResult::AdjustedEarlier(
                    actual.clone(),
                    self.context.bd_processor.sub(observed, *num)?,
                ),
                BizDayAdjustment::Next(num) => NextResult::AdjustedLater(
                    actual.clone(),
                    self.context.bd_processor.add(observed, *num)?,
                ),
                _ => {
                    if self.context.bd_processor.is_biz_day(&observed)? {
                        next_result
                    } else {
                        match biz_day_adj {
                            BizDayAdjustment::Weekday(dir) => {
                                let adjusted =
                                    WEEKEND_SKIPPER.find_biz_day(observed, dir.clone())?;
                                adjusted_to_next_result(*actual, adjusted)
                            }
                            BizDayAdjustment::BizDay(dir) => {
                                let adjusted = self
                                    .context
                                    .bd_processor
                                    .find_biz_day(observed, dir.clone())?;
                                adjusted_to_next_result(*actual, adjusted)
                            }
                            BizDayAdjustment::NA => next_result,
                            _ => unreachable!(),
                        }
                    }
                }
            }
        } else {
            next_result
        };

        if next_result.actual() <= &self.dtm {
            return Ok(None);
        }

        if let Some(end) = &self.end {
            // Filter when the actual date exceeds the end boundary.
            // Also filter when the observed (adjusted) date exceeds the end,
            // so that e.g. a biz-day adjustment that pushes into the next
            // month does not produce a result whose settlement date is beyond
            // the caller's stated upper bound.
            if next_result.actual() > &end || next_result.observed() > &end {
                self.dtm = end.clone();
                self.index += 1;
                return Ok(Some(NextResult::Single(end.clone())));
            }
        };

        self.index += 1;
        self.dtm = next_result.actual().clone();
        Ok(Some(next_result))
    }
}

fn adjusted_to_next_result(
    dtm: NaiveDateTime,
    adjusted: NaiveDateTime,
) -> NextResult<NaiveDateTime> {
    if adjusted == dtm {
        NextResult::Single(adjusted)
    } else if adjusted > dtm {
        NextResult::AdjustedLater(dtm, adjusted)
    } else {
        NextResult::AdjustedEarlier(dtm, adjusted)
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
            .collect::<Vec<NextResult<DateTime<_>>>>()
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
            .collect::<Vec<NextResult<DateTime<_>>>>()
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
            .collect::<Vec<NextResult<DateTime<_>>>>()
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
            .collect::<Vec<NextResult<DateTime<_>>>>()
            .unwrap();

        dbg!(&results);

        // let expected = vec![
        //     NextResult::Single(est.with_ymd_and_hms(2025, 1, 1, 23, 0, 0).unwrap()),
        //     NextResult::Single(est.with_ymd_and_hms(2025, 2, 1, 23, 0, 0).unwrap()),
        //     NextResult::Single(est.with_ymd_and_hms(2025, 3, 1, 23, 0, 0).unwrap()),
        // ];
        // assert_eq!(results, expected);
    }
}
