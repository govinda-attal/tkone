<!-- cargo-rdme start -->

In-memory schedule triggering built on top of [`lib_schedule`].

[`Scheduler<I, E>`] owns a single [`ScheduleIter`] that fans out each tick
to N async callbacks. Callbacks return `Result<(), E>`; any error is routed
to the async `on_error` handler supplied at construction. Both `I` and `E`
are fully generic and inferred from the arguments to [`Scheduler::new`].

| Iterator type | Spec example |
|---------------|--------------|
| [`lib_schedule::time::SpecIterator`] | `"1H:00:00"` |
| [`lib_schedule::datetime::SpecIterator`] | `"YY-1M-L~WT11:00:00"` |
| [`lib_schedule::date::SpecIterator`] | `"YY-MM-FRI#L"` |

> **Note:** Although `datetime` and `date` iterators are supported, this
> crate is an **in-memory** trigger — state is not persisted across process
> restarts. Long-distance recurrences (daily, weekly, monthly) will be
> missed if the process is restarted between ticks. For those schedules
> consider pairing this crate with an external state store or using a
> persistent job queue. `time`-based intra-day recurrences (every N
> seconds/minutes/hours within a single day) are the primary intended use.

## Example

*Run `cargo run -p lib-trigger --example process_payments` for a complete program.*

<!-- cargo-rdme end -->
