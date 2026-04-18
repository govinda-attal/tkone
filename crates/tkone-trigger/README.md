<!-- cargo-rdme start -->

In-memory schedule triggering built on top of [`tkone_schedule`].

[`Scheduler<I, E>`] owns a single [`ScheduleIter`] that fans out each tick
to N async callbacks. Each callback receives a [`FireContext`] carrying the
[`tkone_schedule::Occurrence`] for that tick, so handlers can inspect the
scheduled occurrence (actual vs. observed date, UTC fire time). Callbacks
return `Result<(), E>`; any error is routed to the async `on_error` handler,
which also receives the [`FireContext`]. Both `I` and `E` are fully generic
and inferred from the arguments to [`Scheduler::new`].

| Spec example | Meaning |
|---|---|
| `"1H:00:00"` | Every hour on the hour |
| `"HH:30M:00"` | Every 30 minutes |
| `"HH:MM:10S"` | Every 10 seconds |
| `"09:30:00"` | Daily at 09:30 |

**Only [`tkone_schedule::time`] specs are supported.** `date` and `datetime`
specs (daily, weekly, monthly) require persistent state to survive process
restarts — a missed tick on restart is silently lost. Use the `tempo` crate
for those schedules.

## Example

*Run `cargo run -p tkone-trigger --example process_payments` for a complete program.*

<!-- cargo-rdme end -->
