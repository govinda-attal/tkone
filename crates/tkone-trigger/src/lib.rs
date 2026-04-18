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
//!
//! **Only [`tkone_schedule::time`] specs are supported.** `date` and `datetime`
//! specs (daily, weekly, monthly) require persistent state to survive process
//! restarts — a missed tick on restart is silently lost. Use the `tempo` crate
//! for those schedules.
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
use tkone_schedule::Occurrence;
use tokio::task::JoinHandle;
pub use tokio_util::sync::CancellationToken;
pub use inventory;

// ── Start-spec resolution ─────────────────────────────────────────────────────

/// Resolves a `start_spec` string to a [`DateTime<Utc>`] for use as the
/// scheduler's start point.
///
/// Resolution order:
/// 1. **RFC 3339 / ISO 8601 datetime** — if `s` parses as a datetime, it is
///    returned directly.  Use this to pin a scheduler to a known wall-clock
///    moment: `"2026-06-01T09:00:00Z"`.
/// 2. **tkone-schedule time spec** — if `s` does not parse as a datetime, it is
///    treated as a time spec (e.g. `"09:00:00"` or `"1H:00:00"`).  The first
///    occurrence of that spec strictly after `Utc::now()` is returned.  Use this
///    to express relative starts such as "next 9 am" or "next top of the hour".
/// 3. **Fallback** — `Utc::now()` if neither parse succeeds.
///
/// # Examples
///
/// ```rust,no_run
/// use tkone_trigger::resolve_start_spec;
///
/// // Pin to a fixed datetime
/// let dt = resolve_start_spec("2026-06-01T09:00:00Z");
///
/// // Start from the next occurrence of 9 am
/// let dt = resolve_start_spec("09:00:00");
///
/// // Start from the next top of the hour
/// let dt = resolve_start_spec("1H:00:00");
/// ```
pub fn resolve_start_spec(s: &str) -> DateTime<Utc> {
    // 1. Try RFC 3339 / ISO 8601
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return dt.with_timezone(&Utc);
    }
    // 2. Try as a tkone-schedule time spec — return the next occurrence after now
    if let Ok(mut iter) = tkone_schedule::time::SpecIteratorBuilder::new_after(s, Utc::now()).build() {
        if let Ok(Some(dt)) = iter.next() {
            return dt.with_timezone(&Utc);
        }
    }
    // 3. Fallback
    Utc::now()
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
    /// since no business-day adjustment is applied. `tkone-trigger` only
    /// supports `time` specs; `date` and `datetime` iterators are excluded.
    fn occurrence(&self) -> &Occurrence<DateTime<Utc>>;
}

/// Concrete tick context passed to every callback and error handler.
///
/// Carries the [`Occurrence<DateTime<Utc>>`] for the current tick.
/// Clone is cheap — the inner `DateTime` values are `Copy`.
#[derive(Clone)]
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
/// Only [`tkone_schedule::time::SpecIterator`] implements this trait.
/// `tkone-trigger` is an **in-memory, intra-day** trigger — it does not persist
/// state across restarts. `date` and `datetime` specs (daily, weekly, monthly)
/// are intentionally excluded: a missed tick on process restart is silently lost,
/// which is unacceptable for those schedules. Use the `tempo` crate for
/// persistent, date-and-datetime-aware scheduling.
pub trait ScheduleIter: Send + 'static {
    /// Returns the next fire time as an [`Occurrence`] in UTC, or `None` when
    /// the iterator is exhausted or encounters an error.
    fn next_fire(&mut self) -> Option<Occurrence<DateTime<Utc>>>;
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
    pub async fn run(mut self) {
        if self.fire_on_start {
            let ctx = FireContext::new(Occurrence::Exact(Utc::now()));
            self.fire_callbacks(ctx).await;
        }

        loop {
            let Some(nr) = self.iter.next_fire() else {
                break;
            };

            let fire_at = nr.observed().clone();
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

            let ctx = FireContext::new(nr);
            self.fire_callbacks(ctx).await;
        }
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
