# ADR 003: WebDAV Client — reqwest + custom PROPFIND parser

**Status**: Accepted  
**Date**: 2025-05-24

## Context

Nextcloud exposes its file API over WebDAV (RFC 4918). We need a client that:

- Handles the full WebDAV method set: GET, PUT, MKCOL, MOVE, DELETE, PROPFIND
- Parses Nextcloud's extended PROPFIND responses (file_id, etag, OC-Checksum headers)
- Supports chunked upload via the Nextcloud Chunked Upload API (TUS-like, not standard WebDAV)
- Handles HTTP 401 → `ClientError::AuthRequired` cleanly for the re-auth signal

## Decision

Use **reqwest** (Tokio-native async HTTP) with a custom XML parser built on **quick-xml** for PROPFIND responses.

reqwest is the de facto standard async HTTP client for Rust, with TLS, redirects, and streaming already handled. Writing our own PROPFIND parser (rather than using a generic WebDAV library) lets us extract Nextcloud-specific properties (oc:fileid, oc:checksum) without fighting a library's abstraction layer.

## Alternatives Considered

| Library | Rejected because |
|---------|-----------------|
| dav-client crate | Last updated 2021; no async support; no Nextcloud extensions |
| webdav-handler | Server-side library, not a client |
| isahc | Less community support; reqwest is better documented for our use case |

## Consequences

- We own the PROPFIND XML parsing; Nextcloud WebDAV schema changes require our update
- Chunked upload protocol is implemented from Nextcloud's documentation, not a standard
- reqwest's connection pool is shared across all requests; per-account credentials are passed per-request
- HTTP 401 and 403 both map to `ClientError::AuthRequired` (conservative; avoids silently retrying 401)
