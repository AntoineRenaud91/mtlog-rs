//! # mtlog-progress
//! A progress bar implementation working gracefully with mtlog's logger.
//!
//! ## Usage with std threads
//! ```toml
//! // Cargo.toml
//! ...
//! [dependencies]
//! mtlog-progress = "0.2.0"
//! mtlog = "0.2.0"
//! ```
//!
//! ```rust
//! use mtlog::logger_config;
//! use mtlog_progress::LogProgressBar;
//!
//! let _guard = logger_config()
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
//! // guard ensures logs are flushed when dropped
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

use colored::Colorize;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct LogProgressBar {
    n_iter: Arc<usize>,
    name: Arc<str>,
    current_iter: Arc<Mutex<usize>>,
    id: Arc<Uuid>,
    finished: Arc<Mutex<bool>>,
    min_duration: Arc<Duration>,
    last_iter: Arc<Mutex<Instant>>,
    last_percentage: Arc<Mutex<f64>>,
    min_percentage_change: Arc<f64>,
}

impl LogProgressBar {
    pub fn new(n_iter: usize, name: &str) -> Self {
        let pb = Self {
            n_iter: Arc::new(n_iter.max(1)),
            name: name.into(),
            current_iter: Arc::new(Mutex::new(0usize)),
            id: Arc::new(Uuid::new_v4()),
            finished: Arc::new(Mutex::new(false)),
            min_duration: Arc::new(Duration::from_millis(100)),
            last_iter: Arc::new(Mutex::new(Instant::now() - Duration::from_millis(100))),
            last_percentage: Arc::new(Mutex::new(0.0)),
            min_percentage_change: Arc::new(0.1),
        };
        pb.send();
        pb
    }

    pub fn with_min_timestep_ms(mut self, min_duration_ms: f64) -> Self {
        self.min_duration = Arc::new(Duration::from_micros(
            (min_duration_ms * 1000.0).round() as u64
        ));
        self
    }

    pub fn with_min_percentage_change(mut self, min_percentage: f64) -> Self {
        self.min_percentage_change = Arc::new(min_percentage);
        self
    }

    pub fn send(&self) {
        if *self.finished.lock().unwrap() {
            return;
        }

        let current_iter = *self.current_iter.lock().unwrap();
        let current_percentage = (current_iter as f64 / *self.n_iter as f64) * 100.0;
        let last_percentage = *self.last_percentage.lock().unwrap();
        let time_elapsed = self.last_iter.lock().unwrap().elapsed() > *self.min_duration;
        let percentage_changed =
            (current_percentage - last_percentage).abs() >= *self.min_percentage_change;

        if time_elapsed || percentage_changed {
            log::info!("___PROGRESS___{}___{}", self.id, self.format());
            *self.last_iter.lock().unwrap() = Instant::now();
            *self.last_percentage.lock().unwrap() = current_percentage;
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
            name = self.name.cyan(),
            bar = bar.cyan(),
            current = current_iter,
            len = n_iter_str.len(),
        )
    }

    pub fn finish(&self) {
        if *self.finished.lock().unwrap() {
            return;
        }
        self.set_progress(*self.n_iter);
        *self.finished.lock().unwrap() = true;
        log::info!("___PROGRESS___{}___FINISHED", self.id)
    }
}

impl Drop for LogProgressBar {
    fn drop(&mut self) {
        if *self.finished.lock().unwrap() {
            return;
        }
        log::info!("___PROGRESS___{}___FINISHED", self.id);
    }
}

#[test]
fn test_progress_bar() {
    use mtlog::logger_config;
    let _guard = logger_config().init_global();
    let n = 5000000;
    let handle = std::thread::spawn(move || {
        let pb = LogProgressBar::new(n, "Background Task");
        for _ in 0..n / 3 {
            pb.inc(1);
        }
        pb.set_progress(0);
        for _ in 0..n / 3 {
            pb.inc(1);
        }
        pb.finish();
    });
    std::thread::sleep(Duration::from_millis(200));
    let pb = LogProgressBar::new(n, "Main Task");
    log::info!("Starting main task");
    for i in 0..n {
        if i == 10 {
            log::info!("Main task is at 10 iterations");
        }
        pb.inc(1);
    }
    pb.finish();
    handle.join().unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let pb_outer = LogProgressBar::new(10, "Outer loop");
    for _ in 0..10 {
        let pb_inner = LogProgressBar::new(n / 10, "Inner loop");
        for _ in 0..n / 10 {
            pb_inner.inc(1);
        }
        pb_inner.finish();
        pb_outer.inc(1);
    }
    pb_outer.finish();
}
