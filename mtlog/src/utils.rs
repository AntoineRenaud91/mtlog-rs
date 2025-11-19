// mtlog/src/utils.rs

use std::{
    ops::Deref,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::{Duration, Instant},
};

use chrono::Utc;
use colored::Colorize;
use crossbeam_channel::{RecvTimeoutError, Sender, unbounded};
use log::Level;
use uuid::Uuid;

use crate::{config::MTLOG_CONFIG, log_writer::LogWriter};

#[derive(Debug, Clone)]
pub struct LogMessage {
    pub message: String,
    pub level: Level,
    pub name: Option<String>,
}

pub struct LogSender {
    sender: Sender<Arc<LogMessage>>,
    handler: Arc<Mutex<Option<JoinHandle<bool>>>>,
    shutdown_initiated: bool,
}
impl Deref for LogSender {
    type Target = Sender<Arc<LogMessage>>;
    fn deref(&self) -> &Self::Target {
        &self.sender
    }
}
impl Drop for LogSender {
    fn drop(&mut self) {
        if !self.shutdown_initiated {
            self.shutdown();
        }
    }
}

impl LogSender {
    pub fn new(sender: Sender<Arc<LogMessage>>, handler: JoinHandle<bool>) -> Self {
        Self {
            sender,
            handler: Arc::new(Mutex::new(Some(handler))),
            shutdown_initiated: false,
        }
    }
    pub fn shutdown(&self) {
        self.send(Arc::new(LogMessage {
            message: "___SHUTDOWN___".into(),
            level: Level::Info,
            name: None,
        }))
        .expect("Unable to send shutdown message to file logger thread");
        if !self
            .handler
            .lock()
            .unwrap()
            .take()
            .unwrap()
            .join()
            .expect("Unable to join file logger thread")
        {
            panic!("Logger thread shutdown failed");
        };
    }
}

fn format_log(message: &str, level: Level, name: Option<&str>) -> String {
    let time = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f");
    let level = match level {
        log::Level::Error => "ERROR".red(),
        log::Level::Warn => "WARN".yellow(),
        log::Level::Info => "INFO".green(),
        log::Level::Debug => "DEBUG".blue(),
        log::Level::Trace => "TRACE".purple(),
    };
    if let Some(name) = name {
        format!("[{time} {name} {level}] {message}")
    } else {
        format!("[{time} {level}] {message}")
    }
}

pub fn spawn_log_thread<W: LogWriter + Send + 'static>(mut writer: W) -> LogSender {
    let (sender, receiver) = unbounded::<Arc<LogMessage>>();
    let handler = std::thread::spawn(move || {
        let mut batch = Vec::with_capacity(32);
        let flush_interval = Duration::from_millis(MTLOG_CONFIG.FLUSH_INTERVAL_MS);
        let mut last_flush = Instant::now();
        loop {
            // Calculate timeout until next flush
            let elapsed = last_flush.elapsed();
            let timeout = if elapsed >= flush_interval {
                Duration::from_millis(1) // Force immediate processing
            } else {
                flush_interval - elapsed
            };

            // Collect a batch of messages with timeout
            match receiver.recv_timeout(timeout) {
                Ok(msg) => {
                    batch.push(msg);
                    // Try to collect more messages without blocking
                    while let Ok(msg) = receiver.try_recv() {
                        batch.push(msg);
                        if batch.len() >= 32 {
                            break;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // Timeout reached - flush if needed
                    if last_flush.elapsed() >= flush_interval && !batch.is_empty() {
                        // Process any pending messages
                    } else if batch.is_empty() {
                        // No messages to process, but we should flush the writer
                        writer.flush();
                        last_flush = Instant::now();
                        continue;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }

            // Process the batch
            let mut should_shutdown = false;
            for log_message in batch.drain(..) {
                let LogMessage {
                    message,
                    level,
                    name,
                } = log_message.as_ref();

                if message == "___SHUTDOWN___" {
                    should_shutdown = true;
                    break;
                }

                if message.starts_with("___PROGRESS___") {
                    let message = message.trim_start_matches("___PROGRESS___");
                    if let Some((uuid_str, message)) = message.split_once("___")
                        && let Ok(uuid) = Uuid::parse_str(uuid_str)
                    {
                        if message == "FINISHED" {
                            writer.finished(uuid);
                        } else {
                            writer.progress(message, uuid);
                        }
                    }
                } else {
                    let message = format_log(message, *level, name.as_deref());
                    writer.regular(&message);
                }
            }

            // Flush periodically or when shutting down
            if should_shutdown || last_flush.elapsed() >= flush_interval {
                writer.flush();
                last_flush = Instant::now();
            }

            if should_shutdown {
                break;
            }
        }
        true
    });
    LogSender::new(sender, handler)
}
