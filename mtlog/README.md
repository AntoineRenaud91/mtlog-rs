# mtlog

[![Crates.io](https://img.shields.io/crates/v/mtlog.svg)](https://crates.io/crates/mtlog)
[![Documentation](https://docs.rs/mtlog/badge.svg)](https://docs.rs/mtlog)

Multi-threaded logger with per-thread configuration and support for log files. 

## Related Crates

- **[`mtlog-tokio`](https://crates.io/crates/mtlog-tokio)** - Use this instead if you're building async applications with tokio
- **[`mtlog-progress`](https://crates.io/crates/mtlog-progress)** - Add this for progress bars that work gracefully with mtlog

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mtlog = "0.2"
```

## Usage

### Basic Usage

```rust
use mtlog::logger_config;

let _guard = logger_config()
   .init_global();
log::info!("Hello, world!");
```

**⚠️ Warning**: the `_guard` must be kept alive for the entire duration of the program (or the thread when using `init_local`).

### Multi-threaded Logging

```rust
use mtlog::logger_config;

let _guard = logger_config()
    .with_name("main")
    .init_global();

log::info!("Hello, world from main thread!");

let handles: Vec<_> = (0..5).map(|i| {
    std::thread::spawn(move || {
       logger_config()
            .with_name(&format!("thread {i}"))
            .init_local();
        log::warn!("Hello, world from thread {i}!")
    })
}).collect();
for h in handles { h.join().unwrap(); }
```

### Logging to Files

Files can be used to log messages. The log file is created if it does not exist and appended to if it does. Different threads can log to different files. If no file is specified in local config, the global file is used.

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

## Configuration

### Flush Interval

For performance optimization, file logging uses batched writes that are flushed periodically. This can be configured via the `MTLOG_FLUSH_INTERVAL_MS` environment variable.

**Default**: 100ms

**Note**: Stdout logging always flushes immediately to ensure progress bars display correctly and logs appear in real-time.

## Documentation

For detailed API documentation, visit [docs.rs/mtlog](https://docs.rs/mtlog).

## License

MIT
