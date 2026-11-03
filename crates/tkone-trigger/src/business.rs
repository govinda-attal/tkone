//! Business-day schedules: datetime specs on a holiday calendar, resumed from
//! stored state so a restart never loses a run.
//!
//! A time spec repeats within the day and can simply start again from now.
//! A business schedule (`YY-MM-BD#3T09:00:00 [cal=GBLO]`) runs once a day or
//! less: a run missed while the process was down must not vanish. So a
//! [`BusinessSchedule`] keeps the last run it fired in a [`StateStore`] and,
//! when rebuilt, resumes after it. Runs it finds in the past are handled by
//! its [`CatchUp`] policy.
//!
//! Delivery is **at least once**: the run is recorded after its jobs finish,
//! so a crash while they run fires that run again on restart.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, FixedOffset, Utc};
use fallible_iterator::FallibleIterator;
use tkone_schedule::biz_day::BizDayProcessor;
use tkone_schedule::datetime::{SpecIterator, SpecIteratorBuilder};
use tkone_schedule::{Error, Occurrence};

use crate::ScheduleIter;

// ── State stores ─────────────────────────────────────────────────────────────

/// Where a [`BusinessSchedule`] keeps the last run it fired, by schedule key.
///
/// The value is the run's [`actual`](Occurrence::actual) time with its UTC
/// offset, which is what [`SpecIteratorBuilder::resume_after`] needs.
/// Implement it on your database to share state between processes.
pub trait StateStore: Send + 'static {
    /// The last run recorded for `key`, if any.
    fn load(&self, key: &str) -> Result<Option<DateTime<FixedOffset>>, Error>;
    /// Records `last_actual` as the last run for `key`.
    fn save(&mut self, key: &str, last_actual: DateTime<FixedOffset>) -> Result<(), Error>;
}

/// A [`StateStore`] in memory: state lasts as long as the process. Clones
/// share the same state, which makes it useful in tests.
#[derive(Debug, Clone, Default)]
pub struct MemoryStore(Arc<Mutex<BTreeMap<String, DateTime<FixedOffset>>>>);

impl MemoryStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl StateStore for MemoryStore {
    fn load(&self, key: &str) -> Result<Option<DateTime<FixedOffset>>, Error> {
        Ok(self.0.lock().expect("store lock").get(key).copied())
    }

    fn save(&mut self, key: &str, last_actual: DateTime<FixedOffset>) -> Result<(), Error> {
        self.0.lock().expect("store lock").insert(key.to_string(), last_actual);
        Ok(())
    }
}

/// A [`StateStore`] in a text file: one `key<TAB>RFC 3339 time` line per
/// schedule. Each save rewrites the file through a temporary file and a
/// rename, so a crash never leaves it half-written. For one process; use a
/// database-backed store to share state between processes.
#[derive(Debug, Clone)]
pub struct FileStore {
    path: PathBuf,
}

impl FileStore {
    /// A store kept in `path`, which is created on the first save.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn read(&self) -> Result<BTreeMap<String, DateTime<FixedOffset>>, Error> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
            Err(e) => return Err(Error::Custom(format!("{}: {e}", self.path.display()))),
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                let (key, time) = line
                    .split_once('\t')
                    .ok_or_else(|| Error::Custom(format!("{}: malformed line `{line}`", self.path.display())))?;
                let time = DateTime::parse_from_rfc3339(time)
                    .map_err(|e| Error::Custom(format!("{}: `{time}`: {e}", self.path.display())))?;
                Ok((key.to_string(), time))
            })
            .collect()
    }
}

impl StateStore for FileStore {
    fn load(&self, key: &str) -> Result<Option<DateTime<FixedOffset>>, Error> {
        Ok(self.read()?.get(key).copied())
    }

    fn save(&mut self, key: &str, last_actual: DateTime<FixedOffset>) -> Result<(), Error> {
        if key.contains(['\t', '\n']) {
            return Err(Error::Custom(format!("schedule key `{key}` must not contain a tab or newline")));
        }
        let mut all = self.read()?;
        all.insert(key.to_string(), last_actual);
        let text: String = all.iter().map(|(k, t)| format!("{k}\t{}\n", t.to_rfc3339())).collect();
        let tmp = self.path.with_extension("tmp");
        let io = |e: std::io::Error| Error::Custom(format!("{}: {e}", self.path.display()));
        std::fs::write(&tmp, text).map_err(io)?;
        std::fs::rename(&tmp, &self.path).map_err(io)
    }
}

// ── Catch-up ─────────────────────────────────────────────────────────────────

/// What a [`BusinessSchedule`] does with runs that fell while it was not
/// running (found in the past when it resumes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CatchUp {
    /// Fire the most recent missed run once, now, and skip the older ones:
    /// a payment run that missed two days runs once. The default.
    #[default]
    Latest,
    /// Fire every missed run, oldest first, now.
    All,
    /// Fire none; record them as passed and wait for the next run.
    Skip,
}

// ── BusinessSchedule ─────────────────────────────────────────────────────────

/// A datetime schedule on business days, resumed from a [`StateStore`].
///
/// ```rust,no_run
/// use tkone_trigger::{BusinessSchedule, CatchUp, FileStore, FireContext, Scheduler};
/// # use tkone_schedule::biz_day::WeekendSkipper;
/// # async fn run() -> Result<(), tkone_schedule::Error> {
/// # let calendar = WeekendSkipper::new();
/// // With tkone-calendar: `let calendar = Calendars::builtin().for_spec(SPEC)?;`
/// let schedule = BusinessSchedule::new(
///     "YY-MM-DD~SBT16:30:00 @2026-01-02[America/New_York]",
///     calendar,
///     FileStore::new("schedules.state"),
///     "usd-payment-run",
///     CatchUp::Latest,
/// )?;
/// let mut scheduler = Scheduler::new(schedule, |_ctx: FireContext, e: String| async move { eprintln!("{e}") });
/// scheduler.add(|ctx: FireContext| async move { println!("fired: {ctx:?}"); Ok(()) });
/// scheduler.try_run().await?;
/// # Ok(()) }
/// ```
pub struct BusinessSchedule<B: BizDayProcessor, S: StateStore> {
    iter: SpecIterator<chrono_tz::Tz, B>,
    zone: chrono_tz::Tz,
    store: S,
    key: String,
    catch_up: CatchUp,
    /// A run read ahead while deciding [`CatchUp::Latest`].
    pending: Option<Occurrence<DateTime<Utc>>>,
    now: fn() -> DateTime<Utc>,
}

fn to_utc(o: Occurrence<DateTime<chrono_tz::Tz>>) -> Occurrence<DateTime<Utc>> {
    match o {
        Occurrence::Exact(t) => Occurrence::Exact(t.with_timezone(&Utc)),
        Occurrence::AdjustedLater(a, t) => Occurrence::AdjustedLater(a.with_timezone(&Utc), t.with_timezone(&Utc)),
        Occurrence::AdjustedEarlier(a, t) => Occurrence::AdjustedEarlier(a.with_timezone(&Utc), t.with_timezone(&Utc)),
    }
}

impl<B: BizDayProcessor, S: StateStore> BusinessSchedule<B, S> {
    /// Builds the schedule `spec` on the business days of `bdp`, keeping its
    /// state in `store` under `key`.
    ///
    /// - **Time zone:** the one written in the spec (`@…[America/New_York]`),
    ///   else UTC. Write the calendar's own zone: a run whose date differs
    ///   there is refused (see `BizDayProcessor::time_zone`).
    /// - **Where it starts:** after the last run stored under `key`; the
    ///   first time, after now.
    ///
    /// # Errors
    ///
    /// If the spec is not a datetime spec, is invalid, names a calendar
    /// other than `bdp`, or the store cannot be read. A schedule that needs
    /// its start to resume (a union, or a cycle with positions) must write
    /// one (`@2026-01-02`).
    pub fn new(spec: &str, bdp: B, store: S, key: impl Into<String>, catch_up: CatchUp) -> Result<Self, Error> {
        Self::with_clock(spec, bdp, store, key, catch_up, Utc::now)
    }

    #[doc(hidden)]
    /// As [`new`](Self::new), with `now` as the clock (for tests).
    pub fn with_clock(
        spec: &str,
        bdp: B,
        store: S,
        key: impl Into<String>,
        catch_up: CatchUp,
        now: fn() -> DateTime<Utc>,
    ) -> Result<Self, Error> {
        let key = key.into();
        let zone = tkone_schedule::time_zone_of(spec)?.unwrap_or(chrono_tz::UTC);
        let iter = match store.load(&key)? {
            Some(last) => SpecIteratorBuilder::resume_after(spec, bdp, zone, last).build()?,
            None => SpecIteratorBuilder::new_after(spec, bdp, now().with_timezone(&zone)).build()?,
        };
        Ok(Self { iter, zone, store, key, catch_up, pending: None, now })
    }

    fn pull(&mut self) -> Result<Option<Occurrence<DateTime<Utc>>>, Error> {
        if let Some(o) = self.pending.take() {
            return Ok(Some(o));
        }
        Ok(self.iter.next()?.map(to_utc))
    }

    fn record(&mut self, o: &Occurrence<DateTime<Utc>>) -> Result<(), Error> {
        // Stored with the schedule's local offset, as `resume_after` expects.
        let actual = o.actual().with_timezone(&self.zone);
        let fixed = DateTime::parse_from_rfc3339(&actual.to_rfc3339()).expect("RFC 3339 round-trips");
        self.store.save(&self.key, fixed)
    }
}

impl<B: BizDayProcessor, S: StateStore> ScheduleIter for BusinessSchedule<B, S> {
    fn next_fire(&mut self) -> Option<Occurrence<DateTime<Utc>>> {
        self.try_next_fire().ok().flatten()
    }

    fn try_next_fire(&mut self) -> Result<Option<Occurrence<DateTime<Utc>>>, Error> {
        loop {
            let Some(run) = self.pull()? else {
                return Ok(None);
            };
            let now = (self.now)();
            if *run.observed() >= now {
                return Ok(Some(run));
            }
            match self.catch_up {
                CatchUp::All => return Ok(Some(run)),
                CatchUp::Skip => self.record(&run)?,
                CatchUp::Latest => match self.pull()? {
                    // A later run is also past: this one is not the latest.
                    Some(next) if *next.observed() < now => {
                        self.record(&run)?;
                        self.pending = Some(next);
                    }
                    next => {
                        self.pending = next;
                        return Ok(Some(run));
                    }
                },
            }
        }
    }

    fn fired(&mut self, run: &Occurrence<DateTime<Utc>>) -> Result<(), Error> {
        self.record(run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use tkone_schedule::biz_day::WeekendSkipper;

    const DAILY: &str = "YY-MM-DD~SBT09:00:00 @2026-01-01[Europe/London]";

    fn monday_noon() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap()
    }

    fn fmt(o: &Occurrence<DateTime<Utc>>) -> String {
        o.observed().format("%a %m-%d %H:%M").to_string()
    }

    fn last_tuesday() -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339("2026-10-27T09:00:00+00:00").unwrap()
    }

    fn schedule(store: MemoryStore, catch_up: CatchUp) -> BusinessSchedule<WeekendSkipper, MemoryStore> {
        BusinessSchedule::with_clock(DAILY, WeekendSkipper::new(), store, "daily", catch_up, monday_noon).unwrap()
    }

    #[test]
    fn test_first_run_starts_after_now() {
        let mut s = schedule(MemoryStore::new(), CatchUp::Latest);
        // 09:00 today has passed at noon: tomorrow's is next.
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Tue 11-03 09:00");
    }

    #[test]
    fn test_catch_up_policies() {
        // Last run Tuesday 27 October; now Monday 2 November at noon. Missed:
        // Wed 28, Thu 29, Fri 30 October and Mon 2 November 09:00.
        let run = |catch_up| {
            let mut store = MemoryStore::new();
            store.save("daily", last_tuesday()).unwrap();
            let mut s = schedule(store.clone(), catch_up);
            let first = s.try_next_fire().unwrap().unwrap();
            let second = s.try_next_fire().unwrap().unwrap();
            (fmt(&first), fmt(&second), store.load("daily").unwrap().unwrap().to_rfc3339())
        };
        // The latest missed run once, then the next; skipped ones are recorded.
        assert_eq!(run(CatchUp::Latest), ("Mon 11-02 09:00".into(), "Tue 11-03 09:00".into(), "2026-10-30T09:00:00+00:00".into()));
        // Every missed run, oldest first; nothing recorded until fired.
        assert_eq!(run(CatchUp::All), ("Wed 10-28 09:00".into(), "Thu 10-29 09:00".into(), "2026-10-27T09:00:00+00:00".into()));
        // None of them: straight to the next run.
        let mut store = MemoryStore::new();
        store.save("daily", last_tuesday()).unwrap();
        let mut s = schedule(store.clone(), CatchUp::Skip);
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Tue 11-03 09:00");
        assert_eq!(store.load("daily").unwrap().unwrap().to_rfc3339(), "2026-11-02T09:00:00+00:00");
    }

    #[test]
    fn test_fired_records_the_actual_time_with_its_offset() {
        let store = MemoryStore::new();
        let mut s = schedule(store.clone(), CatchUp::Latest);
        let run = s.try_next_fire().unwrap().unwrap();
        s.fired(&run).unwrap();
        assert_eq!(store.load("daily").unwrap().unwrap().to_rfc3339(), "2026-11-03T09:00:00+00:00");
        // Rebuilt from the store, it resumes after that run.
        let mut again = schedule(store.clone(), CatchUp::Latest);
        assert_eq!(fmt(&again.try_next_fire().unwrap().unwrap()), "Wed 11-04 09:00");
    }

    #[test]
    fn test_file_store() {
        let path = std::env::temp_dir().join(format!("tkone-trigger-test-{}.state", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut store = FileStore::new(&path);
        assert_eq!(store.load("a").unwrap(), None);
        store.save("a", last_tuesday()).unwrap();
        store.save("b", DateTime::parse_from_rfc3339("2026-03-29T02:30:00+01:00").unwrap()).unwrap();
        let reread = FileStore::new(&path);
        assert_eq!(reread.load("a").unwrap(), Some(last_tuesday()));
        assert_eq!(reread.load("b").unwrap().unwrap().to_rfc3339(), "2026-03-29T02:30:00+01:00");
        assert!(store.save("bad\tkey", last_tuesday()).is_err());
        std::fs::write(&path, "no tab here\n").unwrap();
        assert!(FileStore::new(&path).load("a").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_spec_errors() {
        // A date spec has no time to fire at.
        assert!(BusinessSchedule::new("YY-MM-BD#3", WeekendSkipper::new(), MemoryStore::new(), "k", CatchUp::Latest).is_err());
        // A spec naming a calendar needs that calendar.
        assert!(BusinessSchedule::new("YY-MM-BD#3T09:00:00 [cal=GBLO]", WeekendSkipper::new(), MemoryStore::new(), "k", CatchUp::Latest).is_err());
    }

    #[test]
    fn test_on_a_real_calendar_across_thanksgiving() {
        use tkone_calendar::Calendars;
        const SPEC: &str = "YY-MM-DD~SBT16:30:00 @2026-01-02[America/New_York] [cal=XNYS]";
        fn monday_after() -> DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap()
        }
        let mut store = MemoryStore::new();
        store.save("eod", DateTime::parse_from_rfc3339("2026-11-25T16:30:00-05:00").unwrap()).unwrap();
        let cal = Calendars::builtin().for_spec(SPEC).unwrap();
        let mut s = BusinessSchedule::with_clock(SPEC, cal, store, "eod", CatchUp::Latest, monday_after).unwrap();
        // Thanksgiving (Thu 26) is skipped by the calendar; Fri 27 was missed
        // and is fired once; then Monday's run.
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Fri 11-27 21:30");
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Mon 11-30 21:30");
    }

    #[derive(Clone)]
    struct FailingStore;
    impl StateStore for FailingStore {
        fn load(&self, _: &str) -> Result<Option<DateTime<FixedOffset>>, Error> {
            Ok(Some(last_tuesday()))
        }
        fn save(&mut self, _: &str, _: DateTime<FixedOffset>) -> Result<(), Error> {
            Err(Error::Custom("disk full".into()))
        }
    }

    #[tokio::test]
    async fn test_scheduler_fires_missed_runs_and_records_them() {
        use crate::{FireContext, Scheduler};
        use std::sync::atomic::{AtomicUsize, Ordering};
        // Ends on 30 October, so every run is in the past and fires at once.
        let spec = "YY-MM-DD~SBT09:00:00 @2026-01-01[Europe/London] ..2026-10-30";
        let mut store = MemoryStore::new();
        store.save("daily", last_tuesday()).unwrap();
        let s = BusinessSchedule::new(spec, WeekendSkipper::new(), store.clone(), "daily", CatchUp::All).unwrap();
        let fired = Arc::new(AtomicUsize::new(0));
        let mut scheduler = Scheduler::new(s, |_: FireContext, _: String| async {});
        let count = fired.clone();
        scheduler.add(move |_ctx: FireContext| {
            let count = count.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        scheduler.try_run().await.unwrap();
        assert_eq!(fired.load(Ordering::SeqCst), 3);
        assert_eq!(store.load("daily").unwrap().unwrap().to_rfc3339(), "2026-10-30T09:00:00+00:00");

        // A store that cannot save stops the scheduler with its error.
        let s = BusinessSchedule::new(spec, WeekendSkipper::new(), FailingStore, "daily", CatchUp::All).unwrap();
        let scheduler = Scheduler::new(s, |_: FireContext, _: String| async {});
        assert_eq!(scheduler.try_run().await, Err(Error::Custom("disk full".into())));
    }
}
