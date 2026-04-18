//! Imperative scheduling example using [`lib_trigger::Scheduler`] directly.
//!
//! Demonstrates:
//! - Building a time-based iterator with [`lib_schedule::time::SpecIteratorBuilder`]
//! - Registering multiple async callbacks with [`Scheduler::add`]
//! - `fire_on_start` to execute jobs immediately before the first tick
//! - External shutdown via [`Scheduler::shutdown_token`]

use lib_schedule::time::SpecIteratorBuilder as TimeBuilder;
use lib_trigger::Scheduler;
use thiserror::Error;

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments() -> Result<(), PaymentError> {
    Err(PaymentError::Downstream("payment service is down".to_string()))
}

async fn reconcile_accounts() -> Result<(), PaymentError> {
    println!("[imperative] reconciling accounts");
    Ok(())
}

async fn on_error(e: PaymentError) {
    eprintln!("[imperative] job failed: {e}");
}

#[tokio::main]
async fn main() {
    // Fire every 10 seconds; run for 25 seconds then shut down.
    let mut scheduler = Scheduler::new(
        TimeBuilder::new("HH:MM:10S", chrono::Utc).build().unwrap(),
        on_error,
    )
    .fire_on_start();

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
