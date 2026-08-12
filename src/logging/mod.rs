use tracing_subscriber::EnvFilter;

/// Initialize tracing. Call once from main before any tracing macros.
pub fn init(verbosity: u8) {
    let filter = match verbosity {
        0 => "safehouse=info",
        1 => "safehouse=debug",
        _ => "safehouse=trace",
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter)),
        )
        .init();
}
