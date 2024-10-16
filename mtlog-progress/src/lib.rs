
//! # mtlog-progress
//! A progress bar implementation working gracefully with mtlog's logger.
//!
//! ## Usage with std threads
//! ```toml
//! // Cargo.toml
//! ...
//! [dependencies]
//! mtlog-progress = "0.1.0"
//! mtlog = "0.1.4"
//! ```
//! 
//! ```rust
//! use mtlog::logger_config;
//! use mtlog_progress::LogProgressBar;
//! 
//! logger_config()
//!     .init_global();
//! 
//! let h = std::thread::spawn(|| {
//!     let pb = LogProgressBar::new(100, "My Progress Bar");
//!     for i in 0..100 {
//!         pb.inc(1);
//!         if i == 50 {
//!             log::info!("Halfway there!");
//!         }
//!     }
//!     pb.finish();
//! });
//! log::info!("This log goes below the progress bar");
//! h.join().unwrap(); // the progress bar continue to work at it's line position
//! 
//! ```
//! ## Usage with tokio tasks
//! 
//! ## Usage
//! ```toml
//! // Cargo.toml
//! ...
//! [dependencies]
//! mtlog-progress = "0.1.0"
//! mtlog-tokio = "0.1.0"
//! tokio = { version = "1.40.0", features = ["full"] }
//! ```
//! 
//! ```rust
//! use mtlog_tokio::logger_config;
//! use mtlog_progress::LogProgressBar;
//! 
//! #[tokio::main]
//! async fn main() {
//!     logger_config()
//!         .scope_global(async move {
//!             let h = tokio::spawn(async move {
//!                 logger_config()
//!                     .scope_local(async move {
//!                         let pb = LogProgressBar::new(100, "My Progress Bar");
//!                         for i in 0..100 {
//!                             pb.inc(1);
//!                             if i == 50 {
//!                                 log::info!("Halfway there!");
//!                             }
//!                         }
//!                         pb.finish();
//!                     }).await;    
//!             });
//!             log::info!("This log goes below the progress bar");
//!             h.await.unwrap(); // the progress bar continue to work at it's line position
//!         }).await;
//! }
//! ```


use std::sync::{Arc, Mutex};
use colored::Colorize;
use uuid::Uuid;


#[derive(Clone)]
pub struct LogProgressBar {
    n_iter: Arc<usize>,
    name: Arc<str>,
    current_iter: Arc<Mutex<usize>>,
    id: Arc<Uuid>,
    finished: Arc<Mutex<bool>>
}

impl LogProgressBar {
    pub fn new(n_iter: usize, name: &str) -> Self {
        let pb = Self {
            n_iter: Arc::new(n_iter),
            name: name.into(),
            current_iter: Arc::new(Mutex::new(0usize)),
            id: Arc::new(Uuid::new_v4()),
            finished: Arc::new(Mutex::new(false))
        };
        pb.send();
        pb
    }

    pub fn send(&self) {
        if *self.finished.lock().unwrap() {
            log::info!("___PROGRESS___{}___FINISHED",self.id)
        } else {
            log::info!("___PROGRESS___{}___{}",self.id,self.format())
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
    use mtlog::logger_config;
    logger_config()
        .init_global();
    let pb = LogProgressBar::new(100, "Test");
    for _ in 0..50 {
        pb.inc(1);
    }
    pb.finish();
    std::thread::sleep(std::time::Duration::from_millis(1));
}
