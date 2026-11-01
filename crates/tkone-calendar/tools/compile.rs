//! Compiles `data/*.yaml` into `compiled/calendars.json`, which the crate
//! embeds, after checking every calendar.
//!
//! ```text
//! cargo run -p tkone-calendar --features yaml --bin tkone-calendar-compile            # write
//! cargo run -p tkone-calendar --features yaml --bin tkone-calendar-compile -- --check # CI
//! ```

use std::path::Path;
use std::process::ExitCode;

use tkone_calendar::{read_dir, Calendars};

fn main() -> ExitCode {
    let check = std::env::args().any(|a| a == "--check");
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = crate_dir.join("compiled/calendars.json");

    let files = match read_dir(&crate_dir.join("data")) {
        Ok(files) => files,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let calendars = match Calendars::from_files(&files) {
        Ok(calendars) => calendars,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    let json = serde_json::to_string_pretty(&files).expect("calendar files serialize") + "\n";
    let current = std::fs::read_to_string(&out).unwrap_or_default();
    let codes: Vec<&str> = calendars.codes().collect();
    if check {
        if current != json {
            eprintln!("compiled/calendars.json is out of date; run the compile tool without --check");
            return ExitCode::FAILURE;
        }
        println!("{} calendars checked, compiled data up to date: {}", codes.len(), codes.join(", "));
    } else if current != json {
        if let Err(e) = std::fs::write(&out, json) {
            eprintln!("{}: {e}", out.display());
            return ExitCode::FAILURE;
        }
        println!("wrote compiled/calendars.json: {}", codes.join(", "));
    } else {
        println!("compiled/calendars.json is up to date: {}", codes.join(", "));
    }
    ExitCode::SUCCESS
}
