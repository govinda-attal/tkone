mod biz_day;
mod date;
mod error;
mod prelude;
mod time;
mod utils;

use biz_day::BizDayProcessor;
use chrono::DateTime;
use chrono_tz::Tz;
use fallible_iterator::FallibleIterator;
use prelude::*;

pub trait NextTime: FallibleIterator<Item = DateTime<Tz>, Error = Error> {}

pub trait NextDate<BDP>: FallibleIterator<Item = DateTime<Tz>, Error = Error> {
    type BDP: BizDayProcessor;
}
