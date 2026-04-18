use tkone_schedule::time::SpecIteratorBuilder as TimeBuilder;
use tkone_trigger::{FireContext, TickContext, Scheduler};
use thiserror::Error;

#[derive(Debug, Error)]
enum JobError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments(ctx: FireContext) -> Result<(), JobError> {
    println!("process_payments fired at {:?}", ctx.occurrence().observed());
    Err(JobError::Downstream("payment service is down".to_string()))
}

async fn on_error(ctx: FireContext, e: JobError) {
    eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
}

#[tokio::main]
async fn main() {
    let mut scheduler = Scheduler::new(
        TimeBuilder::new("HH:MM:10S", chrono::Utc).build().unwrap(),
        on_error,
    ).fire_on_start();

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
