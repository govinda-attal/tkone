# tkone

A growing Rust toolkit of libraries and applications across multiple domains. Currently includes a scheduling ecosystem — from simple in-memory time-based triggers to enterprise-grade distributed scheduling with guaranteed delivery. More crates and features are in progress.

## Crates

| Crate | Description |
|-------|-------------|
| [tkone-schedule](crates/tkone-schedule/README.md) | Core recurrence library with a mini-language for date, time, and combined datetime specs |
| [tkone-calendar](crates/tkone-calendar/README.md) | Holiday calendars (London, TARGET2, New York, NYSE, …) as business-day processors for tkone-schedule |
| [Python bindings](bindings/python/README.md) | `pip install ./bindings/python`: `tkone.schedule` and `tkone.calendar` in Python, one module per crate ([conventions](bindings/README.md)) |
| [tkone-trigger](crates/tkone-trigger/README.md) | In-memory scheduler built on tkone-schedule; fans out each tick to async callbacks |
| [tkone-trigger-macros](crates/tkone-trigger-macros/README.md) | Declarative `#[schedule]` / `#[job]` attribute macros for zero-boilerplate wiring |
| [tkone-tempo](crates/tkone-tempo/README.md) | Enterprise distributed scheduler with PostgreSQL persistence and transactional messaging |
| [example-app](crates/example-app/src/) | Runnable examples for all crates, including use cases on real holiday calendars: loan servicing, exchange jobs, payroll, store deliveries |

## Documentation

- [Scheduling in Rust](docs/scheduling-in-rust.md) — patterns and design rationale
- [Outbox / Inbox crate design](docs/outbox-inbox-design.md) — `tkone-outbox` and `tkone-inbox` design (in-progress)
- [Date spec](crates/tkone-schedule/src/date/date-spec.md) · [Time spec](crates/tkone-schedule/src/time/time-spec.md) · [Datetime spec](crates/tkone-schedule/src/datetime/date-time-spec.md) — mini-language references
- [Tempo architecture](crates/tempo/docs/components.md) · [ERD](crates/tempo/docs/erd.md) · [Sequences](crates/tempo/docs/sequences.md)
- [Kubernetes deployment](crates/tempo/docs/kubernetes.md) · [Docker Compose](crates/tempo/docs/docker-compose.md)

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for copyright.
