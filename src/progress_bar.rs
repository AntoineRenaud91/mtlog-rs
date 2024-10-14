use std::sync::{Arc, Mutex};
use colored::Colorize;
use uuid::Uuid;


use crate::LogMessage;

use super::{LogSender,LOG_CONFIG, GLOBAL_LOG_CONFIG};

#[derive(Clone)]
pub struct LogProgressBar {
    sender_stdout: Option<Arc<LogSender>>,
    sender_file: Option<Arc<LogSender>>,
    n_iter: Arc<usize>,
    name: Arc<str>,
    current_iter: Arc<Mutex<usize>>,
    id: Arc<Uuid>,
    finished: Arc<Mutex<bool>>
}

impl LogProgressBar {
    pub fn new(n_iter: usize, name: &str) -> Self {
        let progress_bar = LOG_CONFIG.with(|local_config| {
            let local_config = local_config.borrow();
            let global_config = GLOBAL_LOG_CONFIG.read().unwrap();
            let config = local_config.as_ref().unwrap_or(&global_config);
            let sender_stdout = config.sender_stdout.clone();
            let sender_file = config.sender_file.clone();
            Self {
                n_iter: Arc::new(n_iter),
                name: name.into(),
                current_iter: Arc::new(Mutex::new(0usize)),
                id: Arc::new(Uuid::new_v4()),
                finished: Arc::new(Mutex::new(false)),
                sender_stdout,
                sender_file
            }
        });
        progress_bar.send();
        progress_bar
    }

    pub fn send(&self) {
        let log_message = Arc::new(if *self.finished.lock().unwrap() {
            LogMessage::Finished { uuid: *self.id }
        } else {
            LogMessage::Progress { uuid: *self.id, message: self.format()}
        });
        if let Some(sender) = &self.sender_stdout {
            sender.send(log_message.clone()).expect("Unable to send log message");
        }
        if let Some(sender) = &self.sender_file {
            sender.send(log_message).expect("Unable to send log message");
        }
    }

    pub fn set_progress(&self, n: usize) {
        *self.current_iter.lock().unwrap() = n;
        self.send();
    }

    pub fn inc(&self, n: usize) {
        *self.current_iter.lock().unwrap() += n;
        self.send();
    }

    fn format(&self) -> String {
        let current_iter = *self.current_iter.lock().unwrap();
        let percentage = (current_iter as f64 / *self.n_iter as f64 * 100.0) as usize;
        let bar_length = 20; // Length of the progress bar
        let filled_length = (bar_length * current_iter / *self.n_iter).min(bar_length);
        let bar = "#".repeat(filled_length) + &".".repeat(bar_length - filled_length);
        let n_iter_str = self.n_iter.to_string();
        format!(
            "Progress {name}: [{bar}] {current:>len$}/{n_iter_str} {percentage:>3}%",
            name=self.name.cyan(), 
            bar=bar.cyan(),
            current=current_iter,
            len=n_iter_str.len(),
        )
    }
    
    pub fn finish(&self) {
        if *self.finished.lock().unwrap() {
            return
        }
        *self.finished.lock().unwrap() = true;    
        *self.current_iter.lock().unwrap() = *self.n_iter;
        self.send();
    }
}

impl Drop for LogProgressBar {
    fn drop(&mut self) {
        *self.finished.lock().unwrap() = true;
        self.send();
    }
}


#[test]
fn test_progress_bar() {
    use super::logger_config;
    logger_config()
        .init_global();
    let pb = LogProgressBar::new(100, "Test");
    for _ in 0..50 {
        pb.inc(1);
    }
    pb.finish();
    std::thread::sleep(std::time::Duration::from_millis(1));
}
