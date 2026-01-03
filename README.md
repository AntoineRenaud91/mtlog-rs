# mtlog

Multi-threaded/task loggers with per-thread/task configuration and support for progress bars and log files.

## Crates

- **[`mtlog`](https://crates.io/crates/mtlog)** - For standard multi-threaded applications
- **[`mtlog-tokio`](https://crates.io/crates/mtlog-tokio)** - For async applications with tokio
- **[`mtlog-progress`](https://crates.io/crates/mtlog-progress)** - For progress bars (works with both mtlog and mtlog-tokio)
- **[`mtlog-core`](https://crates.io/crates/mtlog-core)** - Internal shared infrastructure

## Quick Start

### Basic Usage

```toml
[dependencies]
mtlog = "0.2"
```

```rust
use mtlog::logger_config;

let _guard = logger_config()
   .init_global();
log::info!("Hello, world!");
```

### Multi-threaded logging
```rust
use mtlog::logger_config;

let _guard = logger_config()
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
```

### Logging to files
Files can be used to log messages. The log file is created if it does not exist and appended to if it does.
Threads can log to different files. If no file is specified in local config, the global file is used.

```rust
use mtlog::logger_config;

let _guard = logger_config()
    .with_log_file("/tmp/app.log")
    .expect("Unable to create log file")
    .no_stdout() // disable stdout logging if needed
    .init_global();

log::info!("Hello, world!");
drop(_guard); // ensure logs are flushed
assert!(std::fs::read_to_string("/tmp/app.log").unwrap().ends_with("Hello, world!\n"));
```

### Progress bars

Add `mtlog-progress` for automatic progress tracking on iterators:

```toml
[dependencies]
mtlog = "0.2"
mtlog-progress = "0.2"
```

```rust
use mtlog::logger_config;
use mtlog_progress::ProgressIteratorExt;

let _guard = logger_config().init_global();

(0..100)
    .progress("Processing")
    .for_each(|i| {
        // Your work here
        if i == 50 {
            log::info!("Halfway there!");
        }
    });
```

### Async with tokio

For async applications, use `mtlog-tokio`:

```toml
[dependencies]
mtlog-tokio = "0.2"
tokio = { version = "1", features = ["full"] }
```

```rust
use mtlog_tokio::logger_config;

#[tokio::main]
async fn main() {
    logger_config()
        .with_name("main")
        .scope_global(async move {
            log::info!("Hello from async main!");

            let handles: Vec<_> = (0..5).map(|i| {
                tokio::spawn(async move {
                    logger_config()
                        .with_name(&format!("task {i}"))
                        .scope_local(async move {
                            log::info!("Hello from task {i}!");
                        }).await;
                })
            }).collect();

            for h in handles { h.await.unwrap(); }
        }).await;
}
```

## License

MIT