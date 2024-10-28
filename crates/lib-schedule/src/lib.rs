mod biz_day;
mod date;
mod time;
mod error;
mod prelude;
mod time_spec;
mod utils;

use biz_day::BizDayProcessor;
use chrono::{DateTime, TimeZone};
use fallible_iterator::FallibleIterator;
use prelude::*;

pub trait NextTime<Tz: TimeZone>: FallibleIterator<Item = DateTime<Tz>, Error = Error>{}

pub trait NextDate<BDP, Tz: TimeZone>:
    FallibleIterator<Item = DateTime<Tz>, Error = Error>
{
    type BDP: BizDayProcessor;
}
