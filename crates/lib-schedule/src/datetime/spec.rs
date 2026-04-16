use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

use crate::prelude::*;

/// Matches the `T` separator between a date spec and a time spec.
/// The time spec always starts with: `HH:`, `<n>H:`, or a two-digit hour `<dd>:`.
/// This deliberately does NOT match `T` inside weekday names like `TUE` or `THU`.
static DATE_TIME_SEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"T(?:HH|[0-9]{1,2}H|[0-2]?[0-9]):").unwrap());

/// Combined date + time recurrence specification.
///
/// Format: `<date_spec>T<time_spec>`
///
/// ## Examples
/// - `"YY-1M-31L~WT11:00:00"` — last business day of each month at 11:00
/// - `"YY-MM-MONT1H:00:00"` — every Monday, every hour
/// - `"YY-MM-FRI#LT16:30:00"` — last Friday of each month at 16:30
#[derive(Debug, Clone)]
pub struct Spec {
    pub date_spec: String,
    pub time_spec: String,
}

impl FromStr for Spec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let sep = DATE_TIME_SEP
            .find(s)
            .ok_or(Error::ParseError("missing T separator in datetime spec"))?;
        let date_spec = s[..sep.start()].to_string();
        // +1 to skip the 'T' itself; the rest is the time spec
        let time_spec = s[sep.start() + 1..].to_string();
        Ok(Spec {
            date_spec,
            time_spec,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_at_time() {
        let spec = "YY-1M-31L~WT11:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-1M-31L~W");
        assert_eq!(spec.time_spec, "11:00:00");
    }

    #[test]
    fn test_every_hour() {
        let spec = "YY-MM-DDT1H:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-MM-DD");
        assert_eq!(spec.time_spec, "1H:00:00");
    }

    #[test]
    fn test_weekday_not_confused_with_separator() {
        // THU in day spec should not be confused with the T separator
        let spec = "YY-MM-THUT11:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-MM-THU");
        assert_eq!(spec.time_spec, "11:00:00");

        let spec = "YY-MM-TUET11:00:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-MM-TUE");
        assert_eq!(spec.time_spec, "11:00:00");
    }

    #[test]
    fn test_hh_time_spec() {
        let spec = "YY-MM-DDTHH:30M:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-MM-DD");
        assert_eq!(spec.time_spec, "HH:30M:00");
    }
}
