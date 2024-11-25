use std::{collections::BTreeSet, ops::Bound};

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};

use crate::{
    utils::{naive_date_with_last_day_of_month_in_year, DateLikeUtils},
    NextResult,
};

use super::spec::{LastDayOption, WeekdayOption};

pub(super) struct NextResulterByMultiplesAndDay<'a> {
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

    pub fn next(&self) -> Option<NextResult<NaiveDateTime>> {
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
                                        return None;
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
                                        return None;
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
                                return None;
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
                Some(NextResult::Single(next))
            } else {
                None
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
                    return None;
                };
                next
            };
            Some(NextResult::Single(next))
        }
    }
}

#[derive(Debug)]
pub(super) struct NextResulterByDay<'a> {
    dtm: &'a NaiveDateTime,
    ld_opt: Option<LastDayOption>,
    day: Option<u32>,
    month: Option<u32>,
    year: Option<u32>,
}

impl<'a> NextResulterByDay<'a> {
    pub fn new(dtm: &'a NaiveDateTime) -> Self {
        Self {
            dtm,
            day: None,
            month: None,
            year: None,
            ld_opt: None,
        }
    }

    pub fn day(&mut self, day: u32) -> &mut Self {
        self.day = Some(day);
        self
    }

    pub fn month(&mut self, month: u32) -> &mut Self {
        self.month = Some(month);
        self
    }

    pub fn year(&mut self, year: u32) -> &mut Self {
        self.year = Some(year);
        self
    }

    pub fn last_day_option(&mut self, opt: &LastDayOption) -> &mut Self {
        self.ld_opt = Some(opt.clone());
        self
    }

    pub fn last_day(&mut self) -> &mut Self {
        self.ld_opt = Some(LastDayOption::LastDay);
        self
    }

    // given that day, month and year are all optional, need to write function such that
    // if all three are not provided it should pick next day, it is okay to overflow to next month in dtm
    // if month is provided it should pick next day in that month and adjusted or observed datetime in `next result`` should be as per day option, if year is not provided it is okay to overflow to next year in dtm
    // if year is provided and month is none then it should pick next day in that year and adjusted or observed datetime in `next result`` should be as per day option. it is okay for next to overflow to next month in dtm
    // if year is provided and month is provided then it should pick next day in that month and year and adjusted or observed datetime in `next result`` should be as per day option
    // if
    pub fn build(&self) -> Option<NextResult<NaiveDateTime>> {
        let dtm = self.dtm.clone();
        let ld_opt = self.ld_opt.as_ref().unwrap_or(&LastDayOption::NA);

        let month = self.month.unwrap_or(dtm.month());
        let year = self
            .year
            .map(|year| year as i32)
            .unwrap_or(dtm.year() as i32);


        let day = self.day.unwrap_or_else(|| {
            if ld_opt == &LastDayOption::LastDay {
                if month == 12 {
                    let next_day = NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap();
                    let last_day = next_day.pred_opt().unwrap();
                    last_day.day()
                } else {
                    let next_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                    let last_day = next_day.pred_opt().unwrap();
                    last_day.day()
                }
            } else {
                dtm.day()
            }
        });

        if let Some(updated) = NaiveDate::from_ymd_opt(year, month, day) {
            return Some(NextResult::Single(NaiveDateTime::new(updated, dtm.time())));
        }

        let occurrence = match *ld_opt {
            LastDayOption::NA | LastDayOption::LastDay => {
                let next_mnth_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_mnth_day.pred_opt().unwrap();
                NextResult::Single(NaiveDateTime::new(last_day, dtm.time()))
            }
            LastDayOption::NextMonthFirstDay => {
                let next_mnth_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_mnth_day.pred_opt().unwrap();
                NextResult::AdjustedLater(
                    NaiveDateTime::new(last_day, dtm.time()),
                    NaiveDateTime::new(next_mnth_day, dtm.time()),
                )
            }
            LastDayOption::NextMonthOverflow => {
                let next_mnth_day = NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap();
                let last_day = next_mnth_day.pred_opt().unwrap();
                let last_day_num = last_day.day();
                NextResult::AdjustedLater(
                    NaiveDateTime::new(last_day, dtm.time()),
                    dtm + Duration::days(day as i64 - last_day_num as i64),
                )
            }
        };
        if occurrence.actual() > &dtm {
            Some(occurrence)
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub(super) struct NextResulterByWeekDay<'a> {
    dtm: &'a NaiveDateTime,
    wd: &'a Weekday,
    wd_opt: &'a WeekdayOption,
    month: Option<u32>,
    year: Option<u32>,
    num_months: Option<u32>,
    num_years: Option<u32>,
}

impl<'a> NextResulterByWeekDay<'a> {
    pub fn new(dtm: &'a NaiveDateTime, wd: &'a Weekday, wd_opt: &'a WeekdayOption) -> Self {
        Self {
            dtm,
            wd,
            wd_opt,
            month: None,
            year: None,
            num_months: None,
            num_years: None,
        }
    }

    pub fn month(&mut self, month: u32) -> &mut Self {
        self.month = Some(month);
        self
    }

    pub fn year(&mut self, year: u32) -> &mut Self {
        self.year = Some(year);
        self
    }

    pub fn num_months(&mut self, num_months: u32) -> &mut Self {
        self.num_months = Some(num_months);
        self
    }

    pub fn num_years(&mut self, num_years: u32) -> &mut Self {
        self.num_years = Some(num_years);
        self
    }

    pub fn build(&self) -> Option<NextResult<NaiveDateTime>> {
        let dtm = self.dtm.clone();
        let wd = self.wd;
        let wd_opt = self.wd_opt;
        let mut next_rs_by_day = &mut NextResulterByDay::new(&dtm);

        let year_month = self.month.map_or_else(
            || {
                let Some(num_months) = self.num_months else {
                    return None;
                };
                let (year, month) = ffwd_months(&dtm, num_months);
                Some((Some(year), month))
            },
            |month| Some((None, month)),
        );

        let year = self.year.or_else(|| {
            self.num_years.map(|num_years| {
                let diff = if let Some((Some(year), _)) = &year_month {
                    *year as i32 - dtm.year()
                } else {
                    0
                };
                dtm.year() as u32 + num_years + diff as u32
            })
        });
        if let Some((Some(year), month)) = year_month {
            next_rs_by_day = next_rs_by_day.month(month).year(year);
        } else if let Some((None, month)) = year_month {
            next_rs_by_day = next_rs_by_day.month(month);
        }

        if let Some(year) = year {
            next_rs_by_day = next_rs_by_day.year(year);
        }

        dbg!(&next_rs_by_day);

        let interim_result = next_rs_by_day.build();

        let Some(interim_result) = interim_result else {
            return None;
        };

        let interim = interim_result.actual().clone();
        dbg!(&interim);
        let next = match wd_opt {
            WeekdayOption::Starting(occurrence) => {
                let occurrence = occurrence.unwrap_or(1);
                interim.to_months_weekday(wd, occurrence).unwrap_or(interim)
            }
            WeekdayOption::Ending(occurrence) => {
                let occurrence = occurrence.unwrap_or(1);
                interim
                    .to_months_last_weekday(wd, occurrence)
                    .unwrap_or(interim)
            }
            WeekdayOption::NA => {
                let next = interim.to_weekday(wd);
                if next == interim {
                    next + Duration::days(7)
                } else {
                    next
                }
            }
        };

        if let Some(year) = self.year {
            if next.year() != year as i32 {
                return None;
            }
        }

        if let Some(month) = self.month {
            if next.month() != month {
                return None;
            }
        }
        Some(NextResult::Single(next))
    }
}

pub(super) fn ffwd_months(dtm: &NaiveDateTime, num: u32) -> (u32, u32) {
    let mut new_month = dtm.month() + num;
    let mut new_year = dtm.year() as u32;
    new_year += (new_month - 1) / 12;
    new_month = (new_month - 1) % 12 + 1;
    (new_year, new_month)
}
