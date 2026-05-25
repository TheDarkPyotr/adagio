# ADR 002: Journal Storage — SQLite via sqlx

**Status**: Accepted  
**Date**: 2025-05-24

## Context

The sync journal records the last-known-good state for every file: checksum, etag, mtime, sync status, retry count, and error messages. It must:

- Persist across process restarts (durable)
- Support concurrent reads from the UI thread alongside write-heavy sync cycles
- Be queryable by pair_id, path, status, and file_id
- Fit on resource-constrained devices (< 50 MB for 500k entries)

## Decision

Use **SQLite** accessed through **sqlx** in WAL (Write-Ahead Logging) mode.

SQLite is embedded, requires no server, and is well-supported on all platforms. WAL mode allows concurrent readers and a single writer without blocking. sqlx provides compile-time query checking and async access, fitting naturally into our Tokio runtime. The schema is a single `journal_entries` table with indexed columns for the most common queries.

## Alternatives Considered

| Storage | Rejected because |
|---------|-----------------|
| PostgreSQL / MySQL | Requires separate server process; non-embedded |
| sled (embedded Rust KV) | No SQL query planner; complex multi-column lookups |
| RocksDB | Overkill for < 1M entries; complex compaction tuning |
| Plain JSON files | No atomic multi-key updates; O(n) scan for queries |

## Consequences

- SQLite WAL mode limits to one writer at a time; high-throughput write phases serialise through the journal
- Database migrations require forward-compatible schema changes (additive only while in v0)
- sqlx requires `cargo sqlx prepare` to regenerate offline query metadata after schema changes
