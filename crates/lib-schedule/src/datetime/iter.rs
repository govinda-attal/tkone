
// use core::marker::PhantomData;
// use std::str::FromStr;
// use crate::biz_day::BizDayProcessor;
// use crate::prelude::*;
// use super::spec::Spec;

// use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Offset, TimeZone, Timelike, Utc};
// use fallible_iterator::FallibleIterator;

// use crate::date::{SpecIterator as DateSpecIterator, SpecIteratorBuilder as DateSpecIteratorBuilder};
// use crate::time::SpecIterator as TimeSpecIterator;


// #[derive(Debug)]
// pub struct SpecIterator<Tz: TimeZone> {
//     start: DateTime<Tz>,
//     end: Option<DateTime<Tz>>,
//     remaining: Option<u32>,
//     dtm: DateTime<Tz>,
//     spec: Spec,
//     bd_processor: Box<dyn BizDayProcessor>,
// }


// struct StartDateTime<Tz: TimeZone>(DateTime<Tz>);
// struct NoStart;
// struct EndDateTime<Tz: TimeZone>(DateTime<Tz>);
// struct EndSpec(String);
// struct NoEnd;
// struct Sealed;
// struct NotSealed;

// pub struct SpecIteratorBuilder<Tz: TimeZone, START, END, S> {
//     start: START,
//     spec: String,
//     bd_processor: Box<dyn BizDayProcessor>,
//     end: END,
//     timezone: Tz,
//     marker_sealed: PhantomData<S>,
// }

// impl<Tz: TimeZone, END> SpecIteratorBuilder<Tz, StartDateTime<Tz>, END, NotSealed> {
//     pub fn with_end_spec(
//         self,
//         end_spec: impl Into<String>,
//     ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
//         SpecIteratorBuilder {
//             start: self.start,
//             spec: self.spec,
//             bd_processor: self.bd_processor,
//             end: EndSpec(end_spec.into()),
//             marker_sealed: PhantomData,
//             timezone: self.timezone,
//         }
//     }

//     pub fn with_end(
//         self,
//         end: DateTime<Tz>,
//     ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
//         SpecIteratorBuilder{
//             start: self.start,
//             spec: self.spec,
//             bd_processor: self.bd_processor,
//             end: EndDateTime(end),
//             marker_sealed: PhantomData,
//             timezone: self.timezone,
//         }
//     }
// }

// impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndDateTime<Tz>, Sealed> {
//     pub fn build(self) -> Result<SpecIterator<Tz>> {
//         let start = self.start.0;
//         let start = start.timezone().with_ymd_and_hms(start.year(), start.month(), start.day(), start.hour(), start.minute(), start.second()).unwrap();
//         Ok(SpecIterator::new_with_end(
//             &self.spec,
//             start,
//             self.bd_processor,
//             self.end.0,
//         )?)
//     }
// }

// impl<Tz: TimeZone> SpecIteratorBuilder<Tz, StartDateTime<Tz>, EndSpec, Sealed> {
//     pub fn build(self) -> Result<SpecIterator<Tz>> {
//         let start = self.start.0;
//         let start = start.timezone().with_ymd_and_hms(start.year(), start.month(), start.day(), start.hour(), start.minute(), start.second()).unwrap();
//         Ok(SpecIterator::new_with_end_spec(
//             &self.spec,
//             start,
//             self.bd_processor,
//             &self.end.0,
//         )?)
//     }
// }

// impl<Tz: TimeZone, START, S> SpecIteratorBuilder<Tz, START, NoEnd, S> {
//     pub fn new_with_start(
//         spec: &str,
//         start: DateTime<Tz>,
//         bdp: Box<dyn BizDayProcessor>,
//     ) -> SpecIteratorBuilder<Tz, StartDateTime<Tz>, NoEnd, NotSealed> {
//         SpecIteratorBuilder {
//             timezone: start.timezone(),
//             start: StartDateTime(start),
//             spec: spec.to_string(),
//             bd_processor: bdp,
//             end: NoEnd,
//             marker_sealed: PhantomData,
//         }
//     }

// }
// impl<Tz: TimeZone, S> SpecIteratorBuilder<Tz, StartDateTime<Tz>, NoEnd, S> {
//     pub fn build(self) -> Result<SpecIterator<Tz>> {
//         Ok(SpecIterator::new(
//             &self.spec,
//             self.start.0,
//             self.bd_processor,
//         )?)
//     }
// }


// impl<Tz: TimeZone> SpecIteratorBuilder<Tz, NoStart, NoEnd, NotSealed> {
//     pub fn new(
//         spec: &str,
//         tz: &Tz,
//         bdp: Box<dyn BizDayProcessor>,
//     ) -> SpecIteratorBuilder<Tz, StartDateTime<Utc>, NoEnd, NotSealed> {
//         let now = Utc::now();
//         let now = now.timezone().with_ymd_and_hms(now.year(), now.month(), now.day(), now.hour(), now.minute(), now.second()).unwrap();
//         SpecIteratorBuilder {
//             start: StartDateTime(now),
//             spec: spec.to_string(),
//             bd_processor: bdp,
//             end: NoEnd,
//             marker_sealed: PhantomData,
//             timezone: tz.clone(),
//         }
//     }

//     pub fn build(self) -> Result<SpecIterator<Tz>> {
//         let start = self.timezone.from_utc_datetime(&Utc::now().naive_utc()).with_timezone(&self.timezone);
//         Ok(SpecIterator::new(
//             &self.spec,
//             start,
//             self.bd_processor,
//         )?)
//     }
// }

// impl<Tz: TimeZone> SpecIterator<Tz> {
//     fn new(spec: &str, start: DateTime<Tz>, bd_processor: Box<dyn BizDayProcessor>) -> Result<Self> {
//         let spec = Spec::from_str(spec)?;
//         Ok(Self {
//             spec,
//             start: start.clone(),
//             end: None,
//             remaining: None,
//             bd_processor,
//             dtm: start,
//         })
//     }

//     fn new_with_end(
//         spec: &str,
//         start: DateTime<Tz>,
//         bd_processor: Box<dyn BizDayProcessor>,
//         end: DateTime<Tz>,
//     ) -> Result<Self> {
//         if end <= start {
//             return Err(Error::Custom(
//                 "end date-time must be after the start date-time",
//             ));
//         }
//         let spec = Spec::from_str(spec)?;
//         Ok(Self {
//             spec,
//             start: start.clone(),
//             end: Some(end),
//             remaining: None,
//             bd_processor,
//             dtm: start,
//         })
//     }

//     fn new_with_end_spec(
//         spec: &str,
//         start: DateTime<Tz>,
//         bd_processor: Box<dyn BizDayProcessor>,
//         end_spec: &str,
//     ) -> Result<Self> {
//         let spec = Spec::from_str(spec)?;
//         let end_spec = Spec::from_str(end_spec)?;
//         let mut time_spec_iter = TimeSpecIterator::new(&end_spec.time_spec, start.clone())?;
//         let end = match time_spec_iter.next()? {
//             Some(dtm) => {
//                 let mut date_spec_iter = DateSpecIteratorBuilder::new(&end_spec.date_spec, dtm, bd_processor.clone()).build()?;
//                 date_spec_iter.next()?
//             }
//             None => None,
//         };
//         let Some(end) = end else {
//             return Err(Error::Custom(
//                 "End spec must result in a date-time after the start date-time",
//             ));
//         };
//         if end <= start {
//             return Err(Error::Custom(
//                 "End spec must result in a date-time after the start date-time",
//             ));
//         }
//         Ok(Self {
//             spec,
//             start: start.clone(),
//             end: Some(end),
//             remaining: None,
//             bd_processor,
//             dtm: start,
//         })
//     }
// }

// impl<Tz: TimeZone> FallibleIterator for SpecIterator<Tz> {
//     type Item = DateTime<Tz>;
//     type Error = Error;

//     fn next(&mut self) -> Result<Option<Self::Item>> {
//         dbg!(&self.dtm);
//         let remaining = if let Some(remaining) = self.remaining {
//             if remaining == 0 {
//                 return Ok(None);
//             }
//             Some(remaining - 1)
//         } else {
//             None
//         };

//         if let Some(end) = &self.end {
//             if &self.dtm >= end {
//                 return Ok(None);
//             }
//         }

//         let mut time_spec_iter = TimeSpecIterator::new(&self.spec.time_spec, self.dtm.clone())?;
//         let next = match time_spec_iter.next()? {
//             None => self.dtm.clone(),
//             Some(next) => next,
//         };

//         let next = DateSpecIteratorBuilder::new(&self.spec.date_spec, next, self.bd_processor).build()?.next()?;

//         let Some(next) = next else {
//             return Ok(None);
//         };

//         if next <= self.dtm {
//             return Ok(None);
//         }

//         self.dtm = next;
//         self.remaining = remaining;

//         Ok(Some(self.dtm.clone()))
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use crate::biz_day::WeekendSkipper;
//     use chrono::Utc;
//     #[test]
//     fn test_spec_iter() {
//         let tz = Utc;
//         let now = Utc::now();
//         let tmp = SpecIteratorBuilder::new(
//             "YY:1M:31L:PT11:00:00",
//             // now,
//             &Utc,
//             Box::new(WeekendSkipper {}),
//         )
//         // .with_end(Utc::with_ymd_and_hms(&tz, 2025, 04, 30, 11, 00, 0).unwrap())
//         .build()
//         .unwrap();
//         let tmp = tmp.take(5).collect::<Vec<DateTime<_>>>().unwrap();
//         dbg!(&tmp);
//     }

// }
