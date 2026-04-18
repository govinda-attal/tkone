# tkone

A growing Rust toolkit of libraries and applications across multiple domains. Currently includes a scheduling ecosystem — from simple in-memory time-based triggers to enterprise-grade distributed scheduling with guaranteed delivery. More crates and features are in progress.

## Crates

| Crate | Description |
|-------|-------------|
| [tkone-schedule](crates/tkone-schedule/README.md) | Core recurrence library with a mini-language for date, time, and combined datetime specs |
| [tkone-trigger](crates/tkone-trigger/README.md) | In-memory scheduler built on tkone-schedule; fans out each tick to async callbacks |
| [tkone-trigger-macros](crates/tkone-trigger-macros/README.md) | Declarative `#[schedule]` / `#[job]` attribute macros for zero-boilerplate wiring |
| [tkone-tempo](crates/tkone-tempo/README.md) | Enterprise distributed scheduler with PostgreSQL persistence and transactional messaging |
| [example-app](crates/example-app/src/) | Runnable examples for all crates |

## Documentation

- [Scheduling in Rust](docs/scheduling-in-rust.md) — patterns and design rationale
- [Date spec](crates/tkone-schedule/src/date/date-spec.md) · [Time spec](crates/tkone-schedule/src/time/time-spec.md) · [Datetime spec](crates/tkone-schedule/src/datetime/date-time-spec.md) — mini-language references
- [Tempo architecture](crates/tempo/docs/components.md) · [ERD](crates/tempo/docs/erd.md) · [Sequences](crates/tempo/docs/sequences.md)
- [Kubernetes deployment](crates/tempo/docs/kubernetes.md) · [Docker Compose](crates/tempo/docs/docker-compose.md)

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.
