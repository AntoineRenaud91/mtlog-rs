# mtlog
Multi-threaded logger with support for progress bars and log files.
//!
## Usage
```toml
// Cargo.toml
...
[dependencies]
mtlog = "0.1.0"
```

```rust
use mtlog::logger_config;

logger_config()
   .init_global();
log::info!("Hello, world!");
std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
```

## Multi-threaded logging
```rust
use mtlog::logger_config;

logger_config()
    .with_name("main")
    .init_global();

log::info!("Hello, world from main thread!");

for i in 0..5 {
    std::thread::spawn(move || {
       logger_config()
            .with_name(&format!("thread {i}"))
            .init_local();
    log::warn!("Hello, world from thread {i}!")
   });
}
std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
```

## Logging to files
Files can be used to log messages. The log file is created if it does not exist and appended to if it does.
Threads can log to different files. If no file is specified in local config, the global file is used.

```rust
use mtlog::logger_config;

logger_config()
    .with_log_file("/tmp/app.log")
    .expect("Unable to create log file")
    .no_stdout() // disable stdout logging if needed   
    .init_global();

log::info!("Hello, world!");
std::thread::sleep(std::time::Duration::from_millis(1)); // wait for log to flush
assert!(std::fs::read_to_string("/tmp/app.log").unwrap().ends_with("Hello, world!\n"));
```

## Progress bar
A progress bar implementation is provided. Multiple progress bars can be created and updated concurrently without interfering with each other and regular logs

```rust
use mtlog::{logger_config,LogProgressBar};

logger_config()
    .init_global();

let pb = LogProgressBar::new(100, "My Progress Bar");
for i in 0..100 {
    pb.inc(1);
    if i == 50 {
       log::info!("Halfway there!");
    }
}
pb.finish();
```