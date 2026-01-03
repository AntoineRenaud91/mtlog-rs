# mtlog-tokio

[![Crates.io](https://img.shields.io/crates/v/mtlog-tokio.svg)](https://crates.io/crates/mtlog-tokio)
[![Documentation](https://docs.rs/mtlog-tokio/badge.svg)](https://docs.rs/mtlog-tokio)

Scoped logging for tokio runtimes with per-task configuration and support for log files.

## Related Crates

- **[`mtlog`](https://crates.io/crates/mtlog)** - Use this instead for non-async, standard multi-threaded applications
- **[`mtlog-progress`](https://crates.io/crates/mtlog-progress)** - Add this for progress bars that work gracefully with mtlog

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mtlog-tokio = "0.2"
tokio = { version = "1", features = ["full"] }
```

## Features

- Scoped logging with automatic cleanup using async/await
- Task-local logger configuration
- File logging with automatic file creation and appending
- Configurable log levels and output destinations
- Designed specifically for tokio async runtimes

## Usage

### Basic Usage

```rust
use mtlog_tokio::logger_config;

#[tokio::main]
async fn main() {
    logger_config()
        .scope_global(async move {
            log::info!("Hello, world!");
            // logs are automatically flushed when scope_global completes
        }).await;
}
```

### Multi-task Logging

```rust
use mtlog_tokio::logger_config;

#[tokio::main]
async fn main() {
    logger_config()
        .with_name("main")
        .scope_global(async move {
            log::info!("Hello, world from main task!");
            let handles: Vec<_> = (0..5).map(|i| {
                tokio::spawn(async move {
                    logger_config()
                        .with_name(&format!("task {i}"))
                        .scope_local(async move {
                            log::warn!("Hello, world from task {i}!")
                        }).await;
                })
            }).collect();
            for h in handles { h.await.unwrap(); }
        }).await;
}
```

### Logging to Files

Files can be used to log messages. The log file is created if it does not exist and appended to if it does.
Tasks can log to different files. If no file is specified in local config, the global file is used.

```rust
use mtlog_tokio::logger_config;

#[tokio::main]
async fn main() {
    logger_config()
        .with_log_file("/tmp/app.log")
        .unwrap()
        .no_stdout() // disable stdout logging if needed
        .scope_global(async move {
            log::info!("Hello, world!");
        }).await;
    assert!(std::fs::read_to_string("/tmp/app.log").unwrap().ends_with("Hello, world!\n"));
}
```

## Configuration

### Flush Interval

For performance optimization, file logging uses batched writes that are flushed periodically. This can be configured via the `MTLOG_FLUSH_INTERVAL_MS` environment variable.

**Default**: 100ms

## Documentation

For detailed API documentation, visit [docs.rs/mtlog-tokio](https://docs.rs/mtlog-tokio).

## License

MIT
