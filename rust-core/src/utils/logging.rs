use tracing::Level;
use tracing_subscriber::fmt;

pub fn init_logging(level: Option<Level>) {
    let log_level = level.unwrap_or(Level::INFO);
    
    let subscriber = fmt::Subscriber::builder()
        .with_max_level(log_level)
        .with_thread_ids(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");
}

pub fn init_logging_with_file(level: Option<Level>, log_file: &str) {
    let log_level = level.unwrap_or(Level::INFO);
    
    let file_appender = tracing_appender::rolling::daily(".", log_file);
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = fmt::Subscriber::builder()
        .with_max_level(log_level)
        .with_thread_ids(true)
        .with_line_number(true)
        .with_writer(non_blocking)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set global subscriber");
}
