use chrono::{DateTime, TimeZone};

mod date_spec;
mod error;
mod time_spec;
mod utils;

pub trait NextTime {
    fn next<Tz: TimeZone>(&self, from: &DateTime<Tz>) -> DateTime<Tz>;
}
