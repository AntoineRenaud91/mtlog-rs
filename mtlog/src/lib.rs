//! # mtlog
//! Multi-threaded logger with support for progress bars and log files.
//!
//! ## Usage
//! ```toml
//! // Cargo.toml
//! ...
//! [dependencies]
//! mtlog = "0.1.4"
//! ```
//! 
//! ```rust
//! use mtlog::logger_config;
//! 
//! logger_config()
//!    .init_global();
//! log::info!("Hello, world!");
//! std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
//! ```
//! 
//! ## Multi-threaded logging
//! ```rust
//! use mtlog::logger_config;
//! 
//! logger_config()
//!     .with_name("main")
//!     .init_global();
//! 
//! log::info!("Hello, world from main thread!");
//! 
//! for i in 0..5 {
//!     std::thread::spawn(move || {
//!        logger_config()
//!             .with_name(&format!("thread {i}"))
//!             .init_local();
//!     log::warn!("Hello, world from thread {i}!")
//!    });
//! }
//! std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
//! ```
//! 
//! ## Logging to files
//! Files can be used to log messages. The log file is created if it does not exist and appended to if it does.
//! Threads can log to different files. If no file is specified in local config, the global file is used.
//! 
//! ```rust
//! use mtlog::logger_config;
//! 
//! logger_config()
//!     .with_log_file("/tmp/app.log")
//!     .expect("Unable to create log file")
//!     .no_stdout() // disable stdout logging if needed   
//!     .init_global();
//! 
//! log::info!("Hello, world!");
//! std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
//! assert!(std::fs::read_to_string("/tmp/app.log").unwrap().ends_with("Hello, world!\n"));
//! ```


// mod progress_bar;
mod log_writer;
mod utils;

use std::{cell::RefCell, path::Path, sync::{Arc, LazyLock, RwLock}};
use log_writer::{LogFile, LogStdout};
use utils::{spawn_log_thread, LogSender, LogMessage};
use log::{LevelFilter, Log};


/// Configuration for the logger.
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


thread_local! {
    /// Thread-local logger configuration for finer control over logging settings per thread.
    pub static LOG_CONFIG: RefCell<Option<LogConfig>> = const { RefCell::new(None) };
}


/// Custom logger implementation for handling log records.
struct MTLogger;

impl Log for MTLogger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        LOG_CONFIG.with(|local_config| {
            let local_config = local_config.borrow();
            let global_config = GLOBAL_LOG_CONFIG.read().unwrap();
            let config = local_config.as_ref().unwrap_or(&global_config);
            let level = record.level();
            if level > config.level {
                return;
            }
            let log_message = Arc::new(LogMessage { level, name: config.name.clone(), message: record.args().to_string()});
            if let Some(sender) = &config.sender_stdout {
                sender.send(log_message.clone()).expect("Unable to send log message to stdout logging thread");
            }
            if let Some(sender) = &config.sender_file {
                sender.send(log_message).expect("Unable to send log message to file logging thread");
            }
        });
    }

    fn flush(&self) {
        if let Some(s) = LOG_CONFIG.take() {
            if let Some(s) = s.sender_stdout.as_deref() {
                s.shutdown();
            }
            if let Some(s) = s.sender_file.as_deref() {
                s.shutdown();
            }
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
        let Self { log_file, no_stdout, no_file, log_level, name } = self;
        let sender_file = if no_file {
            None
        } else if let Some(log_file) = log_file {
            let sender = spawn_log_thread(log_file);
            Some(Arc::new(sender))
        } else {
            GLOBAL_LOG_CONFIG.read().unwrap().sender_file.clone()
        };
        let sender_stdout = if no_stdout {None} else {
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
    pub fn with_log_file<P: AsRef<Path>>(self, path: P) -> Result<Self,std::io::Error> {
        Ok(Self { log_file: Some(LogFile::new(path)?), ..self })
    }
    /// Maybe sets a log file.
    pub fn maybe_with_log_file<P: AsRef<Path>>(self, path: Option<P>) -> Result<Self,std::io::Error> {
        Ok(Self { log_file: path.map(|p| LogFile::new(p)).transpose()? , ..self })
    }
    /// Ignore stdout logging
    pub fn no_stdout(self) -> Self {
        Self { no_stdout: true, ..self }
    }
    /// Dynamically set the stdout flag.
    pub fn with_stdout(self, yes: bool) -> Self {
        Self { no_stdout: !yes, ..self }
    }
    /// Ignore file logging
    pub fn no_file(self) -> Self {
        Self { no_file: true, ..self }
    }
    /// Sets a log name
    pub fn with_name(self, name: &str) -> Self {
        Self { name: Some(name.into()), ..self }
    }
    /// Maybe sets a log name
    pub fn maybe_with_name(self, name: Option<&str>) -> Self {
        Self { name: name.map(String::from), ..self }
    }
    // Initalize the logger globaly
    pub fn init_global(self) {
        *GLOBAL_LOG_CONFIG.write().unwrap()=self.build();
    }
    // Initalize the logger for the current thread
    pub fn init_local(self) {
        LOG_CONFIG.with(|logger_config| {
            let mut logger_config = logger_config.borrow_mut();
            *logger_config = Some(self.build());
        });
    }
}

/// Returns a default ConfigBuilder for configuring the logger.
pub fn logger_config() -> ConfigBuilder {
    ConfigBuilder::default()
}