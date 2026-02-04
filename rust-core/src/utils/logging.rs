use tracing::{Level, Subscriber};
use tracing_subscriber::{fmt, EnvFilter, Layer, Registry};

pub fn init_logging(level: Option<Level>) {
    let log_level = level.unwrap_or(Level::INFO);
    
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.as_str()));

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_line_number(true);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(fmt_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");
}

pub fn init_logging_with_file(level: Option<Level>, log_file: &str) {
    let log_level = level.unwrap_or(Level::INFO);
    
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level.as_str()));

    let file_appender = tracing_appender::rolling::daily(".", log_file);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let fmt_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_level(true)
        .with_line_number(true);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false);

    let subscriber = Registry::default()
        .with(env_filter)
        .with(fmt_layer)
        .with(file_layer);

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");
}
