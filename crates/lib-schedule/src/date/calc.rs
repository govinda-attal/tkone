use crate::prelude::*;
use crate::biz_day::WeekendSkipper;
use super::spec::Spec;
use chrono::{DateTime, TimeZone};
use fallible_iterator::FallibleIterator;

struct Calculator<Tz: TimeZone> {
    spec: Spec,
    dtm: DateTime<Tz>,
    bd_processor: WeekendSkipper, // Using the example BizDateProcessor
}

impl<Tz: TimeZone> Calculator<Tz> {
    fn new(dtm: DateTime<Tz>, spec: &str) -> Result<Self> {
        let spec = spec.parse()?;
        Ok(Self {
            spec,
            dtm,
            bd_processor: WeekendSkipper {},
        })
    }
}

impl<Tz: TimeZone> FallibleIterator for Calculator<Tz> {
    type Item = DateTime<Tz>;
    type Error = Error;

    fn next(&mut self) -> Result<Option<Self::Item>> {
        Ok(Some(self.dtm.clone()))
    }
}
