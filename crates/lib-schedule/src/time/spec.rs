use crate::prelude::*;
use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

/// ## SPEC_EXPR
/// Regular expression for matching time recurrence specifications.
/// It matches various combinations of hours, minutes, and seconds.
///
/// ### Supported Formats
///
/// - `HH:MM:SS`: Time format with hours in the range 00-23, minutes in the range 00-59, and seconds in the range 00-59.
/// - `<num>H:<num>M:<num>S`: Duration format with hours, minutes, and seconds specified as numbers followed by `H`, `M`, and `S` respectively.
///
/// ### Examples
///
/// - `12:34:56`: Matches time in hours, minutes, and seconds.
/// - `1H:1M:1S`: Matches duration in hours, minutes, and seconds.
pub const SPEC_EXPR: &str = r"([01][0-9]|2[0-3]|[0-9]H|1[0-9]H|2[0-3]H|HH):([0-5][0-9]|[0-5]?[0-9]M|MM):([0-5][0-9]|[0-5]?[0-9]S|SS)";
static SPEC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(SPEC_EXPR).unwrap());
const CYCLE_EXPR: &str = r"(?:HH|MM|SS)|(?:(?<num>\d+)(?<type>[HMS])?)";
static CYCLE_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(CYCLE_EXPR).unwrap());

/// ## Spec
/// Represents a time specification.
///
/// The `Spec` struct is used to define specification for time to support flexible scheduling options.
/// Best way to instantiate a `Spec` is to parse it from a string that matches the `SPEC_EXPR` regular expression.
/// ### Examples
///
/// ```rust
/// use lib_schedule::time::{Spec, Cycle};
/// use std::str::FromStr;
/// let spec = "1H:30:SS".parse::<Spec>().unwrap();
/// assert_eq!(spec.hours, Cycle::Every(1));
/// assert_eq!(spec.minutes, Cycle::At(30));
/// assert_eq!(spec.seconds, Cycle::NA);
/// ```
#[derive(Default, Debug, PartialEq, Clone)]
pub struct Spec {
    pub hours: Cycle,
    pub minutes: Cycle,
    pub seconds: Cycle,
}

#[derive(Default, Debug, PartialEq, Eq, Hash, Clone)]
pub enum Cycle {
    #[default]
    NA,
    At(u8),
    Every(u8),
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let caps = &SPEC_RE
            .captures(s)
            .ok_or(Error::ParseError("Invalid time spec"))?;
        let mut cycles = caps
            .iter()
            .skip(1)
            .flatten()
            .map(|m| Cycle::try_from(m.as_str()))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            hours: cycles.remove(0),
            minutes: cycles.remove(0),
            seconds: cycles.remove(0),
        })
    }
}

impl TryFrom<&str> for Cycle {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        let cycle = CYCLE_RE
            .captures(value)
            .ok_or(Error::ParseError("Invalid time spec"))?;

        let Some(num) = cycle.name("num") else {
            return Ok(Cycle::NA);
        };
        let num = num.as_str().parse::<u8>().unwrap();
        let cycle = if cycle.name("type").is_some() {
            Cycle::Every(num)
        } else {
            Cycle::At(num)
        };
        Ok(cycle)
    }
}

impl ToString for Spec {
    fn to_string(&self) -> String {
        let to_string = |cycle: &Cycle, cycle_type: char| match cycle {
            Cycle::NA => f!("{}{}", cycle_type, cycle_type),
            Cycle::At(num) => f!("{:02}", num),
            Cycle::Every(num) => f!("{:02}{}", num, cycle_type),
        };
        f!(
            "{}:{}:{}",
            to_string(&self.hours, 'H'),
            to_string(&self.minutes, 'M'),
            to_string(&self.seconds, 'S')
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
                hours: Cycle::NA,
                minutes: Cycle::Every(30),
                seconds: Cycle::At(5),
                ..Default::default()
            },
        );
        assert_eq!(time_spec.to_string(), "HH:30M:05");
    }
}
