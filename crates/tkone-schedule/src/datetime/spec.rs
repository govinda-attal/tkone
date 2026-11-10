use regex::Regex;
use std::str::FromStr;
use std::sync::LazyLock;

use crate::prelude::*;

/// Matches the `T` separator between a date spec and a time spec.
/// The time spec always starts with: `HH:`, `_:`, `<n>H:`, a two-digit hour `<dd>:`, an hour
/// range `<dd>-<dd>:`, or an hour list `[…]:`.
/// This deliberately does NOT match `T` inside weekday names like `TUE` or `THU`.
static DATE_TIME_SEP: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"T(?:HH|_|\[[0-9,\-]+\]|[0-9]{1,2}H|[0-2]?[0-9](?:-[0-2]?[0-9])?):").unwrap());

/// Combined date + time recurrence specification.
///
/// Format: `<date_spec>T<time_spec>`
///
/// ## Examples
/// - `"YY-1M-31L~WT11:00:00"` — last day of each month adjusted to nearest weekday at 11:00
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
        let sep =
            DATE_TIME_SEP.find(s).ok_or_else(|| Error::InvalidDateTimeSpec(format!("missing T separator: {s}")))?;
        let date_spec = s[..sep.start()].to_string();
        // +1 to skip the 'T' itself; the rest is the time spec
        let time_spec = s[sep.start() + 1..].to_string();
        Ok(Spec { date_spec, time_spec })
    }
}

impl Spec {
    /// Describes the spec in plain English, to check it reads as intended
    /// before it is scheduled: the date part, then the times on each date.
    ///
    /// "The start" is the start (or `new_after` cursor) for the date part and,
    /// with the default [`TimeAnchor::Start`](crate::TimeAnchor::Start), for
    /// the time part too. With [`TimeAnchor::Midnight`](crate::TimeAnchor::Midnight)
    /// the time part counts from midnight of each date instead.
    ///
    /// # Errors
    ///
    /// Returns an error if either part does not parse.
    ///
    /// ```rust
    /// use tkone_schedule::datetime::Spec;
    /// let spec: Spec = "YY-MM-MONT1H:MM:00".parse().unwrap();
    /// assert_eq!(
    ///     spec.describe().unwrap(),
    ///     "every Monday; every hour, at the start's minute and second 00"
    /// );
    /// ```
    pub fn describe(&self) -> Result<String> {
        let date = self.date_spec.parse::<crate::date::Spec>()?;
        let time = self.time_spec.parse::<crate::time::Spec>()?;
        Ok(format!("{}; {}", date.describe(), time.describe()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_describe() {
        for (spec, expected) in [
            ("YY-MM-FRI#LT16:30:00", "on the last Friday of every month; at 16:30:00"),
            ("YY-1M-31L~WT11:00:00", "on the 31st of every month (or the last day, in shorter months), moved to the nearest weekday when it falls on a weekend; at 11:00:00"),
            ("YY-MM-DDTHH:30M:00", "every day; every 30 minutes, at second 00, counted from the start"),
            ("YY-1M-31T_:_:_", "on the 31st of every month (skipped in shorter months); at the start's time of day"),
        ] {
            assert_eq!(spec.parse::<Spec>().unwrap().describe().unwrap(), expected, "{spec}");
        }
        assert!("YY-13-01T09:00:00".parse::<Spec>().unwrap().describe().is_err());
    }

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
    fn test_time_lists_and_ranges() {
        for (s, date, time) in [
            ("YY-MM-DDT[09,12,17]:00:00", "YY-MM-DD", "[09,12,17]:00:00"),
            ("YY-MM-[MON,THU]T09-16:15M:00", "YY-MM-[MON,THU]", "09-16:15M:00"),
            ("YY-MM-BD#3T[09-11,14]:00:00", "YY-MM-BD#3", "[09-11,14]:00:00"),
            ("YY-MM-SATT9-16:00:00", "YY-MM-SAT", "9-16:00:00"),
        ] {
            let spec = s.parse::<Spec>().unwrap();
            assert_eq!((spec.date_spec.as_str(), spec.time_spec.as_str()), (date, time), "{s}");
        }
    }

    #[test]
    fn test_hh_time_spec() {
        let spec = "YY-MM-DDTHH:30M:00".parse::<Spec>().unwrap();
        assert_eq!(spec.date_spec, "YY-MM-DD");
        assert_eq!(spec.time_spec, "HH:30M:00");
    }
}
