use std::{ops::Deref, sync::{mpsc::{channel, Sender}, Arc}, thread::JoinHandle};

use chrono::Utc;
use colored::Colorize;
use log::Level;
use uuid::Uuid;

use crate::log_writer::LogWriter;


/// Enum representing a log message which can be of three different types.
#[derive(Debug,Clone)]
pub enum LogMessage {
    /// A regular log message with level, name and message
    Regular{level: Level, name: Option<String>, message: String},
    /// A progress log message, containing the progress bar identifier, the progress message, and a flag indicating if it's finished.
    Progress{uuid: Uuid, message: String},
    /// A progress log message indicating the end of the progress bar.
    Finished{uuid: Uuid},
    /// An shutdown message indicating the end of logging.
    Shutdown
}

pub struct LogSender{
    sender: Sender<Arc<LogMessage>>,
    handler: Option<JoinHandle<bool>>,
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
        Self {sender, handler: Some(handler), shutdown_initiated: false}
    }
    pub fn shutdown(&mut self) {
        self.send(Arc::new(LogMessage::Shutdown)).expect("Unable to send shutdown message to file logger thread");
        if !self.handler.take().unwrap().join().expect("Unable to join file logger thread") {
            panic!("Logger thread shutdown failed");
        };
    }
}

fn format_log(message: &str, level: Level, name: &Option<String>) -> String {
    let time = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f");
    let level = match level {
        log::Level::Error => "ERROR".red(),
        log::Level::Warn => "WARN".yellow(),
        log::Level::Info => "INFO".green(),
        log::Level::Debug => "DEBUG".blue(),
        log::Level::Trace => "TRACE".purple(),
    };
    if let Some(name) = name {
        format!("[{time} {name} {}] {}", level, message)
    } else {
        format!("[{time} {}] {}", level, message)
    }
}

pub fn spawn_log_thread<W: LogWriter+Send+'static>(mut writer: W)-> LogSender {
    let (sender, receiver) = channel::<Arc<LogMessage>>();
    let handler = std::thread::spawn(move || {
        for log_message in receiver {
            match log_message.as_ref() {
                LogMessage::Regular { level, name, message } => {
                    let message = format_log(message, *level, name);
                    writer.regular(&message);
                },
                LogMessage::Progress { uuid, message } => {
                    writer.progress(message, *uuid);
                }
                LogMessage::Finished { uuid } => {
                    writer.finished(*uuid);
                }
                LogMessage::Shutdown => {
                    break;
                }
            }
        }
        true
    });
    LogSender::new(sender, handler)
}