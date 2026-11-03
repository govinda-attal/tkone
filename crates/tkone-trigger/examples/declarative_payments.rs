//! The same two jobs, declared with `tkone-trigger-macros`: a scheduler
//! struct with `#[schedule]` and `#[on_error]`, and jobs registered with
//! `#[job(…)]`. `start_spec = "HH:MM:00"` starts at the next top of the
//! minute; `fire_on_start` runs the jobs once straight away.
//!
//! ```text
//! cargo run -p tkone-trigger --example declarative_payments
//! ```

use thiserror::Error;
use tkone_trigger::{FireContext, TickContext};
use tkone_trigger_macros::{job, schedule};

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

struct PaymentSchedule;

/// Every 10 seconds from the next top of the minute, and once on start.
#[schedule(spec = "HH:MM:10S", start_spec = "HH:MM:00", fire_on_start)]
impl PaymentSchedule {
    #[on_error]
    async fn on_error(ctx: FireContext, e: PaymentError) {
        eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
    }
}

#[job(PaymentSchedule)]
async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("process_payments fired at {:?}", ctx.occurrence().observed());
    Err(PaymentError::Downstream("payment service is down".to_string()))
}

#[job(PaymentSchedule)]
async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    println!("reconciling accounts, fired at {:?}", ctx.occurrence().observed());
    Ok(())
}

#[tokio::main]
async fn main() {
    println!("schedule: {}", PaymentSchedule::describe());
    let shutdown = PaymentSchedule::shutdown_token();
    tokio::select! {
        _ = PaymentSchedule::run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("scheduler stopped");
}
