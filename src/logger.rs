use chrono::prelude::*;
use env_logger::Builder as EnvLogBuilder;
use gelf;
use log::{self, LevelFilter as LogLevelFilter, Log, Metadata, Record};
use std::env;
use std::io::Write;
use std::sync::Arc;

pub struct CombinedLogger {
    pub inner: Vec<Arc<Log>>,
    pub filter: Box<Fn(&Record) -> bool + Send + Sync>,
}

impl Default for CombinedLogger {
    fn default() -> Self {
        Self {
            inner: vec![],
            filter: Box::new(|_| true),
        }
    }
}

impl Log for CombinedLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.inner.iter().any(|logger| logger.enabled(metadata))
    }

    fn log(&self, record: &Record) {
        if (self.filter)(record) {
            for logger in &self.inner {
                logger.log(record);
            }
        }
    }

    fn flush(&self) {
        for logger in &self.inner {
            logger.flush();
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrayLogConfig {
    /// Endpoint to send messages to
    pub addr: String,
    pub cluster: Option<String>,
}

pub fn init(graylog_config: Option<&GrayLogConfig>) {
    let mut builder = EnvLogBuilder::new();
    builder
        .format(|formatter, record| {
            let now = Utc::now();
            writeln!(formatter, "{} - {:5} - {}", now.to_rfc3339(), record.level(), record.args())
        }).filter(None, LogLevelFilter::Info);

    if let Ok(v) = env::var("RUST_LOG") {
        builder.parse(&v);
    }

    let mut combined_logger = CombinedLogger::default();

    let stdio_logger = Arc::new(builder.build());
    let log_level = stdio_logger.filter();
    let log_filter = {
        let stdio_logger = stdio_logger.clone();
        move |record: &Record| stdio_logger.matches(record)
    };

    combined_logger.filter = Box::new(log_filter);
    combined_logger.inner.push(stdio_logger);

    if let Some(config) = graylog_config {
        let backend = gelf::UdpBackend::new(&config.addr).unwrap();
        let mut logger = gelf::Logger::new(Box::new(backend)).unwrap();

        if let Some(cluster) = config.cluster.as_ref() {
            logger.set_default_metadata(String::from("cluster"), cluster.clone());
        }

        combined_logger.inner.push(Arc::new(logger));
    }

    log::set_max_level(log_level);
    log::set_boxed_logger(Box::new(combined_logger)).expect("Failed to install logger");
}
