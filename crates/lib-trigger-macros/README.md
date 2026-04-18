<!-- cargo-rdme start -->

Declarative scheduling macros built on top of [`lib_trigger`].

This crate provides two attribute macros that eliminate the boilerplate
of wiring up [`lib_trigger::Scheduler`] by hand.

| Macro | Applied to | Purpose |
|-------|-----------|---------|
| [`#[schedule]`](macro@schedule) | `impl` block | Turn a plain struct into a scheduler entry point |
| [`#[job]`](macro@job) | `async fn` | Register a function as a job on a named scheduler |

## Quick start

### 1 — Define a scheduler struct

Apply `#[schedule]` to an `impl` block. The block must contain exactly one
method marked `#[on_error]`; its parameter type becomes the shared error
type `E` for all jobs attached to this scheduler.

```rust
use lib_trigger_macros::schedule;
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
    async fn on_error(e: AppError) {
        eprintln!("job failed: {e}");
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
| `spec = "..."` | yes | [`lib_schedule`] time spec, e.g. `"1H:00:00"` |
| `fire_on_start` | no | Fire all jobs once immediately before the first tick |

### 2 — Register jobs

Apply `#[job(SchedulerStruct)]` to any `async fn` that returns
`Result<(), E>`. The error type must match the one inferred from
`#[on_error]`.

```rust
use lib_trigger_macros::job;

#[job(MySchedule)]
async fn do_work() -> Result<(), AppError> {
    println!("tick");
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
