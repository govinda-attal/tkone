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
| `"[00-11,14-23]:30M:00"` | Every 30 minutes, except 12:00–13:59 |
| `"09-16:15M:00"` | Every 15 minutes from 09:00 to 16:45 |

## Time zones and starts

[`time_schedule`] builds the iterator for a spec. Times are read on the
wall clock of a time zone: the `tz` you pass, else UTC. Steps are aligned to
the clock, as cron's are: `HH:15M:00` fires at :00, :15, :30 and :45 whenever
the process starts. A `start_spec` ([`resolve_start_spec_in`]) sets from when
on times are produced. Past occurrences are never fired.

## Business-day schedules

A time spec repeats within the day and simply starts again from now after
a restart. A schedule that runs once a day or less, such as "16:30 on New
York business days" (`YY-MM-DD~SBT16:30:00` on the `USNY` calendar, in
`America/New_York`), must not lose a run to a restart. [`BusinessSchedule`]
runs such a `Schedule` on a holiday calendar (any
[`BizDayProcessor`](tkone_schedule::biz_day::BizDayProcessor), such as a
`tkone-calendar` `Calendar`), records each run it fires in a
[`StateStore`] ([`MemoryStore`], [`FileStore`], or your own), and resumes
after the last one when rebuilt. Runs missed while it was down follow its
[`CatchUp`] policy. Delivery is at least once.

*Run `cargo run -p tkone-trigger --example business_day_jobs` for a complete program.*

## Example

*Run `cargo run -p tkone-trigger --example process_payments` for a complete program.*

<!-- cargo-rdme end -->
