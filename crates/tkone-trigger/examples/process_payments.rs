//! A scheduler built directly: two jobs every 10 seconds, from the next
//! top of the minute, for 25 seconds. One job always fails, to show errors
//! reaching `on_error`; the other always succeeds.
//!
//! The start is set on the iterator, not the `Scheduler`:
//! `resolve_start_spec` takes an RFC 3339 datetime (`"2026-06-01T09:00:00Z"`)
//! or a time spec meaning its next occurrence (`"HH:MM:00"`, the next top of
//! the minute), and fails on anything else rather than start from now.
//! `time_schedule(spec, start_spec, tz)` does the same in one call and reads
//! the schedule in a time zone.
//!
//! ```text
//! cargo run -p tkone-trigger --example process_payments
//! ```

use thiserror::Error;
use tkone_schedule::time::SpecIteratorBuilder as TimeBuilder;
use tkone_trigger::{resolve_start_spec, FireContext, Scheduler, TickContext};

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("process_payments fired at {:?}", ctx.occurrence().observed());
    Err(PaymentError::Downstream("payment service is down".to_string()))
}

async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    println!("reconciling accounts, fired at {:?}", ctx.occurrence().observed());
    Ok(())
}

async fn on_error(ctx: FireContext, e: PaymentError) {
    eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
}

#[tokio::main]
async fn main() {
    // From the next :00 seconds, then every 10 seconds.
    let start = resolve_start_spec("HH:MM:00").expect("a valid start spec");
    let iter = TimeBuilder::new_after("HH:MM:10S", start).build().unwrap();

    let mut scheduler = Scheduler::new(iter, on_error).fire_on_start();
    scheduler.add(process_payments);
    scheduler.add(reconcile_accounts);

    let shutdown = scheduler.shutdown_token();
    tokio::select! {
        _ = scheduler.run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("scheduler stopped");
}
