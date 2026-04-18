//! # tempo
//!
//! Enterprise-grade distributed scheduler built on [`tkone_schedule`].
//!
//! Tempo adds a persistent coordination layer on top of the in-process
//! scheduling primitives in `tkone-schedule` and `tkone-trigger`:
//!
//! - Pre-generated occurrences stored in PostgreSQL
//! - Distributed claim/fire via `SELECT FOR UPDATE SKIP LOCKED`
//! - Transactional outbox → Kafka / Iggy for reliable downstream notification
//! - Idempotent inbox for completion tracking
//! - Versioned schedule definitions with non-disruptive rollout
//! - Horizontal scalability through shard-key-partitioned workers
//!
//! See the [`docs/`](../docs/) folder for architecture, design decisions,
//! entity diagrams, and end-to-end sequence diagrams.
