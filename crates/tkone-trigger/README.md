<!-- cargo-rdme start -->

In-memory schedule triggering built on top of [`tkone_schedule`].

[`Scheduler<I, E>`] owns a single [`ScheduleIter`] that fans out each tick
to N async callbacks. Callbacks return `Result<(), E>`; any error is routed
to the async `on_error` handler supplied at construction. Both `I` and `E`
are fully generic and inferred from the arguments to [`Scheduler::new`].

**Only [`tkone_schedule::time::SpecIterator`] is supported.** `tkone-trigger`
is an **in-memory, intra-day** trigger and does not persist state across
process restarts. `date` and `datetime` specs (daily, weekly, monthly) are
intentionally excluded — a missed tick on restart is silently lost, which is
unacceptable for those schedules. Use the `tempo` crate for persistent,
date-and-datetime-aware scheduling.

| Spec example | Meaning |
|---|---|
| `"1H:00:00"` | Every hour on the hour |
| `"HH:30M:00"` | Every 30 minutes |
| `"HH:MM:10S"` | Every 10 seconds |
| `"09:30:00"` | Daily at 09:30 |

## Example

*Run `cargo run -p tkone-trigger --example process_payments` for a complete program.*

<!-- cargo-rdme end -->
