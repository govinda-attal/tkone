use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::{
    biz_day::BizDayProcessor,
    date::spec::{Cycle, DayCycle, LastDayOption, NextNthDayOption, WeekdayOption},
    utils::{DateLikeUtils, WeekdayStartingMonday},
};

#[derive(Clone, Debug)]
pub(super) struct IterContext<BDP: BizDayProcessor> {
    pub start_dt: NaiveDateTime,
    pub bd_processor: BDP,
}

/// A trait for a component of a date specification (year, month, or day)
/// that can determine the next valid date based on its own rules.
pub(super) trait DateComponent: std::fmt::Debug {
    /// Given a date, find the next valid date according to this component's rules.
    ///
    /// # Arguments
    /// * `after` - The date to start searching from. The result must be strictly after this date.
    /// * `context` - Shared context for the iteration, like the business day processor.
    /// * `reset` - If `true`, the component should find the *first* valid date in the period
    ///   (e.g., first valid day of the month). If `false`, it should find the *next* valid date.
    fn next_date<BDP: BizDayProcessor + Clone>(
        &self,
        after: &NaiveDateTime,
        context: &IterContext<BDP>,
    ) -> Option<NaiveDateTime>;
}

impl DateComponent for Cycle {
    fn next_date<BDP: BizDayProcessor + Clone>(
        &self,
        after: &NaiveDateTime,
        context: &IterContext<BDP>,
    ) -> Option<NaiveDateTime> {
        match self {
            Cycle::AsIs => Some(after.clone()),
            Cycle::ForEach => Some(after.clone()), // ForEach is handled by the parent component driving the iteration
            Cycle::Values(values) => {
                let current_year = after.year() as u32;
                if let Some(next_year) = values.iter().find(|&&y| y >= current_year) {
                    if *next_year == current_year {
                        Some(after.clone())
                    } else {
                        Some(
                            NaiveDate::from_ymd_opt(*next_year as i32, 1, 1)
                                .unwrap()
                                .and_hms_opt(0, 0, 0)
                                .unwrap(),
                        )
                    }
                } else {
                    None
                }
            }
            Cycle::NextNth(n) => {
                let start_year = context.start_dt.year();
                let after_year = after.year();

                let diff = after_year - start_year;
                let remainder = diff.rem_euclid(*n as i32);

                // Stay in the current year only when we are already at the
                // first day of a valid-year month (i.e. the year component has
                // already been positioned by a prior iteration step).
                let next_year = if remainder == 0 && after.day() == 1 {
                    after_year
                } else {
                    after_year + (*n as i32 - remainder)
                };

                // Preserve the month so that a combined `1Y-3M-15` spec
                // advances both clocks together (year +1, month +3 per tick).
                Some(
                    NaiveDate::from_ymd_opt(next_year, after.month(), 1)
                        .unwrap()
                        .and_hms_opt(0, 0, 0)
                        .unwrap(),
                )
            }
        }
    }
}

impl DateComponent for DayCycle {
    fn next_date<BDP: BizDayProcessor + Clone>(
        &self,
        after: &NaiveDateTime,
        context: &IterContext<BDP>,
    ) -> Option<NaiveDateTime> {
        match self {
            DayCycle::AsIs => Some(after.clone()),
            DayCycle::ForEach => Some(*after + Duration::days(1)),
            DayCycle::NextNth(n, opt) => match opt {
                NextNthDayOption::Regular => Some(*after + Duration::days(*n as i64)),
                NextNthDayOption::BizDay => context.bd_processor.add(after, *n).ok(),
                NextNthDayOption::WeekDay => {
                    crate::biz_day::WeekendSkipper::new().add(after, *n).ok()
                }
            },
            DayCycle::OnDays { days, option } => {
                if days.is_empty() {
                    // This is the 'L' case
                    if option == &LastDayOption::LastDay {
                        let next_dt = if after.day()
                            == last_day_of_month(after.year(), after.month()).day()
                        {
                            let (y, m) = ffwd_months(after, 1);
                            NaiveDate::from_ymd_opt(y as i32, m, 1).unwrap()
                        } else {
                            after.date()
                        };
                        let last_day = last_day_of_month(next_dt.year(), next_dt.month());
                        return Some(last_day.and_time(after.time()));
                    } else {
                        return None;
                    }
                }

                let mut d = after.day();
                let mut m = after.month();
                let mut y = after.year();

                // Find next day in the set
                if let Some(&next_day) = days.iter().find(|&&day| day > d) {
                    d = next_day;
                } else {
                    // Roll over to the next month
                    d = *days.iter().next().unwrap();
                    m += 1;
                    if m > 12 {
                        m = 1;
                        y += 1;
                    }
                }

                let mut candidate_date = NaiveDate::from_ymd_opt(y, m, d);
                if candidate_date.is_none() {
                    // The target day does not exist in this month (e.g. day 31 in
                    // February).  Choose the fallback based on the overflow option:
                    //   LastDay / NextMonthFirstDay / NextMonthOverflow → clamp to
                    //     the last day of this month (the iterator will wrap with an
                    //     AdjustedLater result for N/O after the fact).
                    //   NA → skip to the first day of the set in the next month.
                    match option {
                        LastDayOption::LastDay
                        | LastDayOption::NextMonthFirstDay
                        | LastDayOption::NextMonthOverflow => {
                            candidate_date = Some(last_day_of_month(y, m));
                        }
                        LastDayOption::NA => {
                            m += 1;
                            if m > 12 {
                                m = 1;
                                y += 1;
                            }
                            d = *days.iter().next().unwrap();
                            candidate_date = NaiveDate::from_ymd_opt(y, m, d);
                        }
                    }
                }

                candidate_date.map(|date| date.and_time(after.time()))
            }
            DayCycle::OnWeekDays { weekdays, option } => {
                if weekdays.is_empty() {
                    return None;
                }

                match option {
                    WeekdayOption::NA => {
                        let mut next_dt = *after + Duration::days(1);
                        while !weekdays.contains(&WeekdayStartingMonday(next_dt.weekday())) {
                            next_dt += Duration::days(1);
                        }
                        Some(next_dt)
                    }
                    WeekdayOption::Starting(occurrence) => {
                        let occurrence = occurrence.unwrap_or(1);
                        let wd = weekdays.iter().next().unwrap().0; // This logic assumes single weekday for now
                        let mut target_dt = after.clone();
                        loop {
                            if let Some(candidate) = target_dt.to_months_weekday(&wd, occurrence) {
                                if candidate > *after {
                                    return Some(candidate);
                                }
                            }
                            // Move to next month to try again
                            let (y, m) = ffwd_months(&target_dt, 1);
                            target_dt = NaiveDate::from_ymd_opt(y as i32, m, 1)
                                .unwrap()
                                .and_hms_opt(0, 0, 0)
                                .unwrap();
                        }
                    }
                    WeekdayOption::Ending(occurrence) => {
                        let occurrence = occurrence.unwrap_or(1);
                        let wd = weekdays.iter().next().unwrap().0; // This logic assumes single weekday for now
                        let mut target_dt = after.clone();
                        loop {
                            if let Some(candidate) =
                                target_dt.to_months_last_weekday(&wd, occurrence)
                            {
                                if candidate > *after {
                                    return Some(candidate);
                                }
                            }
                            // Move to next month to try again
                            let (y, m) = ffwd_months(&target_dt, 1);
                            target_dt = NaiveDate::from_ymd_opt(y as i32, m, 1)
                                .unwrap()
                                .and_hms_opt(0, 0, 0)
                                .unwrap();
                        }
                    }
                }
            }
        }
    }
}

/// A utility function to find the next date in a sequence of months.
///
/// `must_advance` controls the behaviour for `NextNth` cycles when the
/// current month is already aligned to the cadence (remainder == 0):
///   * `true`  – always step forward by the full period (used when the year
///               just advanced or the day spec is a relative-advance type).
///   * `false` – stay in the current month and let the day component find the
///               next occurrence within this aligned period (used for fixed-day
///               specs whose first occurrence may still lie ahead this month).
pub(super) fn find_next_in_month_cycle(
    cycle: &Cycle,
    after: &NaiveDateTime,
    context: &IterContext<impl BizDayProcessor>,
    must_advance: bool,
) -> Option<NaiveDateTime> {
    match cycle {
        Cycle::AsIs => Some(after.clone()),
        Cycle::ForEach => Some(after.clone()),
        Cycle::Values(values) => {
            let mut current_year = after.year();
            let current_month = after.month();

            loop {
                // Try to find a month in the current year
                if let Some(&next_month) = values.iter().find(|&&m| m >= current_month) {
                    if next_month == current_month {
                        return Some(after.clone());
                    } else {
                        return Some(
                            NaiveDate::from_ymd_opt(current_year, next_month, 1)
                                .unwrap()
                                .and_hms_opt(0, 0, 0)
                                .unwrap(),
                        );
                    }
                }

                // No more valid months in this year, advance to the next year
                current_year += 1;
                if let Some(&first_month) = values.iter().next() {
                    return Some(
                        NaiveDate::from_ymd_opt(current_year, first_month, 1)
                            .unwrap()
                            .and_hms_opt(0, 0, 0)
                            .unwrap(),
                    );
                } else {
                    // The set of months is empty, which is invalid.
                    return None;
                }
            }
        }
        Cycle::NextNth(n) => {
            // Align the cadence to context.start_dt so that, for example,
            // a 3M cycle starting in June always ticks to Sep, Dec, Mar, Jun …
            // regardless of which year we're in.
            let start_month = context.start_dt.month() as i32;
            let start_year = context.start_dt.year();
            let total_months =
                (after.year() - start_year) * 12 + (after.month() as i32 - start_month);
            let remainder = total_months.rem_euclid(*n as i32);

            if !must_advance && remainder == 0 {
                // Current month is already aligned. Stay here and let the day
                // component find the next occurrence within this period (it will
                // roll forward naturally when this month is exhausted).
                return Some(after.clone());
            }

            let months_to_add = *n - remainder as u32;
            let (year, month) = ffwd_months(after, months_to_add);
            Some(
                NaiveDate::from_ymd_opt(year as i32, month, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
            )
        }
    }
}

pub(super) fn ffwd_months(dtm: &NaiveDateTime, num: u32) -> (u32, u32) {
    let mut new_month = dtm.month() + num;
    let mut new_year = dtm.year() as u32;
    new_year += (new_month - 1) / 12;
    new_month = (new_month - 1) % 12 + 1;
    (new_year, new_month)
}

pub(super) fn last_day_of_month(year: i32, month: u32) -> NaiveDate {
    if month == 12 {
        NaiveDate::from_ymd_opt(year, 12, 31).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)
            .unwrap()
            .pred_opt()
            .unwrap()
    }
}
