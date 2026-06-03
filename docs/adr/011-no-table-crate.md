# ADR-011: Hand-rolled column formatting instead of a table crate

**Date**: 2026-05-30
**Status**: Accepted
**Feature**: 008-cli-binary

## Context

The `adagio` CLI displays tabular data (pairs, accounts, conflicts, activity) in
human-readable mode. Several crates offer table rendering (`tabled`, `comfy-table`).

## Decision

Use hand-rolled `format!("{:<width$}", value, width = N)` column alignment instead
of a dedicated table crate.

## Rationale

All data arrives from the daemon as `serde_json::Value`. A table crate would require
re-structuring the data into typed rows before rendering — extra code that buys
nothing over direct formatting. The tables are simple: 3–5 columns, one header line,
no cell merging, no borders beyond a header underline. Hand-rolling is ~15 lines per
table and adds zero binary size. JSON mode (`--json`) simply calls
`serde_json::to_string_pretty` on the same raw value without any re-typing.

## Alternatives Considered

**tabled v0.17**: Native JSON export, flexible styling. Adds ~2 MB to binary; overkill
for simple 5-column tables where the data is already `serde_json::Value`.

**comfy-table**: Excellent ANSI styling, no unsafe code. Requires casting data into
`Cell` types; same extra-work problem as tabled.
