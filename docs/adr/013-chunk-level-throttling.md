# ADR-013: Chunk-Level Throttling in Transfer Functions

**Date**: 2026-05-30
**Status**: Accepted
**Feature**: 009-bandwidth-throttling

## Context

Bandwidth throttling requires a rate limiter (`TokenBucket`) to be invoked for each
unit of data transferred. There are two natural granularities:

1. **Pre-flight per-file** — call `acquire(file_size)` once before uploading/downloading
   the entire file.
2. **Per-chunk loop** — call `acquire(CHUNK_SIZE)` inside the transfer loop for each
   64 KB chunk.

## Decision

Use **per-chunk throttling** at a 64 KB chunk boundary inside `upload_single` and
`download_file`.

## Rationale

**Pre-flight per-file fails for large files.** Consider a 50 MB file at a 500 KB/s
limit: `acquire(50_000_000)` sleeps 100 seconds, then the upload runs at full speed
(~5 Gbps on a fast LAN) in < 0.1 s. Any 10-second measurement window that happens
to capture those 0.1 s of full-speed transfer will read ~500 MB/s — 1000× over the
limit. The spec requires accuracy within 10% over any consecutive 10-second window.

**Per-chunk distributes the throttle evenly.** At 500 KB/s and 64 KB chunks:
- `acquire(65536)` sleeps ~130 ms
- The next chunk starts 130 ms later
- Any 10-second window sees roughly `10 / 0.130 * 65536 = 5 MB`, matching the
  500 KB/s ceiling within a few percent.

**`upload_single` already needs refactoring.** Currently it reads the entire file
into memory before uploading. The chunk refactor simultaneously fixes a memory
overhead issue for large files.

## Alternatives Considered

**Pre-flight acquire(file_size)**: Correct for small files (≤ 1 MB). Violates the
10-second accuracy requirement for large files. Rejected.

**Wrap `reqwest::Body` with a throttled stream**: Clean but requires changes to
`adagio-nextcloud/src/client.rs` (the transport layer). Keeping throttling in
`adagio-core/src/transfer/` maintains better separation of concerns. Rejected.

## Consequences

- `upload_single` and `download_file` each gain one `Option<Arc<TokenBucket>>`
  parameter. All existing callers pass `None` — no behaviour change when unlimited.
- The chunk loop in `upload_single` processes 64 KB at a time. For files that are
  not a multiple of 64 KB, the final chunk is smaller; `acquire(final_chunk_size)`
  is called normally.
