//! A business-day job that survives restarts.
//!
//! An end-of-day reconciliation runs at 16:30 New York time on days the New
//! York Stock Exchange trades. The process was down for a week: on start it
//! finds the missed runs, fires the latest one once (`CatchUp::Latest`),
//! records it, and waits for the next trading day. Restarted again, it
//! resumes after the recorded run.
//!
//! ```text
//! cargo run -p tkone-trigger --example business_day_jobs
//! ```

use chrono::{Duration, TimeZone, Utc};
use tkone_calendar::Calendars;
use tkone_schedule::biz_day::BizDayProcessor;
use tkone_schedule::schedule::Schedule;
use tkone_trigger::{
    BusinessSchedule, CatchUp, FileStore, FireContext, ScheduleIter, Scheduler, StateStore, TickContext,
};

/// 16:30 on NYSE trading days, read in New York.
const RULE: &str = "YY-MM-DD~SBT16:30:00";

fn schedule(
    path: &std::path::Path,
) -> Result<BusinessSchedule<impl BizDayProcessor, FileStore>, tkone_schedule::Error> {
    let nyse = Calendars::builtin().get("XNYS").map_err(tkone_schedule::Error::from)?;
    let start = chrono_tz::America::New_York.with_ymd_and_hms(2026, 1, 2, 0, 0, 0).unwrap();
    let eod = Schedule::builder(start, nyse).rule(RULE).build()?;
    BusinessSchedule::new(eod, FileStore::new(path), "eod-reconciliation", CatchUp::Latest)
}

#[tokio::main]
async fn main() -> Result<(), tkone_schedule::Error> {
    let path = std::env::temp_dir().join("tkone-business-day-jobs.state");
    // Pretend the last run was a week ago, then the process went down.
    let ny = chrono_tz::America::New_York;
    let day = (Utc::now() - Duration::days(7)).with_timezone(&ny).date_naive();
    let week_ago = day.and_hms_opt(16, 30, 0).unwrap().and_local_timezone(ny).earliest().unwrap();
    FileStore::new(&path).save("eod-reconciliation", week_ago.fixed_offset())?;
    println!("{RULE} on XNYS, America/New_York\nlast recorded run: {week_ago}\n");

    let mut scheduler = Scheduler::new(schedule(&path)?, |ctx: FireContext, e: String| async move {
        eprintln!("reconciliation for {} failed: {e}", ctx.occurrence().actual());
    });
    scheduler.add(|ctx: FireContext| async move {
        let run = ctx.occurrence();
        println!(
            "reconciling the run due {} (fired now: it was missed)",
            run.observed().with_timezone(&chrono_tz::America::New_York)
        );
        Ok::<(), String>(())
    });
    let shutdown = scheduler.shutdown_token();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        shutdown.cancel();
    });
    scheduler.try_run().await?;

    // A restart resumes after the recorded run.
    println!("\nstate file: {}", std::fs::read_to_string(&path).unwrap_or_default().trim());
    if let Some(next) = schedule(&path)?.try_next_fire()? {
        println!("after a restart, the next run is {}", next.observed().with_timezone(&chrono_tz::America::New_York));
    }
    let _ = std::fs::remove_file(&path);
    Ok(())
}
