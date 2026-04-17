use lib_schedule::time::SpecIteratorBuilder as TimeBuilder;
use lib_trigger::Scheduler;
use thiserror::Error;

#[derive(Debug, Error)]
enum JobError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments() -> Result<(), JobError> {
    Err(JobError::Downstream("payment service is down".to_string()))
}

async fn on_error(e: JobError) {
    eprintln!("scheduled job failed: {e}");
}

#[tokio::main]
async fn main() {
    let mut scheduler = Scheduler::new(
        TimeBuilder::new("HH:MM:10S", chrono::Utc).build().unwrap(),
        on_error,
    );

    scheduler.add(process_payments);

    let shutdown = scheduler.shutdown_token();
    tokio::select! {
        _ = scheduler.run() => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(25)) => {
            println!("25s elapsed — shutting down");
            shutdown.cancel();
        }
    }
    println!("Scheduler has been shut down");
}
