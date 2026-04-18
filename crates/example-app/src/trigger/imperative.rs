//! Imperative scheduling example using [`tkone_trigger::Scheduler`] directly.
//!
//! Demonstrates:
//! - Building a time-based iterator with [`tkone_schedule::time::SpecIteratorBuilder`]
//! - Registering multiple async callbacks with [`Scheduler::add`]
//! - `fire_on_start` to execute jobs immediately before the first tick
//! - External shutdown via [`Scheduler::shutdown_token`]
//! - Using [`tkone_trigger::resolve_start_spec`] to control where iteration begins
//!
//! # start_spec in the imperative API
//!
//! In the declarative API (`#[schedule(start_spec = "...")]`) the start point is
//! expressed as a macro attribute and the generated `run()` handles it automatically.
//!
//! In the imperative API the start point is set **on the iterator**, not on the
//! `Scheduler`.  [`resolve_start_spec`] accepts either an RFC 3339 datetime string
//! or a tkone-schedule time spec and returns a `DateTime<Utc>` that can be passed
//! directly to [`SpecIteratorBuilder::new_after`]:
//!
//! ```rust,no_run
//! // Start from the next top-of-minute (spec-based start)
//! let start = tkone_trigger::resolve_start_spec("HH:MM:00");
//! let iter  = TimeBuilder::new_after("HH:MM:10S", start).build().unwrap();
//!
//! // Start from a fixed wall-clock moment (RFC 3339 start)
//! let start = tkone_trigger::resolve_start_spec("2026-06-01T09:00:00Z");
//! let iter  = TimeBuilder::new_after("HH:MM:10S", start).build().unwrap();
//! ```
//!
//! Omitting `resolve_start_spec` and using `TimeBuilder::new(spec, Utc)` is
//! equivalent to `start_spec = Utc::now()` — iteration begins from the current
//! instant, which is the default when no start is specified.

use tkone_schedule::time::SpecIteratorBuilder as TimeBuilder;
use tkone_trigger::{resolve_start_spec, FireContext, Scheduler, TickContext};
use thiserror::Error;

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("[imperative] process_payments fired at {:?}", ctx.occurrence().observed());
    Err(PaymentError::Downstream("payment service is down".to_string()))
}

async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    println!("[imperative] reconciling accounts, fired at {:?}", ctx.occurrence().observed());
    Ok(())
}

async fn on_error(ctx: FireContext, e: PaymentError) {
    eprintln!("[imperative] job failed at {:?}: {e}", ctx.occurrence().observed());
}

#[tokio::main]
async fn main() {
    // Start from the next top-of-minute, then fire every 10 seconds.
    // resolve_start_spec("HH:MM:00") treats the argument as a tkone-schedule
    // time spec and returns the first occurrence strictly after now — i.e. the
    // next :00 seconds boundary.  Pass that DateTime to new_after so the
    // iterator skips the partial minute we're already in.
    let start = resolve_start_spec("HH:MM:00");
    let iter  = TimeBuilder::new_after("HH:MM:10S", start).build().unwrap();

    let mut scheduler = Scheduler::new(iter, on_error).fire_on_start();

    scheduler.add(process_payments);
    scheduler.add(reconcile_accounts);

    let shutdown = scheduler.shutdown_token();
    tokio::select! {
        _ = scheduler.run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("[imperative] 25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("[imperative] scheduler stopped");
}
