//! # mtlog-core
//! Core utilities for mtlog - shared logging infrastructure.

mod config;
mod log_writer;
mod utils;

pub use config::MTLOG_CONFIG;
pub use log_writer::{LogFile, LogStdout, LogWriter};
pub use utils::{
    LogMessage, LogSender, LoggerGuard, spawn_log_thread_file, spawn_log_thread_stdout,
};
