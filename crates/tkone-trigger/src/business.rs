//! Business-day schedules: a [`Schedule`] on a holiday calendar, resumed from
//! stored state so a restart never loses a run.
//!
//! A time spec repeats within the day and can simply start again from now.
//! A business schedule (`YY-MM-BD#3T09:00:00` on London's calendar) runs once
//! a day or less: a run missed while the process was down must not vanish. So a
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
use tkone_schedule::schedule::{Schedule, Slot, Slots};
use tkone_schedule::{Error, Occurrence};

use crate::{FireContext, ScheduleIter, TickContext};

// ── State stores ─────────────────────────────────────────────────────────────

/// Where a [`BusinessSchedule`] keeps the last run it fired, by schedule key.
///
/// The value is the run's time (its [`Slot::at`]) with its UTC offset; the
/// schedule resumes with the slots after it. Implement it on your database to
/// share state between processes.
pub trait StateStore: Send + 'static {
    /// The last run recorded for `key`, if any.
    fn load(&self, key: &str) -> Result<Option<DateTime<FixedOffset>>, Error>;
    /// Records `last` as the last run for `key`.
    fn save(&mut self, key: &str, last: DateTime<FixedOffset>) -> Result<(), Error>;
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

    fn save(&mut self, key: &str, last: DateTime<FixedOffset>) -> Result<(), Error> {
        self.0.lock().expect("store lock").insert(key.to_string(), last);
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

    fn save(&mut self, key: &str, last: DateTime<FixedOffset>) -> Result<(), Error> {
        if key.contains(['\t', '\n']) {
            return Err(Error::Custom(format!("schedule key `{key}` must not contain a tab or newline")));
        }
        let mut all = self.read()?;
        all.insert(key.to_string(), last);
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
    ///
    /// The skipped runs' rules are skipped with them: on a schedule whose
    /// runs are for different rules (`accrue` daily, `capitalise` at the
    /// month end), a missed month end is lost if a later daily run was
    /// missed too. Use [`CatchUp::All`] there.
    #[default]
    Latest,
    /// Fire every missed run, oldest first, now.
    All,
    /// Fire none; record them as passed and wait for the next run.
    Skip,
}

// ── BusinessSchedule ─────────────────────────────────────────────────────────

/// A [`Schedule`] on business days, resumed from a [`StateStore`].
///
/// ```rust,no_run
/// use chrono::TimeZone;
/// use tkone_schedule::schedule::Schedule;
/// use tkone_trigger::{BusinessSchedule, CatchUp, FileStore, FireContext, Scheduler};
/// # use tkone_schedule::biz_day::WeekendSkipper;
/// # async fn run() -> Result<(), tkone_schedule::Error> {
/// # let calendar = WeekendSkipper::new();
/// // With tkone-calendar: `let calendar = Calendars::builtin().get("XNYS")?;`
/// let start = chrono_tz::America::New_York.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
/// let payments = Schedule::builder(start, calendar).rule("YY-MM-DD~SBT16:30:00").build()?;
/// let schedule = BusinessSchedule::new(payments, FileStore::new("schedules.state"), "usd-payment-run", CatchUp::Latest)?;
/// let mut scheduler = Scheduler::new(schedule, |_ctx: FireContext, e: String| async move { eprintln!("{e}") });
/// scheduler.add(|ctx: FireContext| async move { println!("fired: {ctx:?}"); Ok(()) });
/// scheduler.try_run().await?;
/// # Ok(()) }
/// ```
pub struct BusinessSchedule<B: BizDayProcessor, S: StateStore> {
    slots: Slots<B>,
    zone: chrono_tz::Tz,
    store: S,
    key: String,
    catch_up: CatchUp,
    /// A run read ahead while deciding [`CatchUp::Latest`].
    pending: Option<FireContext>,
    now: fn() -> DateTime<Utc>,
}

/// A slot as a run: its time, moved from its first rule's actual time when an
/// adjustment moved it, and the rules it is for.
fn to_run(slot: Slot) -> FireContext {
    let at = slot.at.with_timezone(&Utc);
    let actual = slot.sources.first().map_or(at, |source| source.actual.with_timezone(&Utc));
    let occurrence = match actual.cmp(&at) {
        std::cmp::Ordering::Equal => Occurrence::Exact(at),
        std::cmp::Ordering::Less => Occurrence::AdjustedLater(actual, at),
        std::cmp::Ordering::Greater => Occurrence::AdjustedEarlier(actual, at),
    };
    FireContext::new(occurrence).with_sources(slot.sources)
}

impl<B: BizDayProcessor, S: StateStore> BusinessSchedule<B, S> {
    /// Runs `schedule`, keeping its state in `store` under `key`.
    ///
    /// - **Time zone and business days:** the schedule's. On a calendar with
    ///   a time zone, start the schedule in that zone: a run whose date differs
    ///   there is refused (see `BizDayProcessor::time_zone`).
    /// - **Where it starts:** after the last run stored under `key`; the
    ///   first time, after now.
    ///
    /// # Errors
    ///
    /// If the store cannot be read.
    pub fn new(schedule: Schedule<B>, store: S, key: impl Into<String>, catch_up: CatchUp) -> Result<Self, Error> {
        Self::with_clock(schedule, store, key, catch_up, Utc::now)
    }

    #[doc(hidden)]
    /// As [`new`](Self::new), with `now` as the clock (for tests).
    pub fn with_clock(
        schedule: Schedule<B>,
        store: S,
        key: impl Into<String>,
        catch_up: CatchUp,
        now: fn() -> DateTime<Utc>,
    ) -> Result<Self, Error> {
        let key = key.into();
        let zone = schedule.zone();
        let after = match store.load(&key)? {
            Some(last) => last.with_timezone(&zone),
            None => now().with_timezone(&zone),
        };
        let slots = schedule.slots_after(&after)?;
        Ok(Self { slots, zone, store, key, catch_up, pending: None, now })
    }

    fn pull(&mut self) -> Result<Option<FireContext>, Error> {
        if let Some(o) = self.pending.take() {
            return Ok(Some(o));
        }
        Ok(self.slots.next()?.map(to_run))
    }

    fn record(&mut self, o: &Occurrence<DateTime<Utc>>) -> Result<(), Error> {
        // Stored with the schedule's local offset, readable as it stands.
        let at = o.observed().with_timezone(&self.zone);
        let fixed = DateTime::parse_from_rfc3339(&at.to_rfc3339()).expect("RFC 3339 round-trips");
        self.store.save(&self.key, fixed)
    }
}

impl<B: BizDayProcessor, S: StateStore> ScheduleIter for BusinessSchedule<B, S> {
    fn next_fire(&mut self) -> Option<Occurrence<DateTime<Utc>>> {
        self.try_next_fire().ok().flatten()
    }

    fn try_next_fire(&mut self) -> Result<Option<Occurrence<DateTime<Utc>>>, Error> {
        Ok(self.try_next_context()?.map(|ctx| ctx.occurrence().clone()))
    }

    fn try_next_context(&mut self) -> Result<Option<FireContext>, Error> {
        loop {
            let Some(run) = self.pull()? else {
                return Ok(None);
            };
            let now = (self.now)();
            if *run.occurrence().observed() >= now {
                return Ok(Some(run));
            }
            match self.catch_up {
                CatchUp::All => return Ok(Some(run)),
                CatchUp::Skip => self.record(run.occurrence())?,
                CatchUp::Latest => match self.pull()? {
                    // A later run is also past: this one is not the latest.
                    Some(next) if *next.occurrence().observed() < now => {
                        self.record(run.occurrence())?;
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

    /// Weekdays at 09:00 in London, from 1 January 2026.
    fn daily() -> Schedule<WeekendSkipper> {
        let start = chrono_tz::Europe::London.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-DD~SBT09:00:00").build().unwrap()
    }

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
        BusinessSchedule::with_clock(daily(), store, "daily", catch_up, monday_noon).unwrap()
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
        assert_eq!(
            run(CatchUp::Latest),
            ("Mon 11-02 09:00".into(), "Tue 11-03 09:00".into(), "2026-10-30T09:00:00+00:00".into())
        );
        // Every missed run, oldest first; nothing recorded until fired.
        assert_eq!(
            run(CatchUp::All),
            ("Wed 10-28 09:00".into(), "Thu 10-29 09:00".into(), "2026-10-27T09:00:00+00:00".into())
        );
        // None of them: straight to the next run.
        let mut store = MemoryStore::new();
        store.save("daily", last_tuesday()).unwrap();
        let mut s = schedule(store.clone(), CatchUp::Skip);
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Tue 11-03 09:00");
        assert_eq!(store.load("daily").unwrap().unwrap().to_rfc3339(), "2026-11-02T09:00:00+00:00");
    }

    #[test]
    fn test_fired_records_the_time_with_its_offset() {
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
    fn test_on_a_real_calendar_across_thanksgiving() {
        use tkone_calendar::Calendars;
        fn monday_after() -> DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap()
        }
        let mut store = MemoryStore::new();
        store.save("eod", DateTime::parse_from_rfc3339("2026-11-25T16:30:00-05:00").unwrap()).unwrap();
        let start = chrono_tz::America::New_York.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
        let cal = Calendars::builtin().get("XNYS").unwrap();
        let eod = Schedule::builder(start, cal).rule("YY-MM-DD~SBT16:30:00").build().unwrap();
        let mut s = BusinessSchedule::with_clock(eod, store, "eod", CatchUp::Latest, monday_after).unwrap();
        // Thanksgiving (Thu 26) is skipped by the calendar; Fri 27 was missed
        // and is fired once; then Monday's run.
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Fri 11-27 21:30");
        assert_eq!(fmt(&s.try_next_fire().unwrap().unwrap()), "Mon 11-30 21:30");
    }

    /// `accrue` every London business day at 18:00, `capitalise` at the
    /// month end; ends Monday 2 November 2026.
    fn savings() -> Schedule<WeekendSkipper> {
        let london = chrono_tz::Europe::London;
        let start = london.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let until = london.with_ymd_and_hms(2026, 11, 2, 23, 59, 59).unwrap();
        Schedule::builder(start, WeekendSkipper::new())
            .named_rule("accrue", "YY-MM-DD~SBT18:00:00")
            .named_rule("capitalise", "YY-MM-L~PBT18:00:00")
            .until(until)
            .build()
            .unwrap()
    }

    fn rules(ctx: &FireContext) -> (String, Vec<String>) {
        let at = ctx.occurrence().observed().format("%a %d %H:%M").to_string();
        (at, ctx.sources().iter().filter_map(|s| s.rule_id.clone()).collect())
    }

    #[tokio::test]
    async fn test_a_run_names_the_rules_it_is_for() {
        use crate::Scheduler;
        let mut store = MemoryStore::new();
        store.save("savings", DateTime::parse_from_rfc3339("2026-10-28T18:00:00+00:00").unwrap()).unwrap();
        let s = BusinessSchedule::new(savings(), store, "savings", CatchUp::All).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut scheduler = Scheduler::new(s, |_: FireContext, _: String| async {});
        let log = seen.clone();
        scheduler.add(move |ctx: FireContext| {
            let log = log.clone();
            async move {
                log.lock().unwrap().push((rules(&ctx), ctx.is_for("capitalise")));
                Ok(())
            }
        });
        scheduler.try_run().await.unwrap();
        // 31 October 2026 is a Saturday: the month end is Friday the 30th,
        // one run for both rules, in rule order.
        let run = |at: &str, ids: &[&str]| (at.to_string(), ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(
            *seen.lock().unwrap(),
            [
                (run("Thu 29 18:00", &["accrue"]), false),
                (run("Fri 30 18:00", &["accrue", "capitalise"]), true),
                (run("Mon 02 18:00", &["accrue"]), false),
            ]
        );
    }

    #[test]
    fn test_catching_up_the_latest_skips_the_rules_of_skipped_runs() {
        fn monday_evening() -> DateTime<Utc> {
            Utc.with_ymd_and_hms(2026, 11, 2, 19, 0, 0).unwrap()
        }
        let mut store = MemoryStore::new();
        store.save("savings", DateTime::parse_from_rfc3339("2026-10-28T18:00:00+00:00").unwrap()).unwrap();
        let mut s = BusinessSchedule::with_clock(savings(), store, "savings", CatchUp::Latest, monday_evening).unwrap();
        // Friday's capitalisation was skipped with Friday's run.
        assert_eq!(rules(&s.try_next_context().unwrap().unwrap()), ("Mon 02 18:00".into(), vec!["accrue".into()]));
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
        let until = chrono_tz::Europe::London.with_ymd_and_hms(2026, 10, 30, 23, 59, 59).unwrap();
        let start = chrono_tz::Europe::London.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let ended = || {
            Schedule::builder(start, WeekendSkipper::new()).rule("YY-MM-DD~SBT09:00:00").until(until).build().unwrap()
        };
        let mut store = MemoryStore::new();
        store.save("daily", last_tuesday()).unwrap();
        let s = BusinessSchedule::new(ended(), store.clone(), "daily", CatchUp::All).unwrap();
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
        let s = BusinessSchedule::new(ended(), FailingStore, "daily", CatchUp::All).unwrap();
        let scheduler = Scheduler::new(s, |_: FireContext, _: String| async {});
        assert_eq!(scheduler.try_run().await, Err(Error::Custom("disk full".into())));
    }
}
