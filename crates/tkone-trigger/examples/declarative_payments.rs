use tkone_trigger_macros::{job, schedule};
use thiserror::Error;

#[derive(Debug, Error)]
enum JobError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

struct PaymentSchedule;

#[schedule(spec = "HH:MM:10S", fire_on_start)]
impl PaymentSchedule {
    #[on_error]
    async fn on_error(e: JobError) {
        eprintln!("scheduled job failed: {e}");
    }
}

#[job(PaymentSchedule)]
async fn process_payments() -> Result<(), JobError> {
    Err(JobError::Downstream("payment service is down".to_string()))
}

#[job(PaymentSchedule)]
async fn reconcile_accounts() -> Result<(), JobError> {
    println!("reconciling accounts");
    Ok(())
}

#[tokio::main]
async fn main() {
    let shutdown = PaymentSchedule::shutdown_token();
    tokio::select! {
        _ = PaymentSchedule::run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("Scheduler has been shut down");
}
