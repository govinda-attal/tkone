use super::easter::Easter;
use crate::{biz_day::Direction, prelude::*, utils::WeekdayStartingMonday};
use chrono::Weekday;

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use nom::{
    Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, multispace0},
    combinator::{all_consuming, map_res, opt, value},
    error::Error as NomError,
    multi::separated_list1,
    sequence::{delimited, pair, preceded},
    IResult,
};

/// How a single calendar component (years or months) advances on each iteration.
///
/// | Variant | Spec token | Description |
/// |---------|------------|-------------|
/// | `AsIs` | — | Not produced by the parser; behaves like `ForEach` |
/// | `ForEach` | `YY` / `MM` / `_` | Every value in sequence |
/// | `Values(set)` | `2025` / `[2024,2025]` | Restricted to an explicit set |
/// | `NextNth(n)` | `1Y` / `3M` | Advance by *n* units, aligned to the iterator start |
/// | `Positions{n, set}` | `4Y[1,3]` / `3M[1-2]` | Selected positions of an *n*-cycle from the start |
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::date::Cycle;
/// use std::collections::BTreeSet;
///
/// // Parse from a date spec component
/// let spec: tkone_schedule::date::Spec = "YY-3M-15".parse().unwrap();
/// assert_eq!(spec.years,  Cycle::ForEach);
/// assert_eq!(spec.months, Cycle::NextNth(3));
/// assert_eq!(spec.months, Cycle::NextNth(3));
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub enum Cycle {
    /// Not produced by the parser (`_` parses as [`Cycle::ForEach`]). If
    /// constructed directly, behaves like `ForEach`.
    #[default]
    AsIs,
    /// Visit every year (`YY`) or every month (`MM`); `_` is a synonym.
    ForEach,
    /// Restrict to the given set of years or months.
    ///
    /// Spec tokens: a bare number (`2025`), or a bracketed list (`[01,06,12]`).
    Values(BTreeSet<u32>),
    /// Advance by *n* years (`nY`) or months (`nM`), keeping alignment to the
    /// iterator's start date.
    NextNth(u32),
    /// Selected positions of a cycle of *n* years or months, counted from the
    /// start (1 is the start's year / month): `4Y[1,3]`, `3M[1-2]`.
    Positions {
        /// The cycle length *n*.
        every: u32,
        /// The positions in each cycle, 1-based.
        positions: BTreeSet<u32>,
    },
}

/// A cycle of weeks after the day rule (`/nW`, `/nW[…]`): only dates in the
/// selected weeks of each *n*-week cycle are kept. Weeks run Monday to Sunday;
/// week 1 is the one containing the start.
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct WeekCycle {
    /// The cycle length *n*.
    pub every: u32,
    /// The weeks kept in each cycle, 1-based. `/2W` is `/2W[1]`.
    pub positions: BTreeSet<u32>,
}

/// How the day component of a date spec advances.
///
/// | Variant | Spec tokens | Description |
/// |---------|-------------|-------------|
/// | `AsIs` | — | Not produced by the parser; the anchor's day of month |
/// | `ForEach` | `DD` / `_` | Every calendar day |
/// | `NextNth(n, Regular)` | `7D` | Advance *n* calendar days |
/// | `NextNth(n, BizDay)` | `5BD` | Advance *n* business days |
/// | `NextNth(n, WeekDay)` | `3WD` | Advance *n* weekdays (Mon–Fri) |
/// | `OnDays{days, NA}` | `15` / `[5,15,25]` | Specific day(s) of month |
/// | `OnDays{days, LastDay}` | `31L` | Day or last day if month is shorter |
/// | `OnDays{{}, LastDay}` | `L` | Last day of month |
/// | `OnDays{days, NextMonthFirstDay}` | `31N` | Day or 1st of next month on overflow |
/// | `OnDays{days, NextMonthOverflow}` | `31O` | Day or overflow into next month |
/// | `OnWeekDays{wds, NA}` | `FRI` / `[MON,WED]` | Specific weekday(s), every occurrence |
/// | `OnWeekDays{wd, Starting(n)}` | `FRI#2` | *n*th occurrence of weekday from month start |
/// | `OnWeekDays{wd, Ending(None)}` | `FRI#L` | Last occurrence of weekday in month |
/// | `OnWeekDays{wd, Ending(n)}` | `FRI#2L` | *n*th-to-last occurrence of weekday |
/// | `NthBizDay{BizDay, Starting(n)}` | `BD#3` | *n*th business day of the month |
/// | `NthBizDay{BizDay, Ending(None)}` | `BD#L` | Last business day of the month |
/// | `NthBizDay{BizDay, Ending(n)}` | `BD#2L` | *n*th-to-last business day |
/// | `NthBizDay{BizDay, [Starting(1)…Starting(5)]}` | `BD#[1-5]` | 1st to 5th business days |
/// | `NthBizDay{WeekDay, …}` | `WD#3` / `WD#L` | Same, counting weekdays (Mon–Fri) |
/// | `Easter{Western, 0}` | `EAS` | Easter Sunday |
/// | `Easter{Western, -2}` | `EAS-2` | 2 days before Easter Sunday (Good Friday) |
/// | `Easter{Orthodox, 1}` | `OEAS+1` | the day after Orthodox Easter Sunday |
/// | `Positions{8, {1,2,3,4}}` | `8D[1-4]` | Days 1–4 of every 8-day cycle from the start |
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    /// Not produced by the parser (`_` parses as [`DayCycle::ForEach`]). If
    /// constructed directly, the iterator's start day of month, clamped to
    /// the last day of shorter months.
    #[default]
    AsIs,
    /// Every calendar day (`DD`); `_` is a synonym.
    ForEach,
    /// Advance by *n* calendar days / business days / weekdays from the current
    /// position. See [`NextNthDayOption`] for the step variant.
    NextNth(u32, NextNthDayOption),
    /// Selected days of a cycle of *n* calendar days, counted from the start
    /// (day 1 is the start's day): `8D[1-4]` is "4 on, 4 off".
    Positions {
        /// The cycle length *n*.
        every: u32,
        /// The days kept in each cycle, 1-based.
        positions: BTreeSet<u32>,
    },
    /// Specific day(s) of the month, with an optional overflow handling rule.
    OnDays {
        /// The set of target days-of-month (1–31). Empty only for `LastDay` with
        /// no explicit day (`"L"`).
        days: BTreeSet<u32>,
        /// What to do when a target day does not exist in the current month.
        option: LastDayOption,
    },
    /// Specific weekday(s) within the month, with an optional occurrence selector.
    OnWeekDays {
        /// The set of target weekdays.
        weekdays: BTreeSet<WeekdayStartingMonday>,
        /// Which occurrence of the weekday to use.
        option: WeekdayOption,
    },
    /// The *n*th business day (`BD#n`) or weekday (`WD#n`) of the month,
    /// counted from the end with `L` (`BD#L`, `BD#2L`), or several of them
    /// (`BD#[1-5]`, `BD#[1,L]`). A month with fewer such days has no
    /// occurrence for the missing positions.
    NthBizDay {
        /// Which days count.
        calendar: BizDayCalendar,
        /// Which of them to use, each `Starting(Some(n))` or `Ending(…)`.
        options: Vec<WeekdayOption>,
    },
    /// A day counted in calendar days from Easter Sunday (`EAS`, `EAS-2`,
    /// `OEAS+1`). The month rule still filters: `YY-04-EAS` is Easter Sunday
    /// in the years it falls in April.
    Easter {
        /// Which Easter.
        easter: Easter,
        /// Calendar days after (positive) or before (negative) Easter Sunday,
        /// within [`EASTER_OFFSETS`].
        offset: i32,
    },
}

/// The offsets an Easter rule accepts. Every Western Easter (22 March to 25
/// April) plus any of them is in Easter's own year, from 1 January to
/// 31 December.
pub const EASTER_OFFSETS: std::ops::RangeInclusive<i32> = -80..=250;

/// The days counted by [`DayCycle::NthBizDay`].
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum BizDayCalendar {
    /// Business days according to the iterator's
    /// [`crate::biz_day::BizDayProcessor`] (`BD#…`). Default.
    #[default]
    BizDay,
    /// Weekdays (Mon–Fri), using [`crate::biz_day::WeekendSkipper`] (`WD#…`).
    WeekDay,
}

/// Selects which occurrence of a weekday (or, for [`DayCycle::NthBizDay`],
/// of a business day) within the month to use.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    /// Every occurrence of the weekday in the month (no selector). Default.
    #[default]
    NA,
    /// The *n*th occurrence from the start of the month (`MON#n`, `BD#n`).
    /// `None` means the 1st occurrence.
    Starting(Option<u8>),
    /// The *n*th-to-last occurrence in the month.
    ///
    /// - `Ending(None)` → last occurrence (`MON#L`, `BD#L`).
    /// - `Ending(Some(n))` → *n*th-to-last (`MON#nL`, `BD#nL`).
    Ending(Option<u8>),
}

/// What to do when the target day-of-month does not exist in the current month.
///
/// For example, day 31 in February or day 29 in a non-leap-year February.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum LastDayOption {
    /// No special handling; the day is clamped to the last valid day of the
    /// month automatically by the calendar. Default.
    #[default]
    NA,
    /// Clamp to the last day of the month and emit a [`crate::Occurrence::Exact`]
    /// result (`ddL` / `L`).
    LastDay,
    /// Use the 1st of the following month as the *observed* date; the raw
    /// calendar date is still the last day of the current month.
    /// Yields [`crate::Occurrence::AdjustedLater`] (`ddN`).
    NextMonthFirstDay,
    /// Overflow into the next month by the excess days.
    /// Yields [`crate::Occurrence::AdjustedLater`] (`ddO`).
    NextMonthOverflow,
}

/// The step unit used by [`DayCycle::NextNth`].
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum NextNthDayOption {
    /// Advance by *n* calendar days (`nD`). Default.
    #[default]
    Regular,
    /// Advance by *n* business days according to the [`crate::biz_day::BizDayProcessor`]
    /// supplied to the iterator (`nBD`).
    BizDay,
    /// Advance by *n* weekdays (Mon–Fri), always using [`crate::biz_day::WeekendSkipper`]
    /// internally (`nWD`).
    WeekDay,
}

/// Post-processing adjustment applied to the raw calendar date.
///
/// Directional variants (`Weekday`, `BizDay`) are **conditional**: the
/// adjustment only fires when the raw date is *not* already a valid
/// business/week day. Numeric variants (`Prev`, `Next`) are **unconditional**
/// offsets applied every time.
///
/// | Spec token | Variant | Condition | Effect |
/// |------------|---------|-----------|--------|
/// | `~W` | `Weekday(Nearest)` | weekend | roll to nearest weekday (Mon or Fri) |
/// | `~NW` | `Weekday(Next)` | weekend | roll to next weekday |
/// | `~PW` | `Weekday(Prev)` | weekend | roll to previous weekday |
/// | `~B` | `BizDay(Nearest)` | non-biz day | roll to nearest business day |
/// | `~NB` | `BizDay(Next)` | non-biz day | roll to next business day |
/// | `~PB` | `BizDay(Prev)` | non-biz day | roll to previous business day |
/// | `~MF` | `ModifiedFollowing` | non-biz day | next business day, or previous if that is in the next month |
/// | `~SB` | `SkipNonBizDay` | non-biz day | drop the date (no result) |
/// | `~MP` | `ModifiedPreceding` | non-biz day | previous business day, or next if that is in the previous month |
/// | `~3P` | `Prev(3)` | always | 3 business days earlier |
/// | `~2N` | `Next(2)` | always | 2 business days later |
#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum BizDayAdjustment {
    /// No adjustment (`~NA`). Default.
    #[default]
    NA,
    /// Roll to a nearby weekday (Mon–Fri) using [`crate::biz_day::WeekendSkipper`].
    Weekday(Direction),
    /// Roll to a nearby business day using the iterator's
    /// [`crate::biz_day::BizDayProcessor`].
    BizDay(Direction),
    /// Skip (`~SB`): a date that is not a business day produces no result,
    /// instead of moving to one. `YY-MM-[MON-FRI]/2W~SB` is "business days,
    /// every other week".
    SkipNonBizDay,
    /// Modified Following (`~MF`): roll to the next business day, unless that
    /// is in a later month, in which case roll to the previous one.
    ModifiedFollowing,
    /// Modified Preceding (`~MP`): roll to the previous business day, unless
    /// that is in an earlier month, in which case roll to the next one.
    ModifiedPreceding,
    /// Unconditionally retreat by *n* business days (`~nP`).
    Prev(u32),
    /// Unconditionally advance by *n* business days (`~nN`).
    Next(u32),
}

/// A fully parsed date recurrence specification.
///
/// Obtained by parsing a spec string with [`str::parse`] or
/// [`std::str::FromStr`], or by calling [`parse_spec`] directly.
///
/// # Spec format
///
/// ```text
/// <years>-<months>-<days>[/<weeks>][~<adj>]
/// ```
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::date::{Spec, Cycle, DayCycle, LastDayOption};
/// use std::collections::BTreeSet;
///
/// // Every year, every month, last day of month
/// let spec: Spec = "YY-MM-L".parse().unwrap();
/// assert_eq!(spec.years,  Cycle::ForEach);
/// assert_eq!(spec.months, Cycle::ForEach);
/// assert!(matches!(spec.days, DayCycle::OnDays { option: LastDayOption::LastDay, .. }));
/// assert!(spec.biz_day_adj.is_none());
///
/// // Every year, every 3 months, 15th of the month
/// let spec: Spec = "YY-3M-15".parse().unwrap();
/// assert_eq!(spec.months, Cycle::NextNth(3));
///
/// // Round-trip
/// assert_eq!(spec.to_string(), "YY-3M-15");
/// ```
#[derive(Debug, Clone)]
pub struct Spec {
    /// Year recurrence rule.
    pub years: Cycle,
    /// Month recurrence rule.
    pub months: Cycle,
    /// Day-of-month / weekday recurrence rule.
    pub days: DayCycle,
    /// Optional week cycle (`/2W`, `/4W[1,2]`) that keeps only dates in the
    /// selected weeks.
    pub weeks: Option<WeekCycle>,
    /// Optional business day adjustment applied after the raw date is resolved.
    pub biz_day_adj: Option<BizDayAdjustment>,
}

/// A bracketed list, with runs of consecutive values written as ranges:
/// `[01-06,12]`, `[MON-FRI]`, `[2025-2027]`. `items` are ascending
/// `(position, label)` pairs; a single value is written without brackets.
fn fmt_list(items: impl IntoIterator<Item = (u32, String)>) -> String {
    let mut runs: Vec<((u32, String), (u32, String))> = Vec::new();
    for (key, label) in items {
        match runs.last_mut() {
            Some((_, last)) if last.0 + 1 == key => *last = (key, label),
            _ => runs.push(((key, label.clone()), (key, label))),
        }
    }
    let parts: Vec<String> = runs
        .iter()
        .map(|(first, last)| if first.0 == last.0 { first.1.clone() } else { format!("{}-{}", first.1, last.1) })
        .collect();
    match (runs.as_slice(), parts.as_slice()) {
        ([(first, last)], [one]) if first.0 == last.0 => one.clone(),
        _ => format!("[{}]", parts.join(",")),
    }
}

/// The position of a `Starting` business-day option (1 for `NA`).
fn n_of(option: &WeekdayOption) -> u8 {
    match option {
        WeekdayOption::Starting(n) | WeekdayOption::Ending(n) => n.unwrap_or(1),
        WeekdayOption::NA => 1,
    }
}

/// Positions are always bracketed: `[1]`, `[1-2]`, `[1,3]`.
fn fmt_positions(positions: &BTreeSet<u32>) -> String {
    let list = fmt_list(positions.iter().map(|&p| (p, p.to_string())));
    if list.starts_with('[') { list } else { format!("[{list}]") }
}

impl fmt::Display for WeekCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/{}W", self.every)?;
        if self.positions != BTreeSet::from([1]) {
            write!(f, "{}", fmt_positions(&self.positions))?;
        }
        Ok(())
    }
}

/// Helper struct to provide context for displaying a `Cycle`.
struct CycleDisplayer<'a> {
    cycle: &'a Cycle,
    is_year: bool,
}

impl Cycle {
    /// Returns a helper struct that can display the cycle with context.
    fn display(&self, is_year: bool) -> CycleDisplayer<'_> {
        CycleDisplayer {
            cycle: self,
            is_year,
        }
    }
}

impl<'a> fmt::Display for CycleDisplayer<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.cycle {
            Cycle::AsIs => write!(f, "_"),
            Cycle::ForEach => {
                if self.is_year {
                    write!(f, "YY")
                } else {
                    write!(f, "MM")
                }
            }
            // Years are not zero-padded; months are.
            Cycle::Values(vals) => write!(
                f,
                "{}",
                fmt_list(vals.iter().map(|&v| (v, if self.is_year { v.to_string() } else { format!("{v:02}") })))
            ),
            Cycle::NextNth(n) => {
                if self.is_year {
                    write!(f, "{}Y", n)
                } else {
                    write!(f, "{}M", n)
                }
            }
            Cycle::Positions { every, positions } => {
                write!(f, "{every}{}{}", if self.is_year { 'Y' } else { 'M' }, fmt_positions(positions))
            }
        }
    }
}

// --- 2. Weekday Display ---
impl fmt::Display for WeekdayStartingMonday {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self.0 {
            chrono::Weekday::Mon => "MON",
            chrono::Weekday::Tue => "TUE",
            chrono::Weekday::Wed => "WED",
            chrono::Weekday::Thu => "THU",
            chrono::Weekday::Fri => "FRI",
            chrono::Weekday::Sat => "SAT",
            chrono::Weekday::Sun => "SUN",
        };
        write!(f, "{}", s)
    }
}

// --- 3. DayCycle Display ---
impl fmt::Display for DayCycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DayCycle::AsIs => write!(f, "_"),
            DayCycle::ForEach => write!(f, "DD"),
            DayCycle::NextNth(n, opt) => {
                let suffix = match opt {
                    NextNthDayOption::BizDay => "BD",
                    NextNthDayOption::WeekDay => "WD",
                    NextNthDayOption::Regular => "D",
                };
                write!(f, "{}{}", n, suffix)
            }
            DayCycle::Positions { every, positions } => write!(f, "{every}D{}", fmt_positions(positions)),
            DayCycle::OnDays { days, option } => {
                // If specific option like L is present and days is empty
                if days.is_empty() && matches!(option, LastDayOption::LastDay) {
                    return write!(f, "L");
                }

                // If single day
                if days.len() == 1 {
                    let d = days.iter().next().unwrap();
                    write!(f, "{:02}", d)?;
                    match option {
                        LastDayOption::LastDay => write!(f, "L"),
                        LastDayOption::NextMonthFirstDay => write!(f, "N"),
                        LastDayOption::NextMonthOverflow => write!(f, "O"),
                        _ => Ok(()),
                    }
                } else {
                    write!(f, "{}", fmt_list(days.iter().map(|&d| (d, format!("{d:02}")))))
                }
            }
            DayCycle::OnWeekDays { weekdays, option } => {
                if weekdays.len() == 1 {
                    let w = weekdays.iter().next().unwrap();
                    write!(f, "{}", w)?;
                    match option {
                        WeekdayOption::Starting(Some(n)) => write!(f, "#{}", n),
                        WeekdayOption::Ending(None) => write!(f, "#L"),
                        WeekdayOption::Ending(Some(n)) => write!(f, "#{}L", n),
                        _ => Ok(()),
                    }
                } else {
                    let items = weekdays.iter().map(|w| (w.0.num_days_from_monday(), w.to_string()));
                    write!(f, "{}", fmt_list(items))
                }
            }
            DayCycle::NthBizDay { calendar, options } => {
                let unit = match calendar {
                    BizDayCalendar::BizDay => "BD",
                    BizDayCalendar::WeekDay => "WD",
                };
                // Positions from the start (runs as ranges), then from the end.
                let starting: Vec<(u32, String)> = options
                    .iter()
                    .filter_map(|o| match o {
                        WeekdayOption::Starting(_) | WeekdayOption::NA => {
                            let n = u32::from(n_of(o));
                            Some((n, n.to_string()))
                        }
                        WeekdayOption::Ending(_) => None,
                    })
                    .collect();
                let mut parts: Vec<String> = Vec::new();
                if !starting.is_empty() {
                    let list = fmt_list(starting);
                    parts.push(list.trim_start_matches('[').trim_end_matches(']').to_string());
                }
                for o in options {
                    match o {
                        WeekdayOption::Ending(None | Some(1)) => parts.push("L".into()),
                        WeekdayOption::Ending(Some(n)) => parts.push(format!("{n}L")),
                        _ => {}
                    }
                }
                match (options.len(), parts.as_slice()) {
                    (1, [one]) => write!(f, "{unit}#{one}"),
                    _ => write!(f, "{unit}#[{}]", parts.join(",")),
                }
            }
            DayCycle::Easter { easter, offset } => {
                let name = match easter {
                    Easter::Western => "EAS",
                    Easter::Orthodox => "OEAS",
                };
                match offset {
                    0 => write!(f, "{name}"),
                    _ => write!(f, "{name}{offset:+}"),
                }
            }
        }
    }
}

// --- 4. Adjustment Display ---
impl fmt::Display for BizDayAdjustment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BizDayAdjustment::NA => Ok(()),
            BizDayAdjustment::BizDay(Direction::Next) => write!(f, "~NB"),
            BizDayAdjustment::BizDay(Direction::Prev) => write!(f, "~PB"),
            BizDayAdjustment::BizDay(Direction::Nearest) => write!(f, "~B"),
            BizDayAdjustment::Weekday(Direction::Next) => write!(f, "~NW"),
            BizDayAdjustment::Weekday(Direction::Prev) => write!(f, "~PW"),
            BizDayAdjustment::Weekday(Direction::Nearest) => write!(f, "~W"),
            BizDayAdjustment::SkipNonBizDay => write!(f, "~SB"),
            BizDayAdjustment::ModifiedFollowing => write!(f, "~MF"),
            BizDayAdjustment::ModifiedPreceding => write!(f, "~MP"),
            BizDayAdjustment::Next(n) => write!(f, "~{}N", n),
            BizDayAdjustment::Prev(n) => write!(f, "~{}P", n),
        }
    }
}

// --- 5. Spec Display (The Root) ---
impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{}-{}",
            self.years.display(true),
            self.months.display(false),
            self.days
        )?;
        if let Some(weeks) = &self.weeks {
            write!(f, "{weeks}")?;
        }

        // Handle Adjustment
        if let Some(adj) = &self.biz_day_adj {
            write!(f, "{}", adj)?;
        }

        Ok(())
    }
}

// !!! FIX: Concrete Type Alias to stop inference errors !!!
type Res<'a, T> = IResult<&'a str, T, NomError<&'a str>>;

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        parse_spec(s)
    }
}

impl Spec {
    /// Whether a cycle has positions other than the first (`/4W[1,2]`,
    /// `8D[1-4]`, `3M[2]`). An occurrence may then fall anywhere in its
    /// cycle, so the cycle cannot be counted from it: resuming needs the
    /// schedule's start.
    pub(crate) fn needs_start(&self) -> bool {
        let first_only = |positions: &BTreeSet<u32>| positions.len() == 1 && positions.contains(&1);
        let cycle = |c: &Cycle| matches!(c, Cycle::Positions { positions, .. } if !first_only(positions));
        cycle(&self.years)
            || cycle(&self.months)
            || matches!(&self.days, DayCycle::Positions { positions, .. } if !first_only(positions))
            || self.weeks.as_ref().is_some_and(|w| !first_only(&w.positions))
    }

    /// Whether the spec asks the business-day processor anything: an
    /// adjustment other than `~NA` and the weekday ones (`~W`, …), `BD#n`, or
    /// `nBD`. Weekday rules use [`crate::biz_day::WeekendSkipper`] instead.
    pub(crate) fn uses_business_days(&self) -> bool {
        let adjusts = !matches!(self.biz_day_adj, None | Some(BizDayAdjustment::NA | BizDayAdjustment::Weekday(_)));
        adjusts
            || matches!(self.days, DayCycle::NthBizDay { calendar: BizDayCalendar::BizDay, .. })
            || matches!(self.days, DayCycle::NextNth(_, NextNthDayOption::BizDay))
    }

    /// How many business days later than its actual date the adjustment can
    /// move a date: `n` for `~nN`, 1 for the others that can move it later
    /// (`~NB`, `~MF`, …), 0 when it never does (`~PB`, `~nP`, none).
    pub(crate) fn forward_reach(&self) -> u32 {
        match &self.biz_day_adj {
            None | Some(BizDayAdjustment::NA | BizDayAdjustment::SkipNonBizDay | BizDayAdjustment::Prev(_)) => 0,
            Some(BizDayAdjustment::Weekday(Direction::Prev) | BizDayAdjustment::BizDay(Direction::Prev)) => 0,
            Some(BizDayAdjustment::Next(n)) => *n,
            Some(_) => 1,
        }
    }

    /// Describes the spec in plain English, to check it reads as intended
    /// before it is scheduled.
    ///
    /// "The start" is the date the iterator is anchored to: the
    /// `new_with_start` start or the `new_after` cursor. `nY` / `nM` cadences
    /// and day steps are counted from it.
    ///
    /// ```rust
    /// use tkone_schedule::date::Spec;
    /// let describe = |s: &str| s.parse::<Spec>().unwrap().describe();
    /// assert_eq!(describe("YY-MM-FRI#L"), "on the last Friday of every month");
    /// assert_eq!(
    ///     describe("YY-3M-31L~NB"),
    ///     "on the 31st of every 3rd month, counted from the start's month \
    ///      (or the last day, in shorter months), \
    ///      moved to the next business day when it is not one"
    /// );
    /// ```
    pub fn describe(&self) -> String {
        super::describe::describe(self)
    }
}

/// Parse a date spec string into a [`Spec`].
///
/// This is the free-function equivalent of `input.parse::<Spec>()`.
///
/// # Errors
///
/// Returns [`Error::InvalidDateSpec`](crate::Error::InvalidDateSpec) if the string does not
/// conform to the spec grammar.
///
/// # Examples
///
/// ```rust
/// use tkone_schedule::date::parse_spec;
///
/// let spec = parse_spec("YY-1M-31L~W").unwrap();
/// println!("{spec}"); // round-trips back to "YY-1M-31L~W"
/// ```
pub fn parse_spec(input: &str) -> Result<Spec> {
    let full_parser = (
        parse_year_cycle,
        preceded(char('-'), parse_month_cycle),
        preceded(char('-'), parse_day_cycle),
        opt(parse_week_cycle),
        opt(preceded(multispace0, parse_adjustment)),
    );

    let spec = match all_consuming(full_parser).parse(input) {
        Ok((_, (years, months, days, weeks, biz_day_adj))) => Spec {
            years,
            months,
            days,
            weeks,
            biz_day_adj,
        },
        Err(_) => return Err(Error::InvalidDateSpec(format!("failed to parse: {input}"))),
    };
    validate(&spec).map_err(|reason| Error::InvalidDateSpec(format!("{input}: {reason}")))?;
    Ok(spec)
}

/// Reject values the grammar accepts but no calendar can satisfy.
/// A cycle of `every` with positions 1..=every.
fn validate_cycle(what: &str, every: u32, positions: &BTreeSet<u32>) -> core::result::Result<(), String> {
    if every == 0 {
        return Err(format!("{what} cycle must be at least 1"));
    }
    if let Some(p) = positions.iter().find(|p| !(1..=every).contains(*p)) {
        return Err(format!("{what} position {p} is out of range 1-{every}"));
    }
    Ok(())
}

fn validate(spec: &Spec) -> core::result::Result<(), String> {
    if let Cycle::Positions { every, positions } = &spec.years {
        validate_cycle("year", *every, positions)?;
        if matches!(spec.months, Cycle::NextNth(_) | Cycle::Positions { .. }) {
            return Err("a year cycle with positions cannot be combined with a month cycle".into());
        }
    }
    if let Cycle::Positions { every, positions } = &spec.months {
        validate_cycle("month", *every, positions)?;
        if matches!(spec.years, Cycle::NextNth(_)) {
            return Err("a month cycle with positions cannot be combined with a year step".into());
        }
    }
    if let DayCycle::Positions { every, positions } = &spec.days {
        validate_cycle("day", *every, positions)?;
    }
    if let Some(weeks) = &spec.weeks {
        validate_cycle("week", weeks.every, &weeks.positions)?;
        if matches!(spec.days, DayCycle::NextNth(..)) {
            return Err("a week cycle (`/nW`) cannot follow a day step (`nD`, `nBD`, `nWD`)".into());
        }
        if matches!(spec.days, DayCycle::Easter { .. }) {
            return Err("a week cycle (`/nW`) cannot follow an Easter rule (`EAS`, `OEAS`)".into());
        }
    }
    if let Cycle::NextNth(0) = spec.years {
        return Err("year step must be at least 1".into());
    }
    match &spec.months {
        Cycle::NextNth(0) => return Err("month step must be at least 1".into()),
        Cycle::Values(months) => {
            if let Some(m) = months.iter().find(|m| !(1..=12).contains(*m)) {
                return Err(format!("month {m} is out of range 1-12"));
            }
        }
        _ => {}
    }
    match &spec.days {
        DayCycle::NextNth(0, _) => return Err("day step must be at least 1".into()),
        DayCycle::OnDays { days, .. } => {
            if let Some(d) = days.iter().find(|d| !(1..=31).contains(*d)) {
                return Err(format!("day {d} is out of range 1-31"));
            }
        }
        DayCycle::OnWeekDays {
            option: WeekdayOption::Starting(Some(n)) | WeekdayOption::Ending(Some(n)),
            ..
        } if !(1..=5).contains(n) => {
            return Err(format!("weekday occurrence #{n} is out of range 1-5"));
        }
        DayCycle::Easter { offset, .. } if !EASTER_OFFSETS.contains(offset) => {
            return Err(format!(
                "Easter offset {offset:+} is out of range {}..+{}",
                EASTER_OFFSETS.start(),
                EASTER_OFFSETS.end()
            ));
        }
        DayCycle::NthBizDay { options, .. } => {
            for option in options {
                if let WeekdayOption::Starting(Some(n)) | WeekdayOption::Ending(Some(n)) = option {
                    if !(1..=23).contains(n) {
                        return Err(format!("business day #{n} is out of range 1-23"));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// --- Helpers ---

fn parse_u32(input: &'_ str) -> Res<'_, u32> {
    map_res(digit1, str::parse).parse(input)
}

// --- Cycle Parsers ---

/// Most values a bracketed range may expand to; guards `[1-4000000000]`.
const MAX_RANGE: u32 = 10_000;

/// `[a,b,c-d,…]`: values and inclusive ranges. Ranges must run forwards.
fn parse_u32_list(input: &'_ str) -> Res<'_, BTreeSet<u32>> {
    let span = map_res(
        pair(parse_u32, opt(preceded(char('-'), parse_u32))),
        |(lo, hi): (u32, Option<u32>)| {
            let hi = hi.unwrap_or(lo);
            if lo > hi || hi - lo >= MAX_RANGE {
                Err("invalid range")
            } else {
                Ok(lo..=hi)
            }
        },
    );
    let (input, spans) = delimited(char('['), separated_list1(char(','), span), char(']')).parse(input)?;
    Ok((input, spans.into_iter().flatten().collect()))
}

fn parse_cycle_vals(input: &'_ str) -> Res<'_, Cycle> {
    let (input, nums) = parse_u32_list(input)?;
    Ok((input, Cycle::Values(nums)))
}

fn parse_cycle_single_val(input: &'_ str) -> Res<'_, Cycle> {
    let (input, num) = parse_u32(input)?;
    Ok((input, Cycle::Values(BTreeSet::from([num]))))
}

/// `<n><unit>`: `nY` in the year slot, `nM` in the month slot.
fn parse_cycle_next_nth<'a>(input: &'a str, unit: char) -> Res<'a, Cycle> {
    let (input, num) = parse_u32(input)?;
    let (input, _) = char(unit).parse(input)?;
    let (input, positions) = opt(parse_u32_list).parse(input)?;
    Ok((
        input,
        match positions {
            Some(positions) => Cycle::Positions { every: num, positions },
            None => Cycle::NextNth(num),
        },
    ))
}

/// `/nW` or `/nW[…]` after the day rule.
fn parse_week_cycle(input: &'_ str) -> Res<'_, WeekCycle> {
    let (input, every) = preceded(char('/'), parse_u32).parse(input)?;
    let (input, _) = char('W').parse(input)?;
    let (input, positions) = opt(parse_u32_list).parse(input)?;
    Ok((input, WeekCycle { every, positions: positions.unwrap_or_else(|| BTreeSet::from([1])) }))
}

/// `_` is accepted as a synonym of `YY` / `MM` (every year / month).
fn parse_cycle_foreach<'a>(input: &'a str, wildcard: &'static str) -> Res<'a, Cycle> {
    value(Cycle::ForEach, alt((tag(wildcard), tag("_")))).parse(input)
}

fn parse_year_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        |i| parse_cycle_foreach(i, "YY"),
        |i| parse_cycle_next_nth(i, 'Y'),
        parse_cycle_single_val,
    ))
    .parse(input)
}

fn parse_month_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        |i| parse_cycle_foreach(i, "MM"),
        |i| parse_cycle_next_nth(i, 'M'),
        parse_cycle_single_val,
    ))
    .parse(input)
}

// --- Day Cycle Parsers ---

fn parse_weekday_enum(input: &'_ str) -> Res<'_, WeekdayStartingMonday> {
    let (input, w_str) = alt((
        tag("MON"),
        tag("TUE"),
        tag("WED"),
        tag("THU"),
        tag("FRI"),
        tag("SAT"),
        tag("SUN"),
    ))
    .parse(input)?;

    let w = match w_str {
        "MON" => Weekday::Mon,
        "TUE" => Weekday::Tue,
        "WED" => Weekday::Wed,
        "THU" => Weekday::Thu,
        "FRI" => Weekday::Fri,
        "SAT" => Weekday::Sat,
        "SUN" => Weekday::Sun,
        _ => unreachable!(),
    };
    Ok((input, WeekdayStartingMonday(w)))
}

/// `[MON,WED]`, `[MON-FRI]`, `[MON-WED,FRI]`. A range runs Monday→Sunday
/// and must not wrap (`[FRI-MON]` is rejected).
fn parse_day_weekday_list(input: &'_ str) -> Res<'_, DayCycle> {
    let span = map_res(
        pair(parse_weekday_enum, opt(preceded(char('-'), parse_weekday_enum))),
        |(lo, hi)| {
            let (lo, hi) = (lo.0.num_days_from_monday(), hi.unwrap_or(lo).0.num_days_from_monday());
            if lo > hi {
                Err("weekday range must run Monday to Sunday")
            } else {
                Ok((lo..=hi).map(|d| WeekdayStartingMonday(Weekday::try_from(d as u8).unwrap())))
            }
        },
    );
    let (input, spans) = delimited(char('['), separated_list1(char(','), span), char(']')).parse(input)?;
    Ok((
        input,
        DayCycle::OnWeekDays {
            weekdays: spans.into_iter().flatten().collect(),
            option: WeekdayOption::NA,
        },
    ))
}

fn parse_day_int_list(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, days) = parse_u32_list(input)?;
    Ok((
        input,
        DayCycle::OnDays {
            days,
            option: LastDayOption::NA,
        },
    ))
}

fn parse_day_weekday_solo(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, wd) = parse_weekday_enum(input)?;
    Ok((
        input,
        DayCycle::OnWeekDays {
            weekdays: BTreeSet::from([wd]),
            option: WeekdayOption::NA,
        },
    ))
}

/// `#L`, `#n` or `#nL`; `n` must fit a u8 (range-checked in `validate`).
fn parse_occurrence(input: &'_ str) -> Res<'_, WeekdayOption> {
    let (input, _) = char('#').parse(input)?;
    let (input, (num, last)) = alt((
        value((None, true), char('L')),
        pair(map_res(digit1, str::parse::<u8>).map(Some), opt(char('L')).map(|l| l.is_some())),
    ))
    .parse(input)?;

    let option = match (num, last) {
        (None, _) => WeekdayOption::Ending(None),
        (Some(n), true) => WeekdayOption::Ending(Some(n)),
        (Some(n), false) => WeekdayOption::Starting(Some(n)),
    };
    Ok((input, option))
}

fn parse_day_weekday_complex(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, wd) = parse_weekday_enum(input)?;
    let (input, option) = parse_occurrence(input)?;
    Ok((
        input,
        DayCycle::OnWeekDays {
            weekdays: BTreeSet::from([wd]),
            option,
        },
    ))
}

/// One position in `BD#[…]`: `n`, `n-m` (from the start), `L` or `nL` (from
/// the end).
fn parse_biz_position(input: &'_ str) -> Res<'_, Vec<WeekdayOption>> {
    alt((
        value(vec![WeekdayOption::Ending(None)], char('L')),
        map_res(
            (map_res(digit1, str::parse::<u8>), opt(preceded(char('-'), map_res(digit1, str::parse::<u8>))), opt(char('L'))),
            |(lo, hi, last)| match (hi, last) {
                (None, Some(_)) => Ok(vec![WeekdayOption::Ending(Some(lo))]),
                (None, None) => Ok(vec![WeekdayOption::Starting(Some(lo))]),
                (Some(hi), None) if lo <= hi => Ok((lo..=hi).map(|n| WeekdayOption::Starting(Some(n))).collect()),
                _ => Err("invalid business day range"),
            },
        ),
    ))
    .parse(input)
}

/// `BD#n`, `BD#L`, `BD#nL`, `BD#[1-5]`, `BD#[1,L]`, and the same with `WD`.
fn parse_day_nth_biz(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, calendar) = alt((
        value(BizDayCalendar::BizDay, tag("BD")),
        value(BizDayCalendar::WeekDay, tag("WD")),
    ))
    .parse(input)?;
    let list = preceded(char('#'), delimited(char('['), separated_list1(char(','), parse_biz_position), char(']')));
    let (input, options) = alt((list.map(|ps: Vec<Vec<WeekdayOption>>| ps.concat()), parse_occurrence.map(|o| vec![o]))).parse(input)?;
    // Keep the first of any repeated position.
    let mut unique: Vec<WeekdayOption> = Vec::new();
    for o in options {
        if !unique.contains(&o) {
            unique.push(o);
        }
    }
    Ok((input, DayCycle::NthBizDay { calendar, options: unique }))
}

fn parse_day_next_nth(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, num) = parse_u32(input)?;
    // BD/WD must be tried before plain D to avoid partial match on 'B'/'W'
    let (input, type_tag) = alt((tag("BD"), tag("WD"), tag("D"))).parse(input)?;

    let opt = match type_tag {
        "BD" => NextNthDayOption::BizDay,
        "WD" => NextNthDayOption::WeekDay,
        _ => NextNthDayOption::Regular,
    };
    // Positions are only for calendar-day cycles (`8D[1-4]`); `nBD[…]` and
    // `nWD[…]` do not parse.
    if opt == NextNthDayOption::Regular {
        if let (input, Some(positions)) = opt_positions(input)? {
            return Ok((input, DayCycle::Positions { every: num, positions }));
        }
    }
    Ok((input, DayCycle::NextNth(num, opt)))
}

fn opt_positions(input: &'_ str) -> Res<'_, Option<BTreeSet<u32>>> {
    opt(parse_u32_list).parse(input)
}

fn parse_day_single_complex(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, num) = parse_u32(input)?;
    let (input, suffix) = opt(alt((tag("L"), tag("N"), tag("O")))).parse(input)?;

    let option = match suffix {
        Some("L") => LastDayOption::LastDay,
        Some("N") => LastDayOption::NextMonthFirstDay,
        Some("O") => LastDayOption::NextMonthOverflow,
        _ => LastDayOption::NA,
    };

    Ok((
        input,
        DayCycle::OnDays {
            days: BTreeSet::from([num]),
            option,
        },
    ))
}

fn parse_day_literals(input: &'_ str) -> Res<'_, DayCycle> {
    alt((
        value(DayCycle::ForEach, tag("DD")),
        value(DayCycle::ForEach, tag("_")),
        value(
            DayCycle::OnDays {
                days: BTreeSet::new(),
                option: LastDayOption::LastDay,
            },
            tag("L"),
        ),
    ))
    .parse(input)
}

/// `EAS` or `OEAS`, then an optional signed offset in days: `EAS-2`,
/// `OEAS+1`. The offset is range-checked in `validate`.
fn parse_day_easter(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, easter) = alt((
        value(Easter::Orthodox, tag("OEAS")),
        value(Easter::Western, tag("EAS")),
    ))
    .parse(input)?;
    let (input, offset) = opt(pair(
        alt((value(1, char('+')), value(-1, char('-')))),
        map_res(digit1, str::parse::<i32>),
    ))
    .parse(input)?;
    let offset = offset.map_or(0, |(sign, n)| sign * n);
    Ok((input, DayCycle::Easter { easter, offset }))
}

fn parse_day_cycle(input: &'_ str) -> Res<'_, DayCycle> {
    alt((
        parse_day_easter,
        parse_day_weekday_list,
        parse_day_int_list,
        parse_day_nth_biz,
        parse_day_weekday_complex, // WED#1 before solo, so '#' is consumed first
        parse_day_weekday_solo,    // standalone MON/TUE/... without occurrence suffix
        parse_day_next_nth,
        parse_day_literals,
        parse_day_single_complex,
    ))
    .parse(input)
}

// --- Adjustment Parser ---

fn parse_adjustment(input: &'_ str) -> Res<'_, BizDayAdjustment> {
    let (input, _) = char('~').parse(input)?;
    alt((
        value(BizDayAdjustment::SkipNonBizDay, tag("SB")),
        value(BizDayAdjustment::ModifiedFollowing, tag("MF")),
        value(BizDayAdjustment::ModifiedPreceding, tag("MP")),
        value(BizDayAdjustment::Weekday(Direction::Prev), tag("PW")),
        value(BizDayAdjustment::Weekday(Direction::Next), tag("NW")),
        value(BizDayAdjustment::BizDay(Direction::Prev), tag("PB")),
        value(BizDayAdjustment::BizDay(Direction::Next), tag("NB")),
        value(BizDayAdjustment::BizDay(Direction::Nearest), tag("B")),
        value(BizDayAdjustment::Weekday(Direction::Nearest), tag("W")),
        // `~nP` / `~nN`; a bare `~P` / `~N` means 1
        pair(opt(parse_u32), alt((char('P'), char('N')))).map(|(n, dir)| {
            let n = n.unwrap_or(1);
            if dir == 'P' { BizDayAdjustment::Prev(n) } else { BizDayAdjustment::Next(n) }
        }),
    ))
    .parse(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_parsing() {
        let inputs = vec![
            (
                "YY-MM-15BD~NW",
                Cycle::ForEach,
                Cycle::ForEach,
                DayCycle::NextNth(15, NextNthDayOption::BizDay),
                Some(BizDayAdjustment::Weekday(Direction::Next)),
            ),
            (
                "[2023,2024]-01-01",
                Cycle::Values(BTreeSet::from([2023, 2024])),
                Cycle::Values(BTreeSet::from([1])),
                DayCycle::OnDays {
                    days: BTreeSet::from([1]),
                    option: LastDayOption::NA,
                },
                None,
            ),
        ];

        for (raw, yr, mo, dy, adj) in inputs {
            let res = parse_spec(raw).unwrap_or_else(|_| panic!("Failed to parse {}", raw));
            assert_eq!(res.years, yr);
            assert_eq!(res.months, mo);
            assert_eq!(res.days, dy);
            assert_eq!(res.biz_day_adj, adj);
        }
    }
}

#[cfg(test)]
mod table_tests {
    use super::*;
    use chrono::Weekday;
    use std::collections::BTreeSet;

    // A helper to make BTreeSet construction cleaner in the table
    fn set<T: Ord>(items: Vec<T>) -> BTreeSet<T> {
        BTreeSet::from_iter(items)
    }

    struct TestCase {
        name: &'static str,
        input: &'static str,
        expected: Spec,
        expected_str: &'static str,
        // Optional: expected string if different from input (normalization)
        // expected_str: &'static str,
    }

    #[test]
    fn run_table_tests() {
        let cases = vec![
            // Case 1: "YY-1M-29L~W"
            // Note: Parser maps ~W to BizDay(Next), based on previous 'nom' logic
            TestCase {
                name: "test_one",
                input: "YY-1M-29L~W",
                expected_str: "YY-1M-29L~W",
                expected: Spec {
                    years: Cycle::ForEach,
                    months: Cycle::NextNth(1),
                    days: DayCycle::OnDays {
                        days: set(vec![29]),
                        option: LastDayOption::LastDay,
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Weekday(Direction::Nearest)),
                },
            },
            // Case 2: "YY-1M-1WD"
            TestCase {
                name: "test_two",
                input: "YY-1M-1WD",
                expected_str: "YY-1M-1WD",
                expected: Spec {
                    years: Cycle::ForEach,
                    months: Cycle::NextNth(1),
                    days: DayCycle::NextNth(1, NextNthDayOption::WeekDay),
                    weeks: None,
                    biz_day_adj: None,
                },
            },
            // Case 3: "2024-MM-31L~3P"
            TestCase {
                name: "test_31st",
                input: "2024-MM-31L~3P",
                expected_str: "2024-MM-31L~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::ForEach,
                    days: DayCycle::OnDays {
                        days: set(vec![31]),
                        option: LastDayOption::LastDay,
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 4: "2024-1M-TUE#2L~3P"
            TestCase {
                name: "test_weekday",
                input: "2024-1M-TUE#2L~3P",
                expected_str: "2024-1M-TUE#2L~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::NextNth(1),
                    days: DayCycle::OnWeekDays {
                        weekdays: set(vec![WeekdayStartingMonday(Weekday::Tue)]),
                        option: WeekdayOption::Ending(Some(2)),
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 5: "[2024]-[01,02]-TUE#2L~3P"
            // Note: Single value in brackets [2024] parses to Values(Set(2024))
            TestCase {
                name: "test_month_year_set",
                input: "[2024]-[01,02]-TUE#2L~3P",
                expected_str: "2024-[01-02]-TUE#2L~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::Values(set(vec![1, 2])),
                    days: DayCycle::OnWeekDays {
                        weekdays: set(vec![WeekdayStartingMonday(Weekday::Tue)]),
                        option: WeekdayOption::Ending(Some(2)),
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 6: "2024-[01-02]-[05,10,15]~3P"
            TestCase {
                name: "test_month_day_set",
                input: "2024-[01-02]-[05,10,15]~3P",
                expected_str: "2024-[01-02]-[05,10,15]~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::Values(set(vec![1, 2])),
                    days: DayCycle::OnDays {
                        days: set(vec![5, 10, 15]),
                        option: LastDayOption::NA,
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 7: "2024-[01,02]-[SAT,SUN]~3P"
            TestCase {
                name: "test_month_weekday_set",
                input: "2024-[01,02]-[SAT,SUN]~3P",
                expected_str: "2024-[01-02]-[SAT-SUN]~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::Values(set(vec![1, 2])),
                    days: DayCycle::OnWeekDays {
                        weekdays: set(vec![
                            WeekdayStartingMonday(Weekday::Sat),
                            WeekdayStartingMonday(Weekday::Sun),
                        ]),
                        option: WeekdayOption::NA,
                    },
                    weeks: None,
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 8: "[2023,2025]-MM-30BD"
            // Note: Changed input from "30D" to "30BD" to match parser support
            TestCase {
                name: "test_year_month_day",
                input: "[2023,2025]-MM-30BD",
                expected_str: "[2023,2025]-MM-30BD",
                expected: Spec {
                    years: Cycle::Values(set(vec![2023, 2025])),
                    months: Cycle::ForEach,
                    days: DayCycle::NextNth(30, NextNthDayOption::BizDay),
                    weeks: None,
                    biz_day_adj: None,
                },
            },
            // Case 9: "[2023,2025]-MM-L"
            TestCase {
                name: "test_year_month_day_last",
                input: "[2023,2025]-MM-L",
                expected_str: "[2023,2025]-MM-L",
                expected: Spec {
                    years: Cycle::Values(set(vec![2023, 2025])),
                    months: Cycle::ForEach,
                    days: DayCycle::OnDays {
                        days: set(vec![]),
                        option: LastDayOption::LastDay,
                    },
                    weeks: None,
                    biz_day_adj: None,
                },
            },
        ];

        for case in cases {
            println!("Running test: {}", case.name);

            // 1. Parse
            let parsed =
                parse_spec(case.input).unwrap_or_else(|_| panic!("Failed to parse input: {}", case.input));

            // 2. Assert Struct Equality
            // Using Debug formatting for clearer error diffs if they don't match
            assert_eq!(
                format!("{:?}", parsed),
                format!("{:?}", case.expected),
                "Mismatch in test case '{}'",
                case.name
            );

            // 3. Assert Round-Trip String (Uncomment if Display is implemented)
            assert_eq!(
                parsed.to_string(),
                case.expected_str,
                "String mismatch in '{}'",
                case.name
            );
        }
    }
}
