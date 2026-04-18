//! In-memory schedule triggering built on top of [`lib_schedule`].
//!
//! [`Scheduler<I, E>`] owns a single [`ScheduleIter`] that fans out each tick
//! to N async callbacks. Callbacks return `Result<(), E>`; any error is routed
//! to the async `on_error` handler supplied at construction. Both `I` and `E`
//! are fully generic and inferred from the arguments to [`Scheduler::new`].
//!
//! | Iterator type | Spec example |
//! |---------------|--------------|
//! | [`lib_schedule::time::SpecIterator`] | `"1H:00:00"` |
//! | [`lib_schedule::datetime::SpecIterator`] | `"YY-1M-L~WT11:00:00"` |
//! | [`lib_schedule::date::SpecIterator`] | `"YY-MM-FRI#L"` |
//!
//! > **Note:** Although `datetime` and `date` iterators are supported, this
//! > crate is an **in-memory** trigger — state is not persisted across process
//! > restarts. Long-distance recurrences (daily, weekly, monthly) will be
//! > missed if the process is restarted between ticks. For those schedules
//! > consider pairing this crate with an external state store or using a
//! > persistent job queue. `time`-based intra-day recurrences (every N
//! > seconds/minutes/hours within a single day) are the primary intended use.
//!
//! ## Example
//!
//! *Run `cargo run -p lib-trigger --example process_payments` for a complete program.*
#![doc = concat!("```rust,no_run\n", include_str!("../examples/process_payments.rs"), "```")]

use std::any::TypeId;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Alias for a pinned, boxed, `Send` future returning `()`.
///
/// Used as the return type of [`JobEntry::func`] and
/// [`ScheduleErrorHandler::handle_error`].
pub type BoxedFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

/// A registered job entry, submitted via `#[job(StructType)]`.
///
/// `func` is a plain function pointer (not a closure) so that it can be stored
/// in a `#[used]` static — required by [`inventory`]'s link-time collection.
/// Error handling is already baked into the function: on `Err` it calls the
/// struct's `#[on_error]` handler, so the returned future always resolves to `()`.
pub struct JobEntry {
    /// Identifies which scheduler struct this job belongs to.
    pub schedule_type_id: TypeId,
    /// Constructs the job future for one tick. Always returns `()`.
    pub func: fn() -> BoxedFuture,
}

inventory::collect!(JobEntry);

/// Implemented by structs annotated with `#[schedule]`.
///
/// Bridges the `#[job]` macro's error handling to the struct's `#[on_error]` method.
/// Not intended to be implemented manually.
pub trait ScheduleErrorHandler<E: Send + 'static>: 'static {
    fn handle_error(e: E) -> BoxedFuture;
}

use chrono::{DateTime, TimeZone, Utc};
use fallible_iterator::FallibleIterator;
use lib_schedule::biz_day::BizDayProcessor;
use tokio::task::JoinHandle;
pub use tokio_util::sync::CancellationToken;
pub use inventory;

type BoxedCallback<E> =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), E>> + Send + 'static>> + Send + 'static>;

type BoxedErrorHandler<E> =
    Arc<dyn Fn(E) -> Pin<Box<dyn Future<Output = ()> + Send + 'static>> + Send + Sync + 'static>;

/// Abstracts over the three [`lib_schedule`] iterator families.
///
/// Implemented for:
/// - [`lib_schedule::datetime::SpecIterator<Tz, Bdp>`]
/// - [`lib_schedule::date::SpecIterator<Tz, Bdp>`]
/// - [`lib_schedule::time::SpecIterator<Tz>`]
pub trait ScheduleIter: Send + 'static {
    /// Returns the next UTC fire time, or `None` when exhausted or on error.
    fn next_fire(&mut self) -> Option<DateTime<Utc>>;
}

impl<Tz, Bdp> ScheduleIter for lib_schedule::datetime::SpecIterator<Tz, Bdp>
where
    Tz: TimeZone + Send + Sync + 'static,
    Tz::Offset: Send + Sync,
    Bdp: BizDayProcessor + Send + 'static,
{
    fn next_fire(&mut self) -> Option<DateTime<Utc>> {
        self.next().ok()?.map(|nr| nr.observed().with_timezone(&Utc))
    }
}

impl<Tz, Bdp> ScheduleIter for lib_schedule::date::SpecIterator<Tz, Bdp>
where
    Tz: TimeZone + Send + Sync + 'static,
    Tz::Offset: Send + Sync,
    Bdp: BizDayProcessor + Send + 'static,
{
    fn next_fire(&mut self) -> Option<DateTime<Utc>> {
        self.next().ok()?.map(|nr| nr.observed().with_timezone(&Utc))
    }
}

impl<Tz> ScheduleIter for lib_schedule::time::SpecIterator<Tz>
where
    Tz: TimeZone + Send + Sync + 'static,
    Tz::Offset: Send + Sync,
{
    fn next_fire(&mut self) -> Option<DateTime<Utc>> {
        self.next().ok()?.map(|dt| dt.with_timezone(&Utc))
    }
}

/// In-memory scheduler that fans out each schedule tick to N async callbacks.
///
/// - `I` — the iterator type; inferred from the value passed to [`Scheduler::new`].
/// - `E` — the error type returned by callbacks; inferred from the `on_error` handler.
///
/// Errors from callbacks are routed to the `on_error` handler; the scheduler
/// continues to the next tick regardless.
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
    /// `on_error` is called whenever a callback returns `Err`; it receives the
    /// error value and may log, alert, or record metrics. The scheduler always
    /// continues to the next tick.
    pub fn new<H, Hf>(iter: I, on_error: H) -> Self
    where
        H: Fn(E) -> Hf + Send + Sync + 'static,
        Hf: Future<Output = ()> + Send + 'static,
    {
        Self {
            iter,
            shutdown: CancellationToken::new(),
            callbacks: Vec::new(),
            on_error: Arc::new(move |e| Box::pin(on_error(e)) as Pin<Box<dyn Future<Output = ()> + Send + 'static>>),
            fire_on_start: false,
        }
    }

    /// Register an async callback fired on every tick of the schedule.
    ///
    /// Returns a [`CancellationToken`] that silences only this callback on
    /// future ticks without stopping the iterator or other callbacks.
    pub fn add<F, Fut>(&mut self, callback: F) -> CancellationToken
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
    {
        let token = CancellationToken::new();
        self.callbacks
            .push((token.clone(), Box::new(move || Box::pin(callback()))));
        token
    }

    /// Fire all registered callbacks once immediately when [`run`](Self::run) is called,
    /// before waiting for the first scheduled tick.
    pub fn fire_on_start(mut self) -> Self {
        self.fire_on_start = true;
        self
    }

    /// Returns a [`CancellationToken`] that, when cancelled, stops the scheduler
    /// at its next opportunity. Useful for triggering shutdown from outside [`run`](Self::run).
    ///
    /// ```rust,no_run
    /// # use lib_trigger::Scheduler;
    /// # use lib_schedule::time::SpecIteratorBuilder as TimeBuilder;
    /// # async fn example() {
    /// # let mut scheduler = Scheduler::new(
    /// #     TimeBuilder::new("1H:00:00", chrono::Utc).build().unwrap(),
    /// #     |_e: String| async {},
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
    /// The `func` closure has error handling already baked in and returns `()`.
    pub fn add_job(&mut self, func: fn() -> BoxedFuture) {
        let token = CancellationToken::new();
        self.callbacks.push((
            token,
            Box::new(move || {
                let fut = func();
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
    /// On each tick all non-cancelled callbacks are spawned as concurrent tasks.
    /// Errors are forwarded to `on_error`; the scheduler always advances to the next tick.
    /// If [`fire_on_start`](Self::fire_on_start) was called, all callbacks fire once immediately
    /// before waiting for the first scheduled tick.
    pub async fn run(mut self) {
        if self.fire_on_start {
            self.fire_callbacks().await;
        }

        loop {
            let Some(fire_at) = self.iter.next_fire() else {
                break;
            };

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

            self.fire_callbacks().await;
        }
    }

    async fn fire_callbacks(&self) {
        let handles: Vec<JoinHandle<()>> = self
            .callbacks
            .iter()
            .filter(|(token, _)| !token.is_cancelled())
            .map(|(token, cb)| {
                let fut = cb();
                let token = token.clone();
                let shutdown = self.shutdown.clone();
                let on_error = self.on_error.clone();
                tokio::spawn(async move {
                    tokio::select! {
                        _ = token.cancelled() => {}
                        _ = shutdown.cancelled() => {}
                        result = fut => {
                            if let Err(e) = result {
                                on_error(e).await;
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
