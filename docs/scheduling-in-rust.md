# Scheduling Jobs in Rust Without the Boilerplate

There is a recurring problem in backend services: something needs to happen on a schedule. Process payments at the top of every hour. Reconcile accounts every 30 minutes. Generate a report on the last business day of each month. Most languages reach for cron, a job queue, or a third-party platform. Rust has good async primitives, but wiring up a reliable, ergonomic in-process scheduler has historically meant a lot of boilerplate.

This post walks through three crates — **tkone-schedule**, **tkone-trigger**, and **tkone-trigger-macros** — that together take you from a raw recurrence spec to a fully declarative, signal-aware scheduler in a handful of lines.

---

## The Problem with Existing Approaches

Cron strings (`"0 * * * *"`) are compact but opaque and offer no concept of business days, fiscal calendars, or "last Friday of the month." Job queues are great for durability but introduce infrastructure complexity you do not always need. And writing `tokio::time::sleep` loops by hand is error-prone and scatters scheduling logic across a codebase.

What would a purpose-built Rust scheduling library look like if it started from the domain rather than from Unix cron?

---

## tkone-schedule: A Mini-Language for Recurrence

`tkone-schedule` defines a concise spec language for three schedule families: **date**, **time**, and **datetime** (date + time combined). Specs are strings parsed at startup; iteration is lazy and fallible.

### Time specs

A time spec describes intra-day recurrence using `<hours>:<minutes>:<seconds>`:

```text
"1H:00:00"      every hour, on the hour
"HH:30M:00"     every 30 minutes
"HH:MM:10S"     every 10 seconds
"09:30:00"      every day at 09:30 exactly
```

### Date specs

A date spec uses `<years>-<months>-<days>` with an optional business-day adjustment:

```text
"YY-MM-15"          every month on the 15th
"YY-1M-L"           last day of every month
"YY-MM-FRI#L"       last Friday of every month
"YY-3M-01"          first day of every quarter
"YY-1M-L~PB"        last day of the month, adjusted to previous business day
"YY-1M-L~NW"        last day of the month, adjusted to next weekday
```

### DateTime specs

Combine the two with a `T` separator:

```text
"YY-1M-L~NBT11:00:00"     last business day of the month at 11:00
"YY-MM-FRIT09:30:00"      every Friday at 09:30
"YY-MM-DDT1H:00:00"       every day, every hour
```

### Iterating

All three families expose a `SpecIteratorBuilder` that produces a `FallibleIterator`. Date and datetime iterators yield `Occurrence<DateTime<Tz>>`, which distinguishes the raw calendar date from the business-day-adjusted settlement date:

```rust
use tkone_schedule::biz_day::WeekendSkipper;
use tkone_schedule::datetime::SpecIteratorBuilder;
use fallible_iterator::FallibleIterator;
use chrono_tz::Europe::London;
use chrono::{SubsecRound, Utc};

let bdp = WeekendSkipper::new();
let now = Utc::now().with_timezone(&London).trunc_subsecs(0);

// Find the first occurrence strictly after now, then iterate from it
let start = SpecIteratorBuilder::new_after("YY-1M-L~NBT11:00:00", bdp.clone(), now)
    .build().unwrap()
    .next().unwrap().unwrap()
    .observed().clone();

let mut iter = SpecIteratorBuilder::new_with_start("YY-1M-L~NBT11:00:00", bdp, start)
    .build().unwrap();

while let Some(nr) = iter.next().unwrap() {
    println!("fire at {} (raw: {})", nr.observed(), nr.actual());
}
```

`Occurrence::observed()` gives the settlement date; `Occurrence::actual()` gives the raw calendar date before any business-day rule was applied.

### Where it shines

- Financial schedules: end-of-month settlement, T+2 clearing windows, quarterly rebalancing
- Timezone-aware recurrences with correct DST handling (spring-forward and fall-back)
- Business day arithmetic without an external calendar service
- Any domain where "last Friday" or "2nd Wednesday" is a real requirement

---

## tkone-trigger: Turning Iterators into Async Callbacks

`tkone-schedule` tells you *when* to fire. `tkone-trigger` does the firing.

`Scheduler<I, E>` accepts a [`tkone_schedule::time::SpecIterator`] — the only iterator type supported — and fans each tick out to N registered async callbacks. Every callback and the `on_error` handler receive a `FireContext` carrying the `Occurrence` for that tick. Callbacks return `Result<(), E>`; any error is forwarded to `on_error` and the scheduler continues to the next tick regardless.

> **Time specs only.** `tkone-trigger` is an **in-memory, intra-day** trigger. `date` and `datetime` specs are intentionally excluded: a missed tick on process restart is silently lost, which is unacceptable for daily/weekly/monthly schedules. Use the `tempo` crate for persistent, date-and-datetime-aware scheduling.

### FireContext and TickContext

```rust
pub trait TickContext {
    fn occurrence(&self) -> &Occurrence<DateTime<Utc>>;
}
```

`FireContext` is the concrete type passed at runtime. It implements `TickContext`, so handler signatures can be written against the trait for easier testing and mocking. The `Occurrence` it carries is always `Exact` for time specs — no business-day adjustment is applied. The full enum has three variants (used by `date`/`datetime` iterators in `tkone-schedule` and `tempo`):

| Variant | Meaning |
|---------|---------|
| `Exact(t)` | No adjustment; `actual == observed == t` |
| `AdjustedLater(a, o)` | Settlement moved later than the raw date |
| `AdjustedEarlier(a, o)` | Settlement moved earlier than the raw date |

### Imperative API

```rust
use tkone_schedule::time::SpecIteratorBuilder as TimeBuilder;
use tkone_trigger::{FireContext, Scheduler, TickContext};
use thiserror::Error;

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("firing at {:?}", ctx.occurrence().observed());
    // ... call payment service
    Ok(())
}

async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    // ... reconciliation logic
    Ok(())
}

async fn on_error(ctx: FireContext, e: PaymentError) {
    eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
}

#[tokio::main]
async fn main() {
    let mut scheduler = Scheduler::new(
        TimeBuilder::new("1H:00:00", chrono::Utc).build().unwrap(),
        on_error,
    )
    .fire_on_start();                    // run all jobs once immediately

    scheduler.add(process_payments);
    scheduler.add(reconcile_accounts);

    let shutdown = scheduler.shutdown_token();
    tokio::select! {
        _ = scheduler.run() => {}
        _ = tokio::signal::ctrl_c() => { shutdown.cancel(); }
    }
}
```

Key design points:

- **Generic error type** — `E` is inferred from the `on_error` handler; no `Box<dyn Error>` required.
- **FireContext** — every callback and error handler receives the tick's `Occurrence`; for time specs it is always `Occurrence::Exact`.
- **Per-callback cancellation** — `add()` returns a `CancellationToken` that silences only that callback without stopping others or the iterator.
- **Global shutdown** — `shutdown_token()` clones the internal token before `run()` consumes `self`, enabling the `tokio::select!` pattern.
- **`fire_on_start`** — runs all callbacks once immediately before waiting for the first scheduled tick; avoids the "we just deployed, do we wait an hour?" problem.
- **Concurrent fan-out** — all non-cancelled callbacks are spawned as independent Tokio tasks on each tick.

### Where it shines

- Long-running services that need intra-day recurrence (every N seconds/minutes/hours)
- Multiple logically related jobs sharing one schedule (process + reconcile)
- Services that need clean shutdown on SIGTERM without a job-queue dependency

> **In-memory caveat:** `tkone-trigger` does not persist state. If the process restarts between ticks, those ticks are missed. For intra-day recurrences this is usually acceptable; for daily/weekly/monthly schedules use `tempo`.

---

## tkone-trigger-macros: Declarative Scheduling with Zero Wiring

The imperative `Scheduler` API is explicit and flexible, but for typical services it produces the same boilerplate every time: build an iterator, construct a scheduler, call `add` for every job, wire up a shutdown token. `tkone-trigger-macros` eliminates all of it.

Two attribute macros do the work:

| Macro | Applied to | What it does |
|-------|-----------|-------------|
| `#[schedule]` | `impl` block | Generates `run()`, `run_until_signal()`, `shutdown_token()` on the struct |
| `#[job(Struct)]` | `async fn` | Registers the function as a job on the named scheduler at link time |

### Define a scheduler

```rust
use tkone_trigger::{FireContext, TickContext};
use tkone_trigger_macros::schedule;
use thiserror::Error;

#[derive(Debug, Error)]
enum PaymentError {
    #[error("downstream unavailable: {0}")]
    Downstream(String),
}

struct PaymentSchedule;

#[schedule(spec = "1H:00:00", fire_on_start)]
impl PaymentSchedule {
    #[on_error]
    async fn on_error(ctx: FireContext, e: PaymentError) {
        eprintln!("job failed at {:?}: {e}", ctx.occurrence().observed());
    }
}
```

The `#[on_error]` method's second parameter type — `PaymentError` — is the single source of truth for the error type across all jobs. No type parameter annotation needed anywhere. The first parameter is always `FireContext`, giving the error handler access to the tick's occurrence.

### Register jobs

Jobs can live anywhere in the codebase. Each one names the scheduler struct it belongs to:

```rust
use tkone_trigger::{FireContext, TickContext};
use tkone_trigger_macros::job;

#[job(PaymentSchedule)]
async fn process_payments(ctx: FireContext) -> Result<(), PaymentError> {
    println!("firing at {:?}", ctx.occurrence().observed());
    // call payment service
    Ok(())
}

#[job(PaymentSchedule)]
async fn reconcile_accounts(ctx: FireContext) -> Result<(), PaymentError> {
    // reconciliation logic
    Ok(())
}
```

Registration happens at link time via the [`inventory`](https://docs.rs/inventory) crate — the same technique used by Serde's remote impls and `clap`'s derive macros. No central registry, no `add` calls, no `main`-level wiring.

### Run

```rust
#[tokio::main]
async fn main() {
    // Blocks until Ctrl-C, SIGTERM, or the iterator is exhausted
    PaymentSchedule::run_until_signal().await;
}
```

Or, for more control:

```rust
#[tokio::main]
async fn main() {
    let shutdown = PaymentSchedule::shutdown_token();
    tokio::select! {
        _ = PaymentSchedule::run() => {}
        _ = tokio::time::sleep(Duration::from_secs(3600)) => {
            shutdown.cancel();
        }
    }
}
```

### What the macro generates

`#[schedule]` expands to roughly:

```rust
impl PaymentSchedule {
    pub fn shutdown_token() -> CancellationToken { /* OnceLock<CancellationToken> */ }

    pub async fn run() {
        let iter = SpecIteratorBuilder::new("1H:00:00", Utc).build().unwrap();
        let mut scheduler = Scheduler::new(iter, Self::on_error)
            .with_shutdown_token(Self::shutdown_token())
            .fire_on_start();
        // collect all #[job(PaymentSchedule)] entries from the inventory
        for entry in inventory::iter::<JobEntry>() {
            if entry.schedule_type_id == TypeId::of::<PaymentSchedule>() {
                scheduler.add_job(entry.func);
            }
        }
        scheduler.run().await;
    }

    pub async fn run_until_signal() {
        tokio::select! {
            _ = Self::run() => {}
            _ = tokio::signal::ctrl_c() => { Self::shutdown_token().cancel(); }
        }
    }
}
```

`#[job(PaymentSchedule)]` expands to the original function plus a named helper that wraps it: on `Err(e)`, the helper calls `PaymentSchedule::handle_error(ctx, e)` (the `ScheduleErrorHandler` impl generated by `#[schedule]`), then registers a `JobEntry` via `inventory::submit!` using a plain function pointer — a zero-cost `#[used]` static, not a heap allocation at startup.

### Where it shines

- Services with many scheduled jobs spread across modules
- Teams that want scheduling to read like a declaration, not an imperative wiring step
- Codebases where `main` should stay clean and job registration should be co-located with job logic
- Anywhere the imperative API works, but you want fewer lines and less ceremony

---

## The Full Stack at a Glance

```
tkone-schedule          → parse "YY-1M-L~NBT11:00:00", iterate fire times
      ↓
tkone-trigger           → Scheduler<I, E>: sleep → fan-out → error handler
      ↓
tkone-trigger-macros    → #[schedule] / #[job]: zero-boilerplate wiring
```

Each layer is independently usable. If you need fine-grained control — multiple schedulers in one process, different error types per group of jobs, conditional callback cancellation — use `tkone-trigger` directly. If you want the simplest possible path from "I have a spec and some async functions" to a running scheduler, reach for the macros.

---

## Trying It Out

```bash
# Imperative style
cargo run -p example-app --bin imperative

# Declarative macro style
cargo run -p example-app --bin declarative
```

Both examples run a two-job scheduler (one that always fails, one that always succeeds) for 25 seconds, printing output on every tick so you can see the error routing in action.

---

*Built with Rust, Tokio, and an unreasonable fondness for expressive spec languages.*
