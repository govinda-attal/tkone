use crate::prelude::*;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
    combinator::{all_consuming, map_res, opt, value, verify},
    error::Error as NomError,
    multi::separated_list1,
    sequence::{delimited, preceded},
    IResult, Parser,
};

/// ## Spec
/// Represents a time specification.
///
/// The `Spec` struct is used to define specification for time to support flexible scheduling options.
/// Best way to instantiate a `Spec` is to parse it from a string using the format `HH:MM:SS`,
/// where each component is one of: `HH`/`MM`/`SS` (ForEach), `_` (AsIs), `nH`/`nM`/`nS` (Every),
/// a 2-digit number (At), or a list / range of numbers (Values: `[09,12,17]`, `09-16`,
/// `[09-11,14]`).
///
/// ### Examples
///
/// ```rust
/// use tkone_schedule::time::{Spec, Cycle};
/// use std::str::FromStr;
/// let spec = "1H:30:SS".parse::<Spec>().unwrap();
/// assert_eq!(spec.hours, Cycle::Every(1));
/// assert_eq!(spec.minutes, Cycle::At(30));
/// assert_eq!(spec.seconds, Cycle::ForEach);
/// ```
#[derive(Default, Debug, PartialEq, Clone)]
pub struct Spec {
    pub hours: Cycle,
    pub minutes: Cycle,
    pub seconds: Cycle,
}

/// ## Cycle
/// Describes how a single time component (hours, minutes, or seconds) advances on each tick.
///
/// | Variant | Syntax | Meaning |
/// |---------|--------|---------|
/// | `AsIs`     | `_`          | Keep current value unchanged (no-op) |
/// | `ForEach`  | `HH`/`MM`/`SS` | Advance each occurrence; acts as `Every(1)` for the finest component when no `Every` is present |
/// | `At(n)`    | `09`, `30`, `45` | Pin the field to exact value *n* |
/// | `Values(set)` | `[09,12,17]`, `09-16`, `[09-11,14]` | Pin the field to any of these values |
/// | `Every(n)` | `1H`, `30M`, `15S` | Add a duration of *n* units on each tick |
///
/// A list or range with a single value parses as `At`.
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    #[default]
    AsIs,
    ForEach,
    At(u8),
    /// Two or more allowed values, from a list (`[00,30]`) or an inclusive
    /// range (`09-16`).
    Values(BTreeSet<u8>),
    Every(u8),
}

impl Cycle {
    /// The values a fixed field allows (`At` or `Values`), ascending.
    pub(crate) fn pinned(&self) -> Option<Vec<u8>> {
        match self {
            Cycle::At(v) => Some(vec![*v]),
            Cycle::Values(vs) => Some(vs.iter().copied().collect()),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Parser (nom)
// ---------------------------------------------------------------------------

type Res<'a, T> = IResult<&'a str, T, NomError<&'a str>>;

fn parse_u8(input: &str) -> Res<'_, u8> {
    map_res(digit1, str::parse).parse(input)
}

/// `a` or `a-b` (inclusive).
fn parse_span(input: &str) -> Res<'_, (u8, u8)> {
    let (input, lo) = parse_u8(input)?;
    let (input, hi) = opt(preceded(char('-'), parse_u8)).parse(input)?;
    Ok((input, (lo, hi.unwrap_or(lo))))
}

fn values(spans: Vec<(u8, u8)>) -> core::result::Result<Cycle, &'static str> {
    let mut set = BTreeSet::new();
    for (lo, hi) in spans {
        if lo > hi {
            return Err("range start is after its end");
        }
        set.extend(lo..=hi);
    }
    Ok(match set.len() {
        1 => Cycle::At(*set.first().unwrap()),
        _ => Cycle::Values(set),
    })
}

/// A list `[09,12,17]` / `[09-11,14]`, or a range `09-16`.
fn parse_values(input: &str) -> Res<'_, Cycle> {
    let list = delimited(char('['), separated_list1(char(','), parse_span), char(']'));
    let range = (parse_u8, preceded(char('-'), parse_u8)).map(|span| vec![span]);
    map_res(alt((list, range)), values).parse(input)
}

fn parse_hours_every(input: &str) -> Res<'_, Cycle> {
    let (input, n) = verify(parse_u8, |&n| n > 0).parse(input)?;
    let (input, _) = char('H').parse(input)?;
    Ok((input, Cycle::Every(n)))
}

fn parse_hours_at(input: &str) -> Res<'_, Cycle> {
    let (input, n) = parse_u8(input)?;
    Ok((input, Cycle::At(n)))
}

fn parse_hours_cycle(input: &str) -> Res<'_, Cycle> {
    alt((
        parse_values,
        value(Cycle::ForEach, tag("HH")),
        value(Cycle::AsIs, tag("_")),
        parse_hours_every,
        parse_hours_at,
    ))
    .parse(input)
}

fn parse_minutes_every(input: &str) -> Res<'_, Cycle> {
    let (input, n) = verify(parse_u8, |&n| n > 0).parse(input)?;
    let (input, _) = char('M').parse(input)?;
    Ok((input, Cycle::Every(n)))
}

fn parse_minutes_at(input: &str) -> Res<'_, Cycle> {
    let (input, n) = parse_u8(input)?;
    Ok((input, Cycle::At(n)))
}

fn parse_minutes_cycle(input: &str) -> Res<'_, Cycle> {
    alt((
        parse_values,
        value(Cycle::ForEach, tag("MM")),
        value(Cycle::AsIs, tag("_")),
        parse_minutes_every,
        parse_minutes_at,
    ))
    .parse(input)
}

fn parse_seconds_every(input: &str) -> Res<'_, Cycle> {
    let (input, n) = verify(parse_u8, |&n| n > 0).parse(input)?;
    let (input, _) = char('S').parse(input)?;
    Ok((input, Cycle::Every(n)))
}

fn parse_seconds_at(input: &str) -> Res<'_, Cycle> {
    let (input, n) = parse_u8(input)?;
    Ok((input, Cycle::At(n)))
}

fn parse_seconds_cycle(input: &str) -> Res<'_, Cycle> {
    alt((
        parse_values,
        value(Cycle::ForEach, tag("SS")),
        value(Cycle::AsIs, tag("_")),
        parse_seconds_every,
        parse_seconds_at,
    ))
    .parse(input)
}

fn parse_spec(input: &str) -> Result<Spec> {
    let full_parser =
        (parse_hours_cycle, preceded(char(':'), parse_minutes_cycle), preceded(char(':'), parse_seconds_cycle));
    let spec = match all_consuming(full_parser).parse(input) {
        Ok((_, (hours, minutes, seconds))) => Spec { hours, minutes, seconds },
        Err(_) => return Err(Error::InvalidTimeSpec(format!("failed to parse: {input}"))),
    };
    for (cycle, max, unit) in [(&spec.hours, 23, "hour"), (&spec.minutes, 59, "minute"), (&spec.seconds, 59, "second")]
    {
        if let Some(n) = cycle.pinned().and_then(|vs| vs.last().copied()) {
            if n > max {
                return Err(Error::InvalidTimeSpec(format!("{input}: {unit} {n} is out of range 0-{max}")));
            }
        }
    }
    Ok(spec)
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        parse_spec(s)
    }
}

impl Spec {
    /// Describes the spec in plain English, to check it reads as intended
    /// before it is scheduled.
    ///
    /// "The start" is the time the iterator is anchored to: the
    /// `new_with_start` start or the `new_after` cursor (in a datetime spec,
    /// see [`TimeAnchor`](crate::TimeAnchor)). A spec without a step names a
    /// single time of day.
    ///
    /// ```rust
    /// use tkone_schedule::time::Spec;
    /// let describe = |s: &str| s.parse::<Spec>().unwrap().describe();
    /// assert_eq!(describe("HH:MM:00"), "every minute, at second 00");
    /// assert_eq!(describe("1H:MM:00"), "every hour, at the start's minute and second 00");
    /// assert_eq!(describe("HH:30M:00"), "every 30 minutes, at second 00, counted from the start");
    /// assert_eq!(describe("09:30:00"), "at 09:30:00");
    /// ```
    pub fn describe(&self) -> String {
        super::describe::describe(self)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

/// Runs of consecutive values become ranges: `09-16`, `[00,30]`, `[09-11,14]`.
fn fmt_values(vs: &BTreeSet<u8>) -> String {
    let mut runs: Vec<(u8, u8)> = Vec::new();
    for &v in vs {
        match runs.last_mut() {
            Some((_, hi)) if *hi + 1 == v => *hi = v,
            _ => runs.push((v, v)),
        }
    }
    let parts: Vec<String> =
        runs.iter().map(|&(lo, hi)| if lo == hi { format!("{lo:02}") } else { format!("{lo:02}-{hi:02}") }).collect();
    match parts.as_slice() {
        [range] if runs[0].0 != runs[0].1 => range.clone(),
        _ => format!("[{}]", parts.join(",")),
    }
}

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt_cycle = |cycle: &Cycle, letter: char| -> String {
            match cycle {
                Cycle::AsIs => "_".to_string(),
                Cycle::ForEach => format!("{}{}", letter, letter),
                Cycle::At(n) => format!("{:02}", n),
                Cycle::Values(vs) => fmt_values(vs),
                Cycle::Every(n) => format!("{}{}", n, letter),
            }
        };
        write!(f, "{}:{}:{}", fmt_cycle(&self.hours, 'H'), fmt_cycle(&self.minutes, 'M'), fmt_cycle(&self.seconds, 'S'),)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_spec_from_str() {
        let time_spec = "HH:30M:05".parse::<Spec>().unwrap();
        assert_eq!(&time_spec, &Spec { hours: Cycle::ForEach, minutes: Cycle::Every(30), seconds: Cycle::At(5) },);
        assert_eq!(time_spec.to_string(), "HH:30M:05");
    }

    #[test]
    fn test_time_spec_asis_parsing() {
        let spec = "_:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::AsIs);
        assert_eq!(spec.minutes, Cycle::At(0));
        assert_eq!(spec.seconds, Cycle::At(0));
        assert_eq!(spec.to_string(), "_:00:00");
    }

    #[test]
    fn test_time_spec_all_foreach() {
        let spec = "HH:MM:SS".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::ForEach);
        assert_eq!(spec.minutes, Cycle::ForEach);
        assert_eq!(spec.seconds, Cycle::ForEach);
        assert_eq!(spec.to_string(), "HH:MM:SS");
    }

    #[test]
    fn test_time_spec_mixed() {
        let spec = "1H:30:SS".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::Every(1));
        assert_eq!(spec.minutes, Cycle::At(30));
        assert_eq!(spec.seconds, Cycle::ForEach);
        assert_eq!(spec.to_string(), "1H:30:SS");
    }

    #[test]
    fn test_every_zero_is_parse_error() {
        assert!("0H:00:00".parse::<Spec>().is_err(), "0H should be a parse error");
        assert!("HH:0M:00".parse::<Spec>().is_err(), "0M should be a parse error");
        assert!("HH:MM:0S".parse::<Spec>().is_err(), "0S should be a parse error");
    }

    #[test]
    fn test_out_of_range_at_is_parse_error() {
        for s in ["24:00:00", "25:00:00", "HH:60:00", "HH:MM:60", "99:99:99"] {
            assert!(s.parse::<Spec>().is_err(), "{s} should be a parse error");
        }
        for s in ["23:59:59", "00:00:00", "25H:00:00", "HH:90M:00"] {
            assert!(s.parse::<Spec>().is_ok(), "{s} should parse");
        }
    }

    #[test]
    fn test_lists_and_ranges() {
        let spec = "[09,12,17]:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::Values(BTreeSet::from([9, 12, 17])));
        let spec = "09-16:15M:00".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::Values((9..=16).collect()));
        assert_eq!(spec.minutes, Cycle::Every(15));
        let spec = "[09-11,14]:[00,30]:00".parse::<Spec>().unwrap();
        assert_eq!(spec.hours, Cycle::Values(BTreeSet::from([9, 10, 11, 14])));
        assert_eq!(spec.minutes, Cycle::Values(BTreeSet::from([0, 30])));
        // a single value is `At`
        assert_eq!("[09]:00:00".parse::<Spec>().unwrap().hours, Cycle::At(9));
        assert_eq!("09-09:00:00".parse::<Spec>().unwrap().hours, Cycle::At(9));
        for s in
            ["16-09:00:00", "[09,24]:00:00", "HH:[00,60]:00", "HH:MM:55-60", "[]:00:00", "[09,]:00:00", "09-:00:00"]
        {
            assert!(s.parse::<Spec>().is_err(), "{s} should be a parse error");
        }
        for (s, shown) in [
            ("[09,12,17]:00:00", "[09,12,17]:00:00"),
            ("09-16:15M:00", "09-16:15M:00"),
            ("[09-11,14]:[00,30]:00", "[09-11,14]:[00,30]:00"),
            ("[09,10]:00:00", "09-10:00:00"),
            ("[14,09-11]:00:00", "[09-11,14]:00:00"),
        ] {
            assert_eq!(s.parse::<Spec>().unwrap().to_string(), shown, "display of {s}");
        }
    }

    #[test]
    fn test_time_spec_roundtrip() {
        for s in &["HH:MM:SS", "1H:00:00", "HH:30M:00", "_:_:_", "_:00:00", "09:30:00", "HH:MM:30S", "2H:15:00"] {
            let parsed = s.parse::<Spec>().unwrap();
            assert_eq!(&parsed.to_string(), s, "roundtrip failed for {}", s);
        }
    }
}
