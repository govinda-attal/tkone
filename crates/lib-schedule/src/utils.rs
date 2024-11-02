use chrono::{DateTime, Duration, NaiveDateTime, TimeZone};

use crate::prelude::*;

impl <Tz: TimeZone>From<W<(Tz, NaiveDateTime)>> for DateTime<Tz> {
    fn from(W((tz, dtm)): W<(Tz, NaiveDateTime)>) -> Self {
        match tz.from_local_datetime(&dtm) {
            chrono::LocalResult::None => {
                // the positive timezone transition (spring forward)
                tz
                .from_local_datetime(&(dtm.clone() + Duration::hours(1)))
                .latest()
                .unwrap()
            }
            chrono::LocalResult::Single(dtm) => dtm,
            chrono::LocalResult::Ambiguous(_, _) => {
                // the negative timezone transition (fallback)
                tz.from_local_datetime(&dtm).earliest().unwrap()
            }
        }
    }
}
