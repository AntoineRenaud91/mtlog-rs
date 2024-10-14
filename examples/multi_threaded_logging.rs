
use std::sync::mpsc::channel;

use mtlog::logger_config;

fn main() {
    // main thread log to stdout only
    logger_config()
        .with_name("main thread")
        .init_global();
    log::info!("Hello, world!");
    // threaded tasks log to files
    let (handles, senders): (Vec<_>,Vec<_>) = (0..5).map(|i| {
        let (sender, receiver) = channel::<&'static str>();
        (std::thread::spawn(move || {
            logger_config()
                .with_name(&format!("thread {i}"))
                .with_log_file(format!("/tmp/thread_{i}.log"))
                .unwrap()
                .init_local();
            for message in receiver {
                log::warn!("MESSAGE RECEIVED: {message}");
            }
        }),sender)
    }).unzip();
    for sender in senders {
        sender.send("Hello, world!").unwrap();
    }
    for handle in handles {
        handle.join().unwrap();
    }
    for i in 0..5 {
        log::info!("last line of /tmp/thread_{i}.log is:\n\t{}",std::fs::read_to_string(format!("/tmp/thread_{i}.log")).unwrap().trim_end().lines().last().unwrap());
    }
    std::thread::sleep(std::time::Duration::from_millis(1)); // wait for the last log to be writtens
}