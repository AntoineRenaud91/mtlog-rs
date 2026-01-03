use mtlog::logger_config;
use mtlog_progress::ProgressIteratorExt;
use std::{thread, time::Duration};

fn main() {
    let _guard = logger_config().init_global();
    (0..50).progress("Progress").for_each(|i| {
        thread::sleep(Duration::from_millis(20));
        if i == 25 {
            log::info!("Halfway through the range!");
        }
    });
}
