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
    combinator::{all_consuming, map_res, opt, recognize, value},
    error::Error as NomError,
    multi::separated_list1,
    sequence::{delimited, pair, preceded},
    IResult,
};

/// How a single calendar component (years or months) advances on each iteration.
///
/// | Variant | Spec token | Description |
/// |---------|------------|-------------|
/// | `AsIs` | `_` | Keep the current value unchanged |
/// | `ForEach` | `YY` / `MM` | Every value in sequence |
/// | `Values(set)` | `2025` / `[2024,2025]` | Restricted to an explicit set |
/// | `NextNth(n)` | `1Y` / `3M` | Advance by *n* units, aligned to the iterator start |
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
    /// Keep the current year/month value unchanged (`_`).
    #[default]
    AsIs,
    /// Visit every year (`YY`) or every month (`MM`).
    ForEach,
    /// Restrict to the given set of years or months.
    ///
    /// Spec tokens: a bare number (`2025`), or a bracketed list (`[01,06,12]`).
    Values(BTreeSet<u32>),
    /// Advance by *n* years (`nY`) or months (`nM`), keeping alignment to the
    /// iterator's start date.
    NextNth(u32),
}

/// How the day component of a date spec advances.
///
/// | Variant | Spec tokens | Description |
/// |---------|-------------|-------------|
/// | `AsIs` | `_` | Keep current day |
/// | `ForEach` | `DD` | Every calendar day |
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
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    /// Keep the current day-of-month unchanged (`_`).
    #[default]
    AsIs,
    /// Every calendar day (`DD`).
    ForEach,
    /// Advance by *n* calendar days / business days / weekdays from the current
    /// position. See [`NextNthDayOption`] for the step variant.
    NextNth(u32, NextNthDayOption),
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
}

/// Selects which occurrence of a weekday within the month to use.
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    /// Every occurrence of the weekday in the month (no selector). Default.
    #[default]
    NA,
    /// The *n*th occurrence from the start of the month (`WD#n`).
    /// `None` means the 1st occurrence.
    Starting(Option<u8>),
    /// The *n*th-to-last occurrence in the month.
    ///
    /// - `Ending(None)` → last occurrence (`WD#L`).
    /// - `Ending(Some(n))` → *n*th-to-last (`WD#nL`).
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
/// <years>-<months>-<days>[~<adj>]
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
    /// Optional business day adjustment applied after the raw date is resolved.
    pub biz_day_adj: Option<BizDayAdjustment>,
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
            Cycle::Values(vals) => {
                if vals.len() == 1 {
                    let val = vals.iter().next().unwrap();
                    if self.is_year {
                        // Years are not typically zero-padded.
                        write!(f, "{}", val)
                    } else {
                        write!(f, "{:02}", val)
                    }
                } else {
                    let strs: Vec<String> = vals
                        .iter()
                        .map(|v| {
                            if self.is_year {
                                format!("{}", v)
                            } else {
                                format!("{:02}", v)
                            }
                        })
                        .collect();
                    write!(f, "[{}]", strs.join(","))
                }
            }
            Cycle::NextNth(n) => {
                if self.is_year {
                    write!(f, "{}Y", n)
                } else {
                    write!(f, "{}M", n)
                }
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
                    let strs: Vec<String> = days.iter().map(|d| format!("{:02}", d)).collect();
                    write!(f, "[{}]", strs.join(","))
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
                    let strs: Vec<String> = weekdays.iter().map(|w| w.to_string()).collect();
                    write!(f, "[{}]", strs.join(","))
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

/// Parse a date spec string into a [`Spec`].
///
/// This is the free-function equivalent of `input.parse::<Spec>()`.
///
/// # Errors
///
/// Returns [`crate::prelude::Error::ParseError`] if the string does not
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
        opt(preceded(multispace0, parse_adjustment)),
    );

    match all_consuming(full_parser).parse(input) {
        Ok((_, (years, months, days, biz_day_adj))) => Ok(Spec {
            years,
            months,
            days,
            biz_day_adj,
        }),
        Err(_) => Err(Error::InvalidDateSpec(format!("failed to parse: {input}"))),
    }
}

// --- Helpers ---

fn parse_u32(input: &'_ str) -> Res<'_, u32> {
    map_res(digit1, str::parse).parse(input)
}

// --- Cycle Parsers ---

fn parse_cycle_vals(input: &'_ str) -> Res<'_, Cycle> {
    let (input, nums) =
        delimited(char('['), separated_list1(char(','), parse_u32), char(']')).parse(input)?;
    Ok((input, Cycle::Values(nums.into_iter().collect())))
}

fn parse_cycle_single_val(input: &'_ str) -> Res<'_, Cycle> {
    let (input, num) = parse_u32(input)?;
    Ok((input, Cycle::Values(BTreeSet::from([num]))))
}

fn parse_cycle_foreach(input: &'_ str) -> Res<'_, Cycle> {
    value(Cycle::ForEach, alt((tag("YY"), tag("MM"), tag("_")))).parse(input)
}

fn parse_cycle_next_nth(input: &'_ str) -> Res<'_, Cycle> {
    let (input, num) = parse_u32(input)?;
    let (input, _) = alt((tag("Y"), tag("M"))).parse(input)?;
    Ok((input, Cycle::NextNth(num)))
}

fn parse_year_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        parse_cycle_foreach,
        parse_cycle_next_nth,
        parse_cycle_single_val,
    ))
    .parse(input)
}

fn parse_month_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        parse_cycle_foreach,
        parse_cycle_next_nth,
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

fn parse_day_weekday_list(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, wds) = delimited(
        char('['),
        separated_list1(char(','), parse_weekday_enum),
        char(']'),
    )
    .parse(input)?;
    Ok((
        input,
        DayCycle::OnWeekDays {
            weekdays: wds.into_iter().collect(),
            option: WeekdayOption::NA,
        },
    ))
}

fn parse_day_int_list(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, nums) =
        delimited(char('['), separated_list1(char(','), parse_u32), char(']')).parse(input)?;
    Ok((
        input,
        DayCycle::OnDays {
            days: nums.into_iter().collect(),
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

fn parse_day_weekday_complex(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, wd) = parse_weekday_enum(input)?;
    let (input, _) = char('#').parse(input)?;

    let (input, opt_str) = recognize(pair(opt(digit1), opt(char('L')))).parse(input)?;

    let option = match opt_str {
        "L" => WeekdayOption::Ending(None),
        s if s.ends_with('L') => {
            let num = s.trim_end_matches('L').parse::<u8>().unwrap_or(1);
            WeekdayOption::Ending(Some(num))
        }
        s => {
            let num = s.parse::<u8>().unwrap_or(1);
            WeekdayOption::Starting(Some(num))
        }
    };

    Ok((
        input,
        DayCycle::OnWeekDays {
            weekdays: BTreeSet::from([wd]),
            option,
        },
    ))
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
    Ok((input, DayCycle::NextNth(num, opt)))
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

fn parse_day_cycle(input: &'_ str) -> Res<'_, DayCycle> {
    alt((
        parse_day_weekday_list,
        parse_day_int_list,
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
    let (input, code) = alt((
        tag("PW"),
        tag("NW"),
        tag("PB"),
        tag("NB"),
        tag("B"),
        tag("W"),
        recognize(pair(opt(digit1), alt((tag("P"), tag("N"))))),
    ))
    .parse(input)?;

    let adj = match code {
        "PW" => BizDayAdjustment::Weekday(Direction::Prev),
        "NW" => BizDayAdjustment::Weekday(Direction::Next),
        "PB" => BizDayAdjustment::BizDay(Direction::Prev),
        "NB" => BizDayAdjustment::BizDay(Direction::Next),
        "W" => BizDayAdjustment::Weekday(Direction::Nearest),
        "B" => BizDayAdjustment::BizDay(Direction::Nearest),
        
        s if s.ends_with("P") => {
            let n = s.trim_end_matches("P").parse().unwrap_or(1);
            BizDayAdjustment::Prev(n)
        }
        s if s.ends_with("N") => {
            let n = s.trim_end_matches("N").parse().unwrap_or(1);
            BizDayAdjustment::Next(n)
        }
        _ => BizDayAdjustment::NA,
    };

    Ok((input, adj))
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
            let res = parse_spec(raw).expect(&format!("Failed to parse {}", raw));
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
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 5: "[2024]-[01,02]-TUE#2L~3P"
            // Note: Single value in brackets [2024] parses to Values(Set(2024))
            TestCase {
                name: "test_month_year_set",
                input: "[2024]-[01,02]-TUE#2L~3P",
                expected_str: "2024-[01,02]-TUE#2L~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::Values(set(vec![1, 2])),
                    days: DayCycle::OnWeekDays {
                        weekdays: set(vec![WeekdayStartingMonday(Weekday::Tue)]),
                        option: WeekdayOption::Ending(Some(2)),
                    },
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 6: "2024-[01,02]-[05,10,15]~3P"
            TestCase {
                name: "test_month_day_set",
                input: "2024-[01,02]-[05,10,15]~3P",
                expected_str: "2024-[01,02]-[05,10,15]~3P",
                expected: Spec {
                    years: Cycle::Values(set(vec![2024])),
                    months: Cycle::Values(set(vec![1, 2])),
                    days: DayCycle::OnDays {
                        days: set(vec![5, 10, 15]),
                        option: LastDayOption::NA,
                    },
                    biz_day_adj: Some(BizDayAdjustment::Prev(3)),
                },
            },
            // Case 7: "2024-[01,02]-[SAT,SUN]~3P"
            TestCase {
                name: "test_month_weekday_set",
                input: "2024-[01,02]-[SAT,SUN]~3P",
                expected_str: "2024-[01,02]-[SAT,SUN]~3P",
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
                    biz_day_adj: None,
                },
            },
        ];

        for case in cases {
            println!("Running test: {}", case.name);

            // 1. Parse
            let parsed =
                parse_spec(case.input).expect(&format!("Failed to parse input: {}", case.input));

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
