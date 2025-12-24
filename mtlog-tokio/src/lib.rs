//! # mtlog-tokio
//! Scoped logging for tokio runtimes with support for log files.
//!
//! ## Usage
//! ```toml
//! // Cargo.toml
//! ...
//! [dependencies]
//! mtlog-tokio = "0.2.0"
//! tokio = {version = "1.40.0", features = ["full"]}
//! ```
//!
//! ```rust
//! use mtlog_tokio::logger_config;
//!
//! #[tokio::main]
//! async fn main() {
//!     logger_config()
//!         .scope_global(async move {
//!             log::info!("Hello, world!");
//!             // logs are automatically flushed when scope_global completes
//!         }).await;
//! }
//! ```
//!
//! ## Multi-threaded logging
//! ```rust
//! use mtlog_tokio::logger_config;
//!
//! #[tokio::main]
//! async fn main() {
//!     logger_config()
//!         .with_name("main")
//!         .scope_global(async move {
//!             log::info!("Hello, world from main thread!");
//!             let handles: Vec<_> = (0..5).map(|i| {
//!                 tokio::spawn(async move {
//!                     logger_config()
//!                         .with_name(&format!("thread {i}"))
//!                         .scope_local(async move {
//!                             log::warn!("Hello, world from thread {i}!")
//!                         }).await;
//!                 })
//!             }).collect();
//!             for h in handles { h.await.unwrap(); }
//!             // logs are automatically flushed when scope_global completes
//!         }).await;
//! }
//! ```
//!
//! ## Logging to files
//! Files can be used to log messages. The log file is created if it does not exist and appended to if it does.
//! Threads can log to different files. If no file is specified in local config, the global file is used.
//!
//! ```rust
//! use mtlog_tokio::logger_config;
//!
//! #[tokio::main]
//! async fn main() {
//!     logger_config()
//!         .with_log_file("/tmp/app.log")
//!         .unwrap()
//!         .no_stdout() // disable stdout logging if needed
//!         .scope_global(async move {
//!             log::info!("Hello, world!");
//!             // logs are automatically flushed when scope_global completes
//!         }).await;
//!     assert!(std::fs::read_to_string("/tmp/app.log").unwrap().ends_with("Hello, world!\n"));
//! }
//! ```

use log::{LevelFilter, Log};
use mtlog_core::{spawn_log_thread, LogFile, LogMessage, LogSender, LogStdout};
use std::{
    future::Future,
    path::Path,
    sync::{Arc, LazyLock, RwLock},
};

/// Configuration for the logger.
#[derive(Clone)]
struct LogConfig {
    /// Optional log message sender to a thread handling file logging.
    sender_file: Option<Arc<LogSender>>,
    /// Optional log message sender to a thread handling stdout.
    sender_stdout: Option<Arc<LogSender>>,
    /// Optional logger name.
    name: Option<String>,
    /// Maximum log level
    level: LevelFilter,
}

/// Global configuration for the logger, accessible across threads.
static GLOBAL_LOG_CONFIG: LazyLock<Arc<RwLock<LogConfig>>> = LazyLock::new(|| {
    log::set_boxed_logger(Box::new(MTLogger)).unwrap();
    log::set_max_level(LevelFilter::Info);
    let sender = spawn_log_thread(LogStdout::default());
    Arc::new(RwLock::new(LogConfig {
        sender_stdout: Some(Arc::new(sender)),
        sender_file: None,
        name: None,
        level: LevelFilter::Info,
    }))
});

tokio::task_local! {
    /// Thread-local logger configuration for finer control over logging settings per thread.
    pub static LOG_CONFIG: LogConfig;
}

/// Custom logger implementation for handling log records.
struct MTLogger;

impl Log for MTLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        LOG_CONFIG.with(|config| {
            let level = record.level();
            if level > config.level {
                return;
            }
            let log_message = Arc::new(LogMessage {
                level,
                name: config.name.clone(),
                message: record.args().to_string(),
            });
            if let Some(sender) = &config.sender_stdout {
                sender
                    .send(log_message.clone())
                    .expect("Unable to send log message to stdout logging thread");
            }
            if let Some(sender) = &config.sender_file {
                sender
                    .send(log_message)
                    .expect("Unable to send log message to file logging thread");
            }
        });
    }

    fn flush(&self) {
        if let Some(s) = GLOBAL_LOG_CONFIG.write().unwrap().sender_stdout.as_deref() {
            s.shutdown();
        }
        if let Some(s) = GLOBAL_LOG_CONFIG.write().unwrap().sender_file.as_deref() {
            s.shutdown();
        }
    }
}

/// Builder for configuring and initializing the logger.
pub struct ConfigBuilder {
    log_file: Option<LogFile>,
    no_stdout: bool,
    no_file: bool,
    log_level: LevelFilter,
    name: Option<String>,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            log_file: None,
            no_stdout: false,
            no_file: false,
            log_level: LevelFilter::Info,
            name: None,
        }
    }
}

impl ConfigBuilder {
    fn build(self) -> LogConfig {
        let Self {
            log_file,
            no_stdout,
            no_file,
            log_level,
            name,
        } = self;
        let sender_file = if no_file {
            None
        } else if let Some(log_file) = log_file {
            let sender = spawn_log_thread(log_file);
            Some(Arc::new(sender))
        } else {
            GLOBAL_LOG_CONFIG.read().unwrap().sender_file.clone()
        };
        let sender_stdout = if no_stdout {
            None
        } else {
            GLOBAL_LOG_CONFIG.read().unwrap().sender_stdout.clone()
        };
        LogConfig {
            sender_file,
            sender_stdout,
            name,
            level: log_level,
        }
    }

    /// Sets a log file.
    pub fn with_log_file<P: AsRef<Path>>(self, path: P) -> Result<Self, std::io::Error> {
        Ok(Self {
            log_file: Some(LogFile::new(path)?),
            ..self
        })
    }
    /// Maybe sets a log file.
    pub fn maybe_with_log_file<P: AsRef<Path>>(
        self,
        path: Option<P>,
    ) -> Result<Self, std::io::Error> {
        Ok(Self {
            log_file: path.map(|p| LogFile::new(p)).transpose()?,
            ..self
        })
    }
    /// Ignore stdout logging
    pub fn no_stdout(self) -> Self {
        Self {
            no_stdout: true,
            ..self
        }
    }
    /// Dynamically set the stdout flag.
    pub fn with_stdout(self, yes: bool) -> Self {
        Self {
            no_stdout: !yes,
            ..self
        }
    }
    /// Ignore file logging
    pub fn no_file(self) -> Self {
        Self {
            no_file: true,
            ..self
        }
    }
    /// Sets a log name
    pub fn with_name(self, name: &str) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }
    /// Maybe sets a log name
    pub fn maybe_with_name(self, name: Option<&str>) -> Self {
        Self {
            name: name.map(String::from),
            ..self
        }
    }
    /// Initialize the logger globally and run the provided future.
    /// The logger is automatically shut down when the future completes.
    pub async fn scope_global<F: Future>(self, f: F) -> F::Output {
        let config = self.build();
        let mut senders = Vec::new();
        if let Some(ref sender) = config.sender_stdout {
            senders.push(Arc::clone(sender));
        }
        if let Some(ref sender) = config.sender_file {
            senders.push(Arc::clone(sender));
        }
        *GLOBAL_LOG_CONFIG.write().unwrap() = config.clone();
        let result = LOG_CONFIG.scope(config, f).await;
        // Shutdown all senders to ensure logs are flushed
        for sender in senders {
            sender.shutdown();
        }
        result
    }
    // Initalize the logger for the current thread
    pub async fn scope_local<F: Future>(self, f: F) -> F::Output {
        LOG_CONFIG.scope(self.build(), f).await
    }
}

/// Returns a default ConfigBuilder for configuring the logger.
pub fn logger_config() -> ConfigBuilder {
    ConfigBuilder::default()
}
