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

## Example

*Run `cargo run -p lib-trigger --example process_payments` for a complete program.*

<!-- cargo-rdme end -->
