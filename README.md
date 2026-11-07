# tkone

A growing Rust toolkit of libraries and applications across multiple domains. Currently includes a scheduling ecosystem — from simple in-memory time-based triggers to enterprise-grade distributed scheduling with guaranteed delivery. More crates and features are in progress.

## Crates

| Crate | Description |
|-------|-------------|
| [tkone-schedule](crates/tkone-schedule/README.md) | Core recurrence library with a mini-language for date, time, and combined datetime specs |
| [tkone-calendar](crates/tkone-calendar/README.md) | Holiday calendars (London, TARGET2, New York, NYSE, …) as business-day processors for tkone-schedule |
| [Python bindings](bindings/python/README.md) | `pip install ./bindings/python`: `tkone.schedule` and `tkone.calendar` in Python, one module per crate ([conventions](bindings/README.md)) |
| [tkone-trigger](crates/tkone-trigger/README.md) | Async scheduler on tkone-schedule: in-memory time triggers, and business-day jobs that resume from stored state |
| [tkone-trigger-macros](crates/tkone-trigger-macros/README.md) | Declarative `#[schedule]` / `#[job]` attribute macros for zero-boilerplate wiring |
| [tkone-tempo](crates/tkone-tempo/README.md) | Enterprise distributed scheduler with PostgreSQL persistence and transactional messaging |
| [example-app](crates/example-app/src/) | Use cases that combine the crates, on real holiday calendars |

## Examples

Each crate's `examples/` show its own API; `example-app` holds use cases
that combine crates. A new example goes in the crate it demonstrates, or in
`example-app` if it needs several.

| Run | Shows |
|-----|-------|
| `cargo run -p tkone-schedule --example date_recurrence` | dates with a business-day adjustment, from the next one |
| `cargo run -p tkone-schedule --example time_recurrence` | every 30 minutes from a start; hourly until an end |
| `cargo run -p tkone-schedule --example datetime_recurrence` | the last business day of each month at 11:00 London time |
| `cargo run -p tkone-trigger --example process_payments` | a scheduler built directly, with error routing |
| `cargo run -p tkone-trigger --example declarative_payments` | the same with `#[schedule]` and `#[job]` |
| `cargo run -p tkone-trigger --example business_day_jobs` | a NYSE end-of-day job that survives restarts |
| `cargo run -p example-app --bin loan-repayments` | loan servicing: collections on London + New York business days |
| `cargo run -p example-app --bin exchange-jobs` | jobs on NYSE trading days; the wrong calendar refused |
| `cargo run -p example-app --bin payroll` | one payroll template on Sydney's and Auckland's calendars |
| `cargo run -p example-app --bin store-deliveries` | deliveries skipping Johannesburg holidays |

## Documentation

- [Scheduling in Rust](docs/scheduling-in-rust.md) — patterns and design rationale
- [Schedule documents](docs/schedule-document.md) — rule strings inside YAML/JSON schedules; definitions and instances (proposed)
- [Outbox / Inbox crate design](docs/outbox-inbox-design.md) — `tkone-outbox` and `tkone-inbox` design (in-progress)
- [Date spec](crates/tkone-schedule/src/date/date-spec.md) · [Time spec](crates/tkone-schedule/src/time/time-spec.md) · [Datetime spec](crates/tkone-schedule/src/datetime/date-time-spec.md) — mini-language references
- [Tempo architecture](crates/tkone-tempo/docs/components.md) · [ERD](crates/tkone-tempo/docs/erd.md) · [Sequences](crates/tkone-tempo/docs/sequences.md)
- [Kubernetes deployment](crates/tkone-tempo/docs/kubernetes.md) · [Docker Compose](crates/tkone-tempo/docs/docker-compose.md)

## License

Licensed under the [Apache License, Version 2.0](LICENSE). See [NOTICE](NOTICE) for copyright.
