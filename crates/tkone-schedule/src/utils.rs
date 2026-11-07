use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Weekday};

use crate::{prelude::*, DstPolicy, Occurrence};

/// `"a"`, `"a and b"`, `"a, b and c"` — for spec descriptions.
/// Just before `t`, so results "after" it include `t` itself.
pub(crate) fn just_before<Tz: TimeZone>(t: &DateTime<Tz>) -> DateTime<Tz> {
    t.clone() - Duration::nanoseconds(1)
}

pub(crate) fn join(parts: &[String]) -> String {
    match parts {
        [] => String::new(),
        [one] => one.clone(),
        [init @ .., last] => format!("{} and {last}", init.join(", ")),
    }
}

/// Convert a `NaiveDateTime` to `DateTime<Tz>` according to `policy`.
///
/// - `Adjust` (default): a time inside a gap (spring-forward) moves later by
///   the length of the gap; a time inside an overlap (fall-back) takes the
///   earlier offset.
/// - `Strict`: returns [`Error::AmbiguousLocalTime`] for any non-unique mapping.
pub(crate) fn resolve_local<Tz: TimeZone>(
    tz: &Tz,
    dtm: NaiveDateTime,
    policy: DstPolicy,
) -> Result<DateTime<Tz>> {
    match tz.from_local_datetime(&dtm) {
        LocalResult::Single(dt) => Ok(dt),
        LocalResult::None => match policy {
            DstPolicy::Adjust => Ok(gap_shift(tz, dtm)?.1),
            DstPolicy::Strict => Err(Error::AmbiguousLocalTime(format!(
                "{dtm} does not exist in timezone (DST spring-forward gap)"
            ))),
        },
        LocalResult::Ambiguous(earliest, _) => match policy {
            DstPolicy::Adjust => Ok(earliest),
            DstPolicy::Strict => Err(Error::AmbiguousLocalTime(format!(
                "{dtm} is ambiguous in timezone (DST fall-back overlap)"
            ))),
        },
    }
}

/// For a local time inside a gap, return `(label, shifted)`:
///
/// - `label` shows the wall-clock time as scheduled, carrying the offset in
///   force just before the gap (e.g. `2011-12-30 00:00 -10:00` in Apia).
/// - `shifted` is the same instant expressed in the timezone's real offset,
///   i.e. the time moved later by the length of the gap (`02:30` across a
///   1-hour spring-forward becomes `03:30`).
///
/// Both denote one instant, so they compare equal. Works for gaps of any
/// length, including whole skipped days.
fn gap_shift<Tz: TimeZone>(tz: &Tz, dtm: NaiveDateTime) -> Result<(DateTime<Tz>, DateTime<Tz>)> {
    // Walk back to the last local time before the gap; 15-minute steps over
    // 48 hours cover every gap in the tz database.
    let before = (1..=48 * 4)
        .find_map(|k| tz.from_local_datetime(&(dtm - Duration::minutes(15 * k))).latest())
        .ok_or_else(|| {
            Error::AmbiguousLocalTime(format!(
                "{dtm} does not exist in timezone and no offset precedes the gap"
            ))
        })?;
    let offset = before.offset().clone();
    let utc = dtm - Duration::seconds(i64::from(offset.fix().local_minus_utc()));
    Ok((DateTime::from_naive_utc_and_offset(utc, offset), tz.from_utc_datetime(&utc)))
}

/// Convert a `Occurrence<NaiveDateTime>` to `Occurrence<DateTime<Tz>>`,
/// applying `policy` to each embedded naive datetime.
///
/// Under [`DstPolicy::Adjust`], an actual time inside a DST gap keeps its
/// wall-clock label while the observed time moves past the gap, so an
/// otherwise exact occurrence is reported as `AdjustedLater(label, shifted)`.
pub(crate) fn next_result_to_tz<Tz: TimeZone>(
    tz: &Tz,
    next: Occurrence<NaiveDateTime>,
    policy: DstPolicy,
) -> Result<Occurrence<DateTime<Tz>>> {
    let (actual, observed) = next.as_tuple();
    let observed_at = resolve_local(tz, *observed, policy)?;
    let actual_at = match (policy, tz.from_local_datetime(actual)) {
        (DstPolicy::Adjust, LocalResult::None) => gap_shift(tz, *actual)?.0,
        _ => resolve_local(tz, *actual, policy)?,
    };
    Ok(match next {
        Occurrence::Exact(_) if actual_at.naive_local() != observed_at.naive_local() => {
            Occurrence::AdjustedLater(actual_at, observed_at)
        }
        Occurrence::Exact(_) => Occurrence::Exact(observed_at),
        Occurrence::AdjustedLater(..) => Occurrence::AdjustedLater(actual_at, observed_at),
        Occurrence::AdjustedEarlier(..) => Occurrence::AdjustedEarlier(actual_at, observed_at),
    })
}

pub trait DateLikeUtils: Datelike {
    fn to_last_day_of_month(&self) -> Self;
}

impl DateLikeUtils for NaiveDate {
    fn to_last_day_of_month(&self) -> Self {
        NaiveDate::from_ymd_opt(self.year(), self.month() + 1, 1)
            .unwrap_or(NaiveDate::from_ymd_opt(self.year() + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap()
    }
}

impl DateLikeUtils for NaiveDateTime {
    fn to_last_day_of_month(&self) -> Self {
        NaiveDateTime::new(self.date().to_last_day_of_month(), self.time())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeekdayStartingMonday(pub Weekday);

impl Ord for WeekdayStartingMonday {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare based on 0 (Mon) to 6 (Sun)
        self.0
            .num_days_from_monday()
            .cmp(&other.0.num_days_from_monday())
    }
}

impl PartialOrd for WeekdayStartingMonday {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn local(y: i32, m: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(y, m, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
    }

    #[test]
    fn test_gap_moves_later_by_gap_length() {
        let cases = [
            // 1h spring-forward
            (chrono_tz::America::New_York, local(2025, 3, 9, 2, 30), "2025-03-09T03:30:00-04:00"),
            // 30-minute gap
            (chrono_tz::Australia::Lord_Howe, local(2025, 10, 5, 2, 15), "2025-10-05T02:45:00+11:00"),
            // 2h gap
            (chrono_tz::Antarctica::Troll, local(2025, 3, 30, 1, 30), "2025-03-30T03:30:00+02:00"),
            // whole skipped day
            (chrono_tz::Pacific::Apia, local(2011, 12, 30, 0, 0), "2011-12-31T00:00:00+14:00"),
        ];
        for (tz, dtm, expected) in cases {
            let resolved = resolve_local(&tz, dtm, DstPolicy::Adjust).unwrap();
            assert_eq!(resolved.to_rfc3339(), expected, "{tz} {dtm}");
            assert!(resolve_local(&tz, dtm, DstPolicy::Strict).is_err(), "{tz} {dtm}");
        }
    }

    #[test]
    fn test_overlap_takes_earliest_offset() {
        let tz = chrono_tz::America::New_York;
        let resolved = resolve_local(&tz, local(2025, 11, 2, 1, 30), DstPolicy::Adjust).unwrap();
        assert_eq!(resolved.to_rfc3339(), "2025-11-02T01:30:00-04:00");
    }
}
