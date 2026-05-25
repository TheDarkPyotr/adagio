use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize structured tracing. Call once at application startup.
///
/// In release builds, emits JSON to stdout. In debug builds, emits
/// human-readable colorized output. Filter is controlled by RUST_LOG or
/// the provided `default_filter` string.
pub fn init(default_filter: &str) {
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

    #[cfg(not(debug_assertions))]
    {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().json())
            .init();
    }

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt::layer().pretty())
            .init();
    }
}
