use crate::prelude::*;
use std::fmt;
use std::str::FromStr;

use nom::{
    Parser,
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
    combinator::{all_consuming, map_res, value, verify},
    error::Error as NomError,
    sequence::preceded,
    IResult,
};

/// ## Spec
/// Represents a time specification.
///
/// The `Spec` struct is used to define specification for time to support flexible scheduling options.
/// Best way to instantiate a `Spec` is to parse it from a string using the format `HH:MM:SS`,
/// where each component is one of: `HH`/`MM`/`SS` (ForEach), `_` (AsIs), `nH`/`nM`/`nS` (Every),
/// or a 2-digit number (At).
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
/// | `Every(n)` | `1H`, `30M`, `15S` | Add a duration of *n* units on each tick |
#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    #[default]
    AsIs,
    ForEach,
    At(u8),
    Every(u8),
}

// ---------------------------------------------------------------------------
// Parser (nom)
// ---------------------------------------------------------------------------

type Res<'a, T> = IResult<&'a str, T, NomError<&'a str>>;

fn parse_u8(input: &str) -> Res<'_, u8> {
    map_res(digit1, str::parse).parse(input)
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
        value(Cycle::ForEach, tag("MM")),
        value(Cycle::AsIs, tag("_")),
        parse_minutes_every,
        parse_minutes_at,
    ))
    .parse(input)
}

fn parse_seconds_every(input: &str) -> Res<'_,  Cycle> {
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
        value(Cycle::ForEach, tag("SS")),
        value(Cycle::AsIs, tag("_")),
        parse_seconds_every,
        parse_seconds_at,
    ))
    .parse(input)
}

fn parse_spec(input: &str) -> Result<Spec> {
    let full_parser = (
        parse_hours_cycle,
        preceded(char(':'), parse_minutes_cycle),
        preceded(char(':'), parse_seconds_cycle),
    );
    match all_consuming(full_parser).parse(input) {
        Ok((_, (hours, minutes, seconds))) => Ok(Spec {
            hours,
            minutes,
            seconds,
        }),
        Err(_) => Err(Error::InvalidTimeSpec(format!("failed to parse: {input}"))),
    }
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        parse_spec(s)
    }
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

impl fmt::Display for Spec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fmt_cycle = |cycle: &Cycle, letter: char| -> String {
            match cycle {
                Cycle::AsIs => "_".to_string(),
                Cycle::ForEach => format!("{}{}", letter, letter),
                Cycle::At(n) => format!("{:02}", n),
                Cycle::Every(n) => format!("{}{}", n, letter),
            }
        };
        write!(
            f,
            "{}:{}:{}",
            fmt_cycle(&self.hours, 'H'),
            fmt_cycle(&self.minutes, 'M'),
            fmt_cycle(&self.seconds, 'S'),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_spec_from_str() {
        let time_spec = "HH:30M:05".parse::<Spec>().unwrap();
        assert_eq!(
            &time_spec,
            &Spec {
                hours: Cycle::ForEach,
                minutes: Cycle::Every(30),
                seconds: Cycle::At(5),
                ..Default::default()
            },
        );
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
    fn test_time_spec_roundtrip() {
        for s in &[
            "HH:MM:SS",
            "1H:00:00",
            "HH:30M:00",
            "_:_:_",
            "_:00:00",
            "09:30:00",
            "HH:MM:30S",
            "2H:15:00",
        ] {
            let parsed = s.parse::<Spec>().unwrap();
            assert_eq!(&parsed.to_string(), s, "roundtrip failed for {}", s);
        }
    }
}
