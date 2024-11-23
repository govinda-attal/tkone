use std::{collections::BTreeSet, ops::Bound};

use chrono::{Datelike, NaiveDate, NaiveDateTime};

use crate::{utils::naive_date_with_last_day_of_month_in_year, NextResult};

pub struct NextResulterByMultiplesAndDay<'a> {
    dtm: &'a NaiveDateTime,
    years: Option<&'a BTreeSet<u32>>,
    months: Option<&'a BTreeSet<u32>>,
    days: Option<&'a BTreeSet<u32>>,
}

impl<'a> NextResulterByMultiplesAndDay<'a> {
    pub fn new(dtm: &'a NaiveDateTime) -> Self {
        Self {
            dtm,
            years: None,
            months: None,
            days: None,
        }
    }

    pub fn with_years(&mut self, years: &'a BTreeSet<u32>) -> &mut Self {
        self.years = Some(years);
        self
    }

    pub fn with_months(&mut self, months: &'a BTreeSet<u32>) -> &mut Self {
        self.months = Some(months);
        self
    }

    pub fn with_days(&mut self, days: &'a BTreeSet<u32>) -> &mut Self {
        self.days = Some(days);
        self
    }

    pub fn next(&self) -> NextResult<NaiveDateTime> {
        let dtm = self.dtm.clone();
        let mut year = dtm.year() as u32;
        let month = dtm.month();
        let day = dtm.day();

        if let Some(years) = &self.years {
            if years.contains(&(year as u32)) {
                let next = if let Some(months) = &self.months {
                    let next = if months.contains(&month) {
                        let next = if let Some(days) = &self.days {
                            let mut cursor = days.lower_bound(Bound::Excluded(&day));
                            let next = if let Some(next_day) = cursor.next() {
                                let nd = NaiveDate::from_ymd_opt(year as i32, month, *next_day)
                                    .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                        year as i32,
                                        month,
                                    ));
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                let first_day = days.first().unwrap();
                                let next_month = month + 1;
                                let mut cursor = months.lower_bound(Bound::Included(&next_month));
                                let next = if let Some(next_month) = cursor.next() {
                                    let nd = NaiveDate::from_ymd_opt(
                                        year as i32,
                                        *next_month,
                                        *first_day,
                                    )
                                    .unwrap_or(
                                        naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ),
                                    );
                                    NaiveDateTime::new(nd, dtm.time())
                                } else {
                                    let next_year = year + 1;
                                    let mut cursor = years.lower_bound(Bound::Included(&next_year));
                                    let next = if let Some(next_year) = cursor.next() {
                                        let first_month = months.first().unwrap();
                                        let nd = NaiveDate::from_ymd_opt(
                                            *next_year as i32,
                                            *first_month,
                                            *first_day,
                                        )
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            *next_year as i32,
                                            1,
                                        ));
                                        NaiveDateTime::new(nd, dtm.time())
                                    } else {
                                        dtm
                                    };
                                    next
                                };
                                next
                            };
                            next
                        } else {
                            let first_day = 1;
                            let next_month = month + 1;
                            let mut cursor = months.lower_bound(Bound::Included(&next_month));
                            let next = if let Some(next_month) = cursor.next() {
                                let nd =
                                    NaiveDate::from_ymd_opt(year as i32, *next_month, first_day)
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ));
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                let next_year = year + 1;
                                let mut cursor = years.lower_bound(Bound::Included(&next_year));
                                let next =
                                    if let Some(next_year) = cursor.next() {
                                        let first_month = months.first().unwrap();
                                        let nd = NaiveDate::from_ymd_opt(
                                            *next_year as i32,
                                            *first_month,
                                            first_day,
                                        )
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            *next_year as i32,
                                            1,
                                        ));
                                        NaiveDateTime::new(nd, dtm.time())
                                    } else {
                                        dtm
                                    };
                                next
                            };
                            next
                        };
                        next
                    } else {
                        let mut cursor = months.lower_bound(Bound::Excluded(&month));
                        let next = if let Some(next_month) = cursor.next() {
                            let next = if let Some(days) = &self.days {
                                let first_day = days.first().unwrap();
                                let nd =
                                    NaiveDate::from_ymd_opt(year as i32, *next_month, *first_day)
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ));
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                let first_day = 1;
                                let mut cursor = months.lower_bound(Bound::Excluded(&next_month));
                                let next =
                                    if let Some(next_month) = cursor.next() {
                                        let nd = NaiveDate::from_ymd_opt(
                                            year as i32,
                                            *next_month,
                                            first_day,
                                        )
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ));
                                        NaiveDateTime::new(nd, dtm.time())
                                    } else {
                                        let next_year = year + 1;
                                        let first_month = months.first().unwrap();
                                        let nd = NaiveDate::from_ymd_opt(
                                            next_year as i32,
                                            *first_month,
                                            first_day,
                                        )
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            next_year as i32,
                                            *first_month,
                                        ));
                                        NaiveDateTime::new(nd, dtm.time())
                                    };
                                next
                            };
                            next
                        } else {
                            let next_year = year + 1;
                            let mut cursor = years.lower_bound(Bound::Included(&next_year));
                            let next = if let Some(next_year) = cursor.next() {
                                let first_month = months.first().unwrap();
                                let first_day = if let Some(days) = &self.days {
                                    *days.first().unwrap()
                                } else {
                                    1
                                };
                                let nd = NaiveDate::from_ymd_opt(
                                    *next_year as i32,
                                    *first_month,
                                    first_day,
                                )
                                .unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(*next_year as i32, 1),
                                );
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                dtm
                            };
                            next
                        };
                        next
                    };
                    next
                } else {
                    let next = if let Some(days) = &self.days {
                        let mut cursor = days.lower_bound(Bound::Excluded(&day));
                        let next = if let Some(next_day) = cursor.next() {
                            let nd =
                                NaiveDate::from_ymd_opt(year as i32, month, *next_day).unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(year as i32, month),
                                );
                            NaiveDateTime::new(nd, dtm.time())
                        } else {
                            let next_month = month + 1;
                            let first_day = days.first().unwrap();
                            let nd = NaiveDate::from_ymd_opt(year as i32, next_month, *first_day)
                                .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                    year as i32,
                                    next_month,
                                ));
                            NaiveDateTime::new(nd, dtm.time())
                        };
                        next
                    } else {
                        let next_month = month + 1;
                        let next_month = if next_month > 12 {
                            year += 1;
                            1 as u32
                        } else {
                            next_month
                        };
                        let nd = NaiveDate::from_ymd_opt(year as i32, next_month, 1).unwrap_or(
                            naive_date_with_last_day_of_month_in_year(year as i32, next_month),
                        );
                        NaiveDateTime::new(nd, dtm.time())
                    };
                    next
                };
                NextResult::Single(next)
            } else {
                NextResult::Single(dtm)
            }
        } else {
            let next = if let Some(months) = &self.months {
                let next = if months.contains(&month) {
                    let next = if let Some(days) = &self.days {
                        let mut cursor = days.lower_bound(Bound::Excluded(&day));
                        let next = if let Some(next_day) = cursor.next() {
                            let nd =
                                NaiveDate::from_ymd_opt(year as i32, month, *next_day).unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(year as i32, month),
                                );
                            NaiveDateTime::new(nd, dtm.time())
                        } else {
                            let first_day = days.first().unwrap();
                            let next_month = month + 1;
                            let mut cursor = months.lower_bound(Bound::Included(&next_month));
                            let next = if let Some(next_month) = cursor.next() {
                                let nd =
                                    NaiveDate::from_ymd_opt(year as i32, *next_month, *first_day)
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ));
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                let next_year = year + 1;
                                let first_month = months.first().unwrap();
                                let nd = NaiveDate::from_ymd_opt(
                                    next_year as i32,
                                    *first_month,
                                    *first_day,
                                )
                                .unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(next_year as i32, 1),
                                );
                                NaiveDateTime::new(nd, dtm.time())
                            };
                            next
                        };
                        next
                    } else {
                        let first_day = 1;
                        let next_month = month + 1;
                        let mut cursor = months.lower_bound(Bound::Included(&next_month));
                        let next = if let Some(next_month) = cursor.next() {
                            let nd = NaiveDate::from_ymd_opt(year as i32, *next_month, first_day)
                                .unwrap();
                            NaiveDateTime::new(nd, dtm.time())
                        } else {
                            let next_year = year + 1;
                            let first_month = months.first().unwrap();
                            let nd =
                                NaiveDate::from_ymd_opt(next_year as i32, *first_month, first_day)
                                    .unwrap();
                            NaiveDateTime::new(nd, dtm.time())
                        };
                        next
                    };
                    next
                } else {
                    let year = year + 1;
                    let month = months.first().unwrap();
                    let next = if let Some(days) = &self.days {
                        let mut cursor = days.lower_bound(Bound::Excluded(&day));
                        let next = if let Some(next_day) = cursor.next() {
                            let nd =
                                NaiveDate::from_ymd_opt(year as i32, *month, *next_day).unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(year as i32, *month),
                                );
                            NaiveDateTime::new(nd, dtm.time())
                        } else {
                            let first_day = days.first().unwrap();
                            let next_month = month + 1;
                            let mut cursor = months.lower_bound(Bound::Included(&next_month));
                            let next = if let Some(next_month) = cursor.next() {
                                let nd =
                                    NaiveDate::from_ymd_opt(year as i32, *next_month, *first_day)
                                        .unwrap_or(naive_date_with_last_day_of_month_in_year(
                                            year as i32,
                                            *next_month,
                                        ));
                                NaiveDateTime::new(nd, dtm.time())
                            } else {
                                let next_year = year + 1;
                                let first_month = months.first().unwrap();
                                let nd = NaiveDate::from_ymd_opt(
                                    next_year as i32,
                                    *first_month,
                                    *first_day,
                                )
                                .unwrap_or(
                                    naive_date_with_last_day_of_month_in_year(
                                        next_year as i32,
                                        *first_month,
                                    ),
                                );
                                NaiveDateTime::new(nd, dtm.time())
                            };
                            next
                        };
                        next
                    } else {
                        let first_day = 1;
                        let next_month = month + 1;
                        let mut cursor = months.lower_bound(Bound::Included(&next_month));
                        let next = if let Some(next_month) = cursor.next() {
                            let nd = NaiveDate::from_ymd_opt(year as i32, *next_month, first_day)
                                .unwrap();
                            NaiveDateTime::new(nd, dtm.time())
                        } else {
                            let next_year = year + 1;
                            let first_month = months.first().unwrap();
                            let nd =
                                NaiveDate::from_ymd_opt(next_year as i32, *first_month, first_day)
                                    .unwrap();
                            NaiveDateTime::new(nd, dtm.time())
                        };
                        next
                    };
                    next
                };
                next
            } else {
                let next = if let Some(days) = &self.days {
                    let mut cursor = days.lower_bound(Bound::Excluded(&day));
                    let next = if let Some(next_day) = cursor.next() {
                        let nd = NaiveDate::from_ymd_opt(year as i32, month, *next_day).unwrap_or(
                            naive_date_with_last_day_of_month_in_year(year as i32, month),
                        );
                        NaiveDateTime::new(nd, dtm.time())
                    } else {
                        let next_month = month + 1;
                        let first_day = days.first().unwrap();
                        let nd =
                            NaiveDate::from_ymd_opt(year as i32, next_month, *first_day).unwrap_or(
                                naive_date_with_last_day_of_month_in_year(year as i32, next_month),
                            );
                        NaiveDateTime::new(nd, dtm.time())
                    };
                    next
                } else {
                    dtm
                };
                next
            };
            NextResult::Single(next)
        }
    }
}
