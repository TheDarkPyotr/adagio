//! Re-exports the Nextcloud E2EE OCS API client from `adagio-nextcloud`.
//!
//! Centralises the import so `provider.rs` only depends on this module.
pub use adagio_nextcloud::E2eeOcsClient;
