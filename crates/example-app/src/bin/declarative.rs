//! Declarative scheduling example using [`tkone_trigger_macros`].
//!
//! Demonstrates:
//! - Defining a scheduler struct with `#[schedule]` and `#[on_error]`
//! - Registering jobs with `#[job(SchedulerStruct)]`
//! - `fire_on_start` flag in the `#[schedule]` attribute
//! - Shutdown via the generated `shutdown_token()` or `run_until_signal()`

use tkone_trigger::{FireContext, TickContext};
use tkone_trigger_macros::{job, schedule};
use thiserror::Error;

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

struct PaymentSchedule;

/// Fires every 10 seconds; all registered jobs are called immediately on start.
#[schedule(spec = "HH:MM:10S", fire_on_start)]
impl PaymentSchedule {
    #[on_error]
    async fn on_error(ctx: FireContext, e: PaymentError) {
        eprintln!("[declarative] job failed at {:?}: {e}", ctx.occurrence().observed());
    }
}

#[job(PaymentSchedule)]
async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("[declarative] process_payments fired at {:?}", ctx.occurrence().observed());
    Err(PaymentError::Downstream("payment service is down".to_string()))
}

#[job(PaymentSchedule)]
async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    println!("[declarative] reconciling accounts, fired at {:?}", ctx.occurrence().observed());
    Ok(())
}

#[tokio::main]
async fn main() {
    let shutdown = PaymentSchedule::shutdown_token();
    tokio::select! {
        // Run until the iterator is exhausted or shutdown is requested.
        _ = PaymentSchedule::run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("[declarative] 25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("[declarative] scheduler stopped");
}
