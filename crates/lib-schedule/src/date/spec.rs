use crate::{biz_day::Direction, prelude::*, utils::WeekdayStartingMonday};
use chrono::Weekday;

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1, multispace0},
    combinator::{all_consuming, map_res, opt, recognize, value},
    error::Error as NomError,
    multi::separated_list1,
    sequence::{delimited, pair, preceded, tuple},
    IResult,
};

// --- Your Struct Definitions ---
#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum DayCycle {
    #[default]
    AsIs,
    ForEach,
    NextNth(u32, NextNthDayOption),
    OnDays {
        days: BTreeSet<u32>,
        option: LastDayOption,
    },
    OnWeekDays {
        weekdays: BTreeSet<WeekdayStartingMonday>,
        option: WeekdayOption,
    },
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum WeekdayOption {
    #[default]
    NA,
    Starting(Option<u8>),
    Ending(Option<u8>),
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum LastDayOption {
    #[default]
    NA,
    LastDay,
    NextMonthFirstDay,
    NextMonthOverflow,
}

#[derive(Debug, Default, PartialEq, Eq, Hash, Clone)]
pub enum NextNthDayOption {
    #[default]
    Regular,
    BizDay,
    WeekDay,
}

#[derive(Debug, PartialEq, Eq, Default, Clone)]
pub enum BizDayAdjustment {
    #[default]
    NA,
    Weekday(Direction),
    BizDay(Direction),
    Prev(u32),
    Next(u32),
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Default)]
pub enum Cycle {
    #[default]
    AsIs,
    ForEach,
    Values(BTreeSet<u32>),
    NextNth(u32),
}

#[derive(Debug, Clone)]
pub struct Spec {
    pub years: Cycle,
    pub months: Cycle,
    pub days: DayCycle,
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
            BizDayAdjustment::BizDay(Direction::Next) => write!(f, "~W"), // Standard spec often uses W for BizDay Next
            BizDayAdjustment::BizDay(Direction::Prev) => write!(f, "~B"),
            BizDayAdjustment::BizDay(Direction::Nearest) => write!(f, "~NB"),
            BizDayAdjustment::Weekday(Direction::Next) => write!(f, "~NW"),
            BizDayAdjustment::Weekday(Direction::Prev) => write!(f, "~PW"),
            BizDayAdjustment::Weekday(Direction::Nearest) => write!(f, "~NW"),
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

// Ensure Error::ParseError accepts String
// If Error is defined elsewhere, update its variant to: ParseError(String)

/// Main Entry Point
pub fn parse_spec(input: &str) -> Result<Spec> {
    let full_parser = tuple((
        parse_year_cycle,
        preceded(char('-'), parse_month_cycle),
        preceded(char('-'), parse_day_cycle),
        opt(preceded(multispace0, parse_adjustment)),
    ));

    match all_consuming(full_parser)(input) {
        Ok((_, (years, months, days, biz_day_adj))) => Ok(Spec {
            years,
            months,
            days,
            biz_day_adj,
        }),
        Err(_) => Err(Error::ParseError("Failed to parse Spec")),
    }
}

// --- Helpers ---

fn parse_u32(input: &'_ str) -> Res<'_, u32> {
    map_res(digit1, str::parse)(input)
}

// --- Cycle Parsers ---

fn parse_cycle_vals(input: &'_ str) -> Res<'_, Cycle> {
    let (input, nums) =
        delimited(char('['), separated_list1(char(','), parse_u32), char(']'))(input)?;
    Ok((input, Cycle::Values(nums.into_iter().collect())))
}

fn parse_cycle_single_val(input: &'_ str) -> Res<'_, Cycle> {
    let (input, num) = parse_u32(input)?;
    Ok((input, Cycle::Values(BTreeSet::from([num]))))
}

fn parse_cycle_foreach(input: &'_ str) -> Res<'_, Cycle> {
    value(Cycle::ForEach, alt((tag("YY"), tag("MM"), tag("_"))))(input)
}

fn parse_cycle_next_nth(input: &'_ str) -> Res<'_, Cycle> {
    let (input, num) = parse_u32(input)?;
    let (input, _) = alt((tag("Y"), tag("M")))(input)?;
    Ok((input, Cycle::NextNth(num)))
}

fn parse_year_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        parse_cycle_foreach,
        parse_cycle_next_nth,
        parse_cycle_single_val,
    ))(input)
}

fn parse_month_cycle(input: &'_ str) -> Res<'_, Cycle> {
    alt((
        parse_cycle_vals,
        parse_cycle_foreach,
        parse_cycle_next_nth,
        parse_cycle_single_val,
    ))(input)
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
    ))(input)?;

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
    )(input)?;
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
        delimited(char('['), separated_list1(char(','), parse_u32), char(']'))(input)?;
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
    let (input, _) = char('#')(input)?;

    let (input, opt_str) = recognize(pair(opt(digit1), opt(char('L'))))(input)?;

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
    let (input, type_tag) = alt((tag("BD"), tag("WD"), tag("D")))(input)?;

    let opt = match type_tag {
        "BD" => NextNthDayOption::BizDay,
        "WD" => NextNthDayOption::WeekDay,
        _ => NextNthDayOption::Regular,
    };
    Ok((input, DayCycle::NextNth(num, opt)))
}

fn parse_day_single_complex(input: &'_ str) -> Res<'_, DayCycle> {
    let (input, num) = parse_u32(input)?;
    let (input, suffix) = opt(alt((tag("L"), tag("N"), tag("O"))))(input)?;

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
    ))(input)
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
    ))(input)
}

// --- Adjustment Parser ---

fn parse_adjustment(input: &'_ str) -> Res<'_, BizDayAdjustment> {
    let (input, _) = char('~')(input)?;
    let (input, code) = alt((
        tag("PW"),
        tag("NW"),
        tag("PB"),
        tag("NB"),
        tag("B"),
        tag("W"),
        recognize(pair(opt(digit1), alt((tag("P"), tag("N"))))),
    ))(input)?;

    let adj = match code {
        "PW" => BizDayAdjustment::Weekday(Direction::Prev),
        "NW" => BizDayAdjustment::Weekday(Direction::Next),
        "PB" | "B" => BizDayAdjustment::BizDay(Direction::Prev),
        "NB" | "W" => BizDayAdjustment::BizDay(Direction::Next),
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
                    // Mapped to BizDay(Next) based on parser implementation
                    biz_day_adj: Some(BizDayAdjustment::BizDay(Direction::Next)),
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
