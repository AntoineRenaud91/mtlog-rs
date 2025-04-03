use std::{ops::Deref, sync::{mpsc::{channel, Sender}, Arc, Mutex}, thread::JoinHandle};

use chrono::Utc;
use colored::Colorize;
use log::Level;
use uuid::Uuid;

use crate::log_writer::LogWriter;

#[derive(Debug,Clone)]
pub struct LogMessage {
    pub message: String,
    pub level: Level,
    pub name: Option<String>,
}

pub struct LogSender{
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
        Self {sender, handler: Arc::new(Mutex::new(Some(handler))), shutdown_initiated: false}
    }
    pub fn shutdown(&self) {
        self.send(Arc::new(LogMessage {message: "___SHUTDOWN___".into(), level: Level::Info, name: None})).expect("Unable to send shutdown message to file logger thread");
        if !self.handler.lock().unwrap().take().unwrap().join().expect("Unable to join file logger thread") {
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
            let LogMessage { message, level, name } = log_message.as_ref();
            if message == "___SHUTDOWN___" {
                break;
            }
            if message.starts_with("___PROGRESS___") {
                let message = message.trim_start_matches("___PROGRESS___");
                if let Some((uuid_str, message)) = message.split_once("___") {
                    if let Ok(uuid) = Uuid::parse_str(uuid_str) {
                        if message=="FINISHED" {
                            writer.finished(uuid);
                        } else {
                            writer.progress(message, uuid);
                        }
                    }
                }
            } else {
                let message = format_log(message, *level, name);
                writer.regular(&message);
            }
        }
        true
    });
    LogSender::new(sender, handler)
}