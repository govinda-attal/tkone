//! In-memory schedule triggering built on top of [`tkone_schedule`].
//!
//! [`Scheduler<I, E>`] owns a single [`ScheduleIter`] that fans out each tick
//! to N async callbacks. Each callback receives a [`FireContext`] carrying the
//! [`tkone_schedule::Occurrence`] for that tick, so handlers can inspect the
//! scheduled occurrence (actual vs. observed date, UTC fire time). Callbacks
//! return `Result<(), E>`; any error is routed to the async `on_error` handler,
//! which also receives the [`FireContext`]. Both `I` and `E` are fully generic
//! and inferred from the arguments to [`Scheduler::new`].
//!
//! | Spec example | Meaning |
//! |---|---|
//! | `"1H:00:00"` | Every hour on the hour |
//! | `"HH:30M:00"` | Every 30 minutes |
//! | `"HH:MM:10S"` | Every 10 seconds |
//! | `"09:30:00"` | Daily at 09:30 |
//! | `"[00-11,14-23]:30M:00"` | Every 30 minutes, except 12:00–13:59 |
//! | `"09-16:15M:00"` | Every 15 minutes from 09:00 to 16:45 |
//!
//! ## Time zones and starts
//!
//! [`time_schedule`] builds the iterator for a spec. Times are read on the
//! wall clock of a time zone: the `tz` you pass, else UTC. Steps are aligned to
//! the clock, as cron's are: `HH:15M:00` fires at :00, :15, :30 and :45 whenever
//! the process starts. A `start_spec` ([`resolve_start_spec_in`]) sets from when
//! on times are produced. Past occurrences are never fired.
//!
//! ## Business-day schedules
//!
//! A time spec repeats within the day and simply starts again from now after
//! a restart. A schedule that runs once a day or less, such as "16:30 on New
//! York business days" (`YY-MM-DD~SBT16:30:00` on the `USNY` calendar, in
//! `America/New_York`), must not lose a run to a restart. [`BusinessSchedule`]
//! runs such a [`Schedule`](tkone_schedule::schedule::Schedule) on a holiday calendar (any
//! [`BizDayProcessor`](tkone_schedule::biz_day::BizDayProcessor), such as a
//! `tkone-calendar` `Calendar`), records each run it fires in a
//! [`StateStore`] ([`MemoryStore`], [`FileStore`], or your own), and resumes
//! after the last one when rebuilt. Runs missed while it was down follow its
//! [`CatchUp`] policy. Delivery is at least once.
//!
//! *Run `cargo run -p tkone-trigger --example business_day_jobs` for a complete program.*
//!
//! ## Example
//!
//! *Run `cargo run -p tkone-trigger --example process_payments` for a complete program.*
#![doc = concat!("```rust,no_run\n", include_str!("../examples/process_payments.rs"), "```")]

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::{DateTime, TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use tkone_schedule::{Error, Occurrence};
use tokio::task::JoinHandle;
pub use tokio_util::sync::CancellationToken;
pub use inventory;

mod business;
pub use business::{BusinessSchedule, CatchUp, FileStore, MemoryStore, StateStore};

// ── Building a time schedule ─────────────────────────────────────────────────

/// Resolves a `start_spec` string to a [`DateTime<Utc>`]: see
/// [`resolve_start_spec_in`].
///
/// # Errors
///
/// Returns an error if `s` is neither an RFC 3339 datetime nor a valid time
/// spec.
///
/// # Examples
///
/// ```rust,no_run
/// use tkone_trigger::resolve_start_spec;
///
/// // Pin to a fixed datetime
/// let dt = resolve_start_spec("2026-06-01T09:00:00Z")?;
///
/// // Start from the next occurrence of 9 am (UTC)
/// let dt = resolve_start_spec("09:00:00")?;
/// # Ok::<(), tkone_schedule::Error>(())
/// ```
pub fn resolve_start_spec(s: &str) -> Result<DateTime<Utc>, Error> {
    resolve_start_spec_in(s, &Utc)
}

/// Resolves a `start_spec` string to a datetime in `tz`, for use as where a
/// scheduler starts iterating:
///
/// 1. **An RFC 3339 datetime** (`"2026-06-01T09:00:00Z"`) is that instant.
/// 2. **A time spec** (`"09:00:00"`, `"1H:00:00"`) is its first occurrence
///    after now, read on `tz`'s wall clock: "the next 9 am", "the next top of
///    the hour".
///
/// # Errors
///
/// Returns an error if `s` is neither an RFC 3339 datetime nor a valid time
/// spec, instead of silently starting from now.
pub fn resolve_start_spec_in<Tz: TimeZone>(s: &str, tz: &Tz) -> Result<DateTime<Tz>, Error> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(tz));
    }
    let now = Utc::now().with_timezone(tz);
    // "09:00:00" is the next 9 am, tomorrow's if today's has passed.
    let next = tkone_schedule::time::SpecIteratorBuilder::new_after(s, now)
        .repeat_daily()
        .build()
        .and_then(|mut iter| iter.next())
        .map_err(|e| Error::Custom(format!("start_spec `{s}` is neither an RFC 3339 datetime nor a time spec: {e}")))?;
    next.ok_or_else(|| Error::Custom(format!("start_spec `{s}` has no occurrence after now")))
}

/// Builds the iterator for a time-spec schedule, the way `#[schedule]` does.
///
/// - **Time zone:** `tz` (an IANA name such as `"Europe/London"`) if given,
///   else UTC. Times are read on that zone's wall clock, so `09:00:00` stays
///   at 09:00 local time across daylight-saving changes.
/// - **Steps are aligned to the clock** ([`TimeAnchor::Midnight`](tkone_schedule::TimeAnchor::Midnight)):
///   `HH:15M:00` is at :00, :15, :30 and :45, and `2H:00:00` at even hours,
///   whenever the process starts.
/// - **Fixed times repeat daily:** `09:30:00` fires at 09:30 every day.
/// - **Where it starts:** results are the times after `start_spec` (see
///   [`resolve_start_spec_in`]), or after now. Past occurrences are never
///   produced.
///
/// # Errors
///
/// Returns an error if the spec, `start_spec` or `tz` is invalid.
///
/// ```rust
/// let iter = tkone_trigger::time_schedule("09:00:00", None, Some("Europe/London"))?;
/// # Ok::<(), tkone_schedule::Error>(())
/// ```
pub fn time_schedule(
    spec: &str,
    start_spec: Option<&str>,
    tz: Option<&str>,
) -> Result<tkone_schedule::time::SpecIterator<chrono_tz::Tz>, Error> {
    let zone = match tz {
        Some(name) => name
            .parse::<chrono_tz::Tz>()
            .map_err(|_| Error::Custom(format!("`{name}` is not an IANA time zone")))?,
        None => chrono_tz::UTC,
    };
    let after = match start_spec {
        Some(s) => resolve_start_spec_in(s, &zone)?,
        None => Utc::now().with_timezone(&zone),
    };
    // A trigger repeats fixed times (`09:30:00`) every day, on the clock's grid.
    tkone_schedule::time::SpecIteratorBuilder::new_after(spec, after)
        .repeat_daily()
        .with_time_anchor(tkone_schedule::TimeAnchor::Midnight)
        .build()
}

// ── Context ──────────────────────────────────────────────────────────────────

/// Provides access to the schedule occurrence that triggered a tick.
///
/// Implemented by [`FireContext`], which is passed to every callback and error
/// handler on each tick. Write handlers against this trait for easier testing.
pub trait TickContext: Send + 'static {
    /// The [`Occurrence`] for this tick, carrying both the raw calendar date
    /// (`actual`) and the business-day-adjusted settlement date (`observed`).
    ///
    /// For `time`-based iterators the result is always [`Occurrence::Exact`]
    /// since no business-day adjustment is applied. A [`BusinessSchedule`]'s
    /// can be adjusted (`~NB` moved it to the next business day).
    fn occurrence(&self) -> &Occurrence<DateTime<Utc>>;
}

/// Concrete tick context passed to every callback and error handler.
///
/// Carries the [`Occurrence<DateTime<Utc>>`] for the current tick.
/// Clone is cheap — the inner `DateTime` values are `Copy`.
#[derive(Clone, Debug)]
pub struct FireContext {
    occurrence: Occurrence<DateTime<Utc>>,
}

impl FireContext {
    pub fn new(occurrence: Occurrence<DateTime<Utc>>) -> Self {
        Self { occurrence }
    }
}

impl TickContext for FireContext {
    fn occurrence(&self) -> &Occurrence<DateTime<Utc>> {
        &self.occurrence
    }
}

// ── Macro support types ───────────────────────────────────────────────────────

/// Alias for a pinned, boxed, `Send` future returning `()`.
///
/// Used as the return type of [`JobEntry::func`] and
/// [`ScheduleErrorHandler::handle_error`].
pub type BoxedFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// A registered job entry, submitted via `#[job(StructType)]`.
///
/// `func` is a plain function pointer so it can live in a `#[used]` static —
/// required by [`inventory`]'s link-time collection. The function receives a
/// [`FireContext`] for the current tick; error handling is baked in so it
/// always resolves to `()`.
pub struct JobEntry {
    /// Identifies which scheduler struct this job belongs to.
    pub schedule_type_id: TypeId,
    /// Constructs the job future for one tick, given the tick's context.
    pub func: fn(FireContext) -> BoxedFuture,
}

inventory::collect!(JobEntry);

/// Implemented by structs annotated with `#[schedule]`.
///
/// Bridges the `#[job]` macro's error handling to the struct's `#[on_error]`
/// method. Not intended to be implemented manually.
pub trait ScheduleErrorHandler<E: Send + 'static>: 'static {
    fn handle_error(ctx: FireContext, e: E) -> BoxedFuture;
}

// ── Internal callback types ───────────────────────────────────────────────────

type BoxedCallback<E> = Box<
    dyn Fn(FireContext) -> Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'static>>
        + Send
        + 'static,
>;

type BoxedErrorHandler<E> = Arc<
    dyn Fn(FireContext, E) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>>
        + Send
        + Sync
        + 'static,
>;

// ── ScheduleIter ─────────────────────────────────────────────────────────────

/// Drives a [`Scheduler`] tick by tick.
///
/// Implemented by [`tkone_schedule::time::SpecIterator`] (in memory: a time
/// spec simply starts again after a restart) and by [`BusinessSchedule`]
/// (datetime specs on business days, resumed from a [`StateStore`]). `date`
/// and `datetime` iterators do not implement it directly: without stored
/// state a restart would silently lose a run.
pub trait ScheduleIter: Send + 'static {
    /// Returns the next fire time as an [`Occurrence`] in UTC, or `None` when
    /// the iterator is exhausted or encounters an error.
    fn next_fire(&mut self) -> Option<Occurrence<DateTime<Utc>>>;

    /// Like [`next_fire`](Self::next_fire), but reports an error instead of
    /// ending. [`Scheduler::try_run`] uses it.
    fn try_next_fire(&mut self) -> Result<Option<Occurrence<DateTime<Utc>>>, Error> {
        Ok(self.next_fire())
    }

    /// Called after every callback for `occurrence` has finished, so the
    /// iterator can record it. Does nothing by default.
    fn fired(&mut self, _occurrence: &Occurrence<DateTime<Utc>>) -> Result<(), Error> {
        Ok(())
    }
}

impl<Tz> ScheduleIter for tkone_schedule::time::SpecIterator<Tz>
where
    Tz: TimeZone + Send + Sync + 'static,
    Tz::Offset: Send + Sync,
{
    // Time specs have no business-day adjustment; result is always Exact.
    fn next_fire(&mut self) -> Option<Occurrence<DateTime<Utc>>> {
        self.next().ok()?.map(|dt| Occurrence::Exact(dt.with_timezone(&Utc)))
    }
}

// ── Scheduler ────────────────────────────────────────────────────────────────

/// In-memory scheduler that fans out each schedule tick to N async callbacks.
///
/// - `I` — the iterator type; inferred from the value passed to [`Scheduler::new`].
/// - `E` — the error type returned by callbacks; inferred from the `on_error` handler.
///
/// Each callback and the error handler receive a [`FireContext`] for the tick,
/// giving access to the [`Occurrence`] (actual vs. observed occurrence).
/// Errors are routed to `on_error`; the scheduler always continues to the next tick.
pub struct Scheduler<I: ScheduleIter, E: Send + 'static> {
    iter: I,
    shutdown: CancellationToken,
    callbacks: Vec<(CancellationToken, BoxedCallback<E>)>,
    on_error: BoxedErrorHandler<E>,
    fire_on_start: bool,
}

impl<I: ScheduleIter, E: Send + 'static> Scheduler<I, E> {
    /// Create a scheduler driven by `iter`.
    ///
    /// `on_error` receives the [`FireContext`] and the error value whenever a
    /// callback returns `Err`. It may log, alert, or record metrics. The
    /// scheduler always continues to the next tick.
    pub fn new<H, Hf>(iter: I, on_error: H) -> Self
    where
        H: Fn(FireContext, E) -> Hf + Send + Sync + 'static,
        Hf: Future<Output = ()> + Send + 'static,
    {
        Self {
            iter,
            shutdown: CancellationToken::new(),
            callbacks: Vec::new(),
            on_error: Arc::new(move |ctx, e| {
                Box::pin(on_error(ctx, e)) as Pin<Box<dyn Future<Output = ()> + Send + 'static>>
            }),
            fire_on_start: false,
        }
    }

    /// Register an async callback fired on every tick of the schedule.
    ///
    /// The callback receives a [`FireContext`] for each tick.
    /// Returns a [`CancellationToken`] that silences only this callback on
    /// future ticks without stopping the iterator or other callbacks.
    pub fn add<F, Fut>(&mut self, callback: F) -> CancellationToken
    where
        F: Fn(FireContext) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
    {
        let token = CancellationToken::new();
        self.callbacks
            .push((token.clone(), Box::new(move |ctx| Box::pin(callback(ctx)))));
        token
    }

    /// Fire all registered callbacks once immediately when [`run`](Self::run) is called,
    /// before waiting for the first scheduled tick.
    ///
    /// The [`FireContext`] for the startup fire carries `Occurrence::Exact(Utc::now())`.
    pub fn fire_on_start(mut self) -> Self {
        self.fire_on_start = true;
        self
    }

    /// Returns a [`CancellationToken`] that, when cancelled, stops the scheduler
    /// at its next opportunity. Useful for triggering shutdown from outside [`run`](Self::run).
    ///
    /// ```rust,no_run
    /// # use tkone_trigger::{Scheduler, FireContext};
    /// # use tkone_schedule::time::SpecIteratorBuilder as TimeBuilder;
    /// # async fn example() {
    /// # let mut scheduler = Scheduler::new(
    /// #     TimeBuilder::new("1H:00:00", chrono::Utc).build().unwrap(),
    /// #     |_ctx: FireContext, _e: String| async {},
    /// # );
    /// let token = scheduler.shutdown_token();
    /// tokio::spawn(async move {
    ///     tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    ///     token.cancel();
    /// });
    /// scheduler.run().await;
    /// # }
    /// ```
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Replace the internal shutdown token (used by the `#[schedule]` macro to
    /// wire the struct's static token into this scheduler instance).
    pub fn with_shutdown_token(mut self, token: CancellationToken) -> Self {
        self.shutdown = token;
        self
    }

    /// Register a pre-boxed job function (used by the `#[schedule]` macro).
    ///
    /// The function receives a [`FireContext`] and has error handling already
    /// baked in, so it always returns `()`.
    pub fn add_job(&mut self, func: fn(FireContext) -> BoxedFuture) {
        let token = CancellationToken::new();
        self.callbacks.push((
            token,
            Box::new(move |ctx: FireContext| {
                let fut = func(ctx);
                Box::pin(async move {
                    fut.await;
                    Ok(())
                })
            }),
        ));
    }

    /// Cancel all callbacks and stop the scheduler.
    pub async fn shutdown(self) {
        self.shutdown.cancel();
    }

    /// Drive the schedule until the iterator is exhausted or [`shutdown`](Self::shutdown) is called.
    ///
    /// On each tick all non-cancelled callbacks are spawned as concurrent tasks,
    /// each receiving a [`FireContext`] for that tick. Errors are forwarded to
    /// `on_error`; the scheduler always advances to the next tick.
    /// If [`fire_on_start`](Self::fire_on_start) was called, all callbacks fire once immediately
    /// before waiting for the first scheduled tick.
    pub async fn run(self) {
        let _ = self.try_run().await;
    }

    /// Like [`run`](Self::run), but stops at the first error from the
    /// schedule, such as a [`StateStore`] that cannot save, and returns it.
    /// Callback errors still go to `on_error` and do not stop it.
    ///
    /// # Errors
    ///
    /// The schedule's error, if it ends with one.
    pub async fn try_run(mut self) -> Result<(), Error> {
        if self.fire_on_start {
            let ctx = FireContext::new(Occurrence::Exact(Utc::now()));
            self.fire_callbacks(ctx).await;
        }

        while let Some(nr) = self.iter.try_next_fire()? {
            let fire_at = *nr.observed();
            let delay = fire_at - Utc::now();
            if delay > chrono::Duration::zero() {
                let sleep = delay.to_std().unwrap_or_default();
                tokio::select! {
                    _ = self.shutdown.cancelled() => break,
                    _ = tokio::time::sleep(sleep) => {}
                }
            }

            if self.shutdown.is_cancelled() {
                break;
            }

            let ctx = FireContext::new(nr.clone());
            self.fire_callbacks(ctx).await;
            self.iter.fired(&nr)?;
        }
        Ok(())
    }

    async fn fire_callbacks(&self, ctx: FireContext) {
        let handles: Vec<JoinHandle<()>> = self
            .callbacks
            .iter()
            .filter(|(token, _)| !token.is_cancelled())
            .map(|(token, cb)| {
                let ctx_for_cb = ctx.clone();
                let ctx_for_err = ctx.clone();
                let fut = cb(ctx_for_cb);
                let token = token.clone();
                let shutdown = self.shutdown.clone();
                let on_error = self.on_error.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = token.cancelled() => {}
                        _ = shutdown.cancelled() => {}
                        result = fut => {
                            if let Err(e) = result {
                                on_error(ctx_for_err, e).await;
                            }
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            let _ = h.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Timelike, TimeZone};

    #[test]
    fn test_resolve_start_spec() {
        assert_eq!(
            resolve_start_spec("2026-06-01T09:00:00Z").unwrap(),
            Utc.with_ymd_and_hms(2026, 6, 1, 9, 0, 0).unwrap()
        );
        assert!(resolve_start_spec("HH:MM:00").unwrap() > Utc::now());
        // A typo is an error, not a silent "now".
        assert!(resolve_start_spec("9 am").is_err());
        // A relative start is read on the given zone's clock.
        let london = chrono_tz::Europe::London;
        let next_nine = resolve_start_spec_in("09:00:00", &london).unwrap();
        assert_eq!((next_nine.hour(), next_nine.minute()), (9, 0));
    }

    #[test]
    fn test_time_schedule_time_zones() {
        let first_hour = |spec, tz| {
            let mut iter = time_schedule(spec, None, tz).unwrap();
            let t = iter.next().unwrap().unwrap();
            (t.hour(), t.timezone().name().to_string())
        };
        // UTC by default, else `tz`.
        assert_eq!(first_hour("09:00:00", None), (9, "UTC".into()));
        assert_eq!(first_hour("09:00:00", Some("Asia/Tokyo")), (9, "Asia/Tokyo".into()));
        // A zone is not written in the spec.
        assert!(time_schedule("09:00:00 @2026-01-05[America/New_York]", None, None).is_err());
        assert!(time_schedule("09:00:00", None, Some("Mars/Base")).is_err());
        assert!(time_schedule("YY-MM-DD", None, None).is_err());
    }

    #[test]
    fn test_steps_are_on_the_clock() {
        // From 00:07, every 15 minutes is :15, :30, …, not :22, :37.
        let mut iter = time_schedule("HH:15M:00", Some("2030-01-01T00:07:00Z"), None).unwrap();
        let first = iter.next().unwrap().unwrap();
        assert_eq!(first, Utc.with_ymd_and_hms(2030, 1, 1, 0, 15, 0).unwrap());
        let mut iter = time_schedule("2H:00:00", Some("2030-01-01T01:30:00Z"), None).unwrap();
        assert_eq!(iter.next().unwrap().unwrap(), Utc.with_ymd_and_hms(2030, 1, 1, 2, 0, 0).unwrap());
        // Nothing in the past.
        let mut iter = time_schedule("1H:00:00", None, None).unwrap();
        assert!(iter.next().unwrap().unwrap() > Utc::now());
    }
}
