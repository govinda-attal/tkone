//! Easter Sunday, the anchor of the `EAS` and `OEAS` day rules.

use chrono::{Days, NaiveDate};

/// Which Easter a day rule counts from.
///
/// | Variant | Spec token | Computus |
/// |---------|------------|----------|
/// | `Western` | `EAS` | Gregorian (Catholic and Protestant churches) |
/// | `Orthodox` | `OEAS` | Julian, given as a Gregorian date (Eastern Orthodox churches) |
///
/// ```rust
/// use tkone_schedule::date::Easter;
/// use chrono::NaiveDate;
///
/// assert_eq!(Easter::Western.sunday(2025), NaiveDate::from_ymd_opt(2025, 4, 20));
/// assert_eq!(Easter::Western.sunday(2026), NaiveDate::from_ymd_opt(2026, 4, 5));
/// assert_eq!(Easter::Orthodox.sunday(2026), NaiveDate::from_ymd_opt(2026, 4, 12));
/// ```
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Easter {
    /// Western Easter (`EAS`).
    Western,
    /// Orthodox Easter (`OEAS`).
    Orthodox,
}

impl Easter {
    /// Easter Sunday of `year`, as a Gregorian date. `None` before year 1 or
    /// beyond the dates chrono can represent.
    pub fn sunday(self, year: i32) -> Option<NaiveDate> {
        if year < 1 {
            return None;
        }
        match self {
            Self::Western => western(year),
            Self::Orthodox => orthodox(year),
        }
    }
}

/// The anonymous Gregorian algorithm (Meeus, *Astronomical Algorithms*).
fn western(year: i32) -> Option<NaiveDate> {
    let y = i64::from(year);
    let (a, b, c) = (y % 19, y / 100, y % 100);
    let (d, e) = (b / 4, b % 4);
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let (i, k) = (c / 4, c % 4);
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let n = h + l - 7 * m + 114;
    NaiveDate::from_ymd_opt(year, (n / 31) as u32, (n % 31 + 1) as u32)
}

/// Meeus's Julian algorithm, then moved to the Gregorian calendar. The
/// calendars differ by `y/100 - y/400 - 2` days from March of a year to
/// February of the next; Easter is always inside that span.
fn orthodox(year: i32) -> Option<NaiveDate> {
    let y = i64::from(year);
    let (a, b, c) = (y % 4, y % 7, y % 19);
    let d = (19 * c + 15) % 30;
    let e = (2 * a + 4 * b - d + 34) % 7;
    let n = d + e + 114;
    let julian = NaiveDate::from_ymd_opt(year, (n / 31) as u32, (n % 31 + 1) as u32)?;
    julian.checked_add_days(Days::new(u64::try_from(y / 100 - y / 400 - 2).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn test_known_dates() {
        for (year, western, orthodox) in [
            (1961, ymd(1961, 4, 2), ymd(1961, 4, 9)),
            (2000, ymd(2000, 4, 23), ymd(2000, 4, 30)),
            (2008, ymd(2008, 3, 23), ymd(2008, 4, 27)),
            (2011, ymd(2011, 4, 24), ymd(2011, 4, 24)),
            (2024, ymd(2024, 3, 31), ymd(2024, 5, 5)),
            (2025, ymd(2025, 4, 20), ymd(2025, 4, 20)),
            (2038, ymd(2038, 4, 25), ymd(2038, 4, 25)),
        ] {
            assert_eq!(Easter::Western.sunday(year), Some(western), "Western {year}");
            assert_eq!(Easter::Orthodox.sunday(year), Some(orthodox), "Orthodox {year}");
        }
        // The earliest possible date, next reached in 2285.
        assert_eq!(Easter::Western.sunday(2285), Some(ymd(2285, 3, 22)));
        assert_eq!(Easter::Western.sunday(0), None);
    }

    #[test]
    fn test_always_a_sunday_in_range() {
        use chrono::{Datelike, Weekday};
        for year in 1583..=4099 {
            let w = Easter::Western.sunday(year).unwrap();
            assert_eq!(w.weekday(), Weekday::Sun);
            assert!(ymd(year, 3, 22) <= w && w <= ymd(year, 4, 25), "{w}");
            assert_eq!(Easter::Orthodox.sunday(year).unwrap().weekday(), Weekday::Sun);
        }
    }
}
