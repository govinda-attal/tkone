<!-- cargo-rdme start -->

Declarative scheduling macros built on top of [`tkone_trigger`].

This crate provides two attribute macros that eliminate the boilerplate
of wiring up [`tkone_trigger::Scheduler`] by hand.

| Macro | Applied to | Purpose |
|-------|-----------|---------|
| [`#[schedule]`](macro@schedule) | `impl` block | Turn a plain struct into a scheduler entry point |
| [`#[job]`](macro@job) | `async fn` | Register a function as a job on a named scheduler |

Every callback and error handler receives a [`tkone_trigger::FireContext`]
carrying the [`tkone_schedule::Occurrence`] for that tick, so handlers can
inspect the scheduled occurrence.

## Quick start

### 1 — Define a scheduler struct

Apply `#[schedule]` to an `impl` block. The block must contain exactly one
method marked `#[on_error]`. That method takes `(ctx: FireContext, e: ErrorType)`;
the error type becomes the shared `E` for all jobs on this scheduler.

```rust
use tkone_trigger::FireContext;
use tkone_trigger_macros::schedule;
use thiserror::Error;

#[derive(Debug, Error)]
enum AppError {
    #[error("{0}")]
    Msg(String),
}

struct MySchedule;

#[schedule(spec = "1H:00:00")]
impl MySchedule {
    #[on_error]
    async fn on_error(ctx: FireContext, e: AppError) {
        eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
    }
}
```

`#[schedule]` generates three associated functions on `MySchedule`:

```text
fn  shutdown_token() -> CancellationToken
async fn run()
async fn run_until_signal()   // stops on Ctrl-C / SIGTERM
```

#### Attribute arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `spec = "..."` | yes | [`tkone_schedule`] time spec, e.g. `"1H:00:00"` |
| `start_spec = "..."` | no | Where to start iterating. RFC 3339 datetime (`"2026-06-01T09:00:00Z"`) **or** a tkone-schedule time spec (`"09:00:00"`). When omitted the scheduler starts from `Utc::now()`. |
| `fire_on_start` | no | Fire all jobs once immediately before the first tick |

### 2 — Register jobs

Apply `#[job(SchedulerStruct)]` to any `async fn` that takes
`ctx: FireContext` and returns `Result<(), E>`.

```rust
use tkone_trigger::FireContext;
use tkone_trigger_macros::job;

#[job(MySchedule)]
async fn do_work(ctx: FireContext) -> Result<(), AppError> {
    println!("tick at {:?}", ctx.occurrence().observed());
    Ok(())
}
```

Jobs are registered at link time via [`inventory`](https://docs.rs/inventory);
no explicit `add` calls are needed.

### 3 — Run

```rust
// Run until iterator exhausted or shutdown_token cancelled:
MySchedule::run().await;

// Run until the above OR Ctrl-C / SIGTERM:
MySchedule::run_until_signal().await;
```

## Complete example

*Run `cargo run -p example-app --bin declarative` for the full program.*

<!-- cargo-rdme end -->
