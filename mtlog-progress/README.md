# mtlog-progress

[![Crates.io](https://img.shields.io/crates/v/mtlog-progress.svg)](https://crates.io/crates/mtlog-progress)
[![Documentation](https://docs.rs/mtlog-progress/badge.svg)](https://docs.rs/mtlog-progress)

A progress bar implementation that works gracefully with mtlog's logger.
Multiple progress bars can run concurrently across multiple threads or tasks while staying in place when intermingled with regular logs.

## Related Crates

This crate requires a logger from the mtlog family:

- **[`mtlog`](https://crates.io/crates/mtlog)** - Use with standard multi-threaded applications
- **[`mtlog-tokio`](https://crates.io/crates/mtlog-tokio)** - Use with async tokio applications

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
mtlog-progress = "0.2"
mtlog = "0.2"  # or mtlog-tokio for async
```

## Iterator Progress Tracking

The easiest way to add progress tracking is using the `.progress()` method on iterators:

```rust
use mtlog::logger_config;
use mtlog_progress::ProgressIteratorExt;

let _guard = logger_config().init_global();

// For ExactSizeIterator - automatically detects length
(0..100)
    .progress("Processing items")
    .for_each(|i| {
        // Your work here
    });

// For any iterator - provide length manually
vec![1, 2, 3, 4, 5]
    .into_iter()
    .progress_with(5, "Items")
    .map(|x| x * 2)
    .collect::<Vec<_>>();

// With custom configuration
(0..1000)
    .progress("Long task")
    .with_min_timestep_ms(200.0)
    .with_min_percentage_change(1.0)
    .for_each(|_| { /* work */ });
```

The progress bar automatically finishes when the iterator is exhausted or dropped.

## Manual Progress Bar Usage

For more control, you can manually create and update progress bars:

### With std threads

```rust
use mtlog::logger_config;
use mtlog_progress::LogProgressBar;

let _guard = logger_config()
    .init_global();

let h = std::thread::spawn(|| {
    let pb = LogProgressBar::new(100, "My Progress Bar");
    for i in 0..100 {
        pb.inc(1);
        if i == 50 {
            log::info!("Halfway there!");
        }
    }
    pb.finish();
});
log::info!("This log goes below the progress bar");
h.join().unwrap();
```

### With tokio tasks

```toml
[dependencies]
mtlog-progress = "0.2"
mtlog-tokio = "0.2"
tokio = { version = "1", features = ["full"] }
```

```rust
use mtlog_tokio::logger_config;
use mtlog_progress::LogProgressBar;

#[tokio::main]
async fn main() {
    logger_config()
        .scope_global(async move {
            let h = tokio::spawn(async move {
                logger_config()
                    .scope_local(async move {
                        let pb = LogProgressBar::new(100, "My Progress Bar");
                        for i in 0..100 {
                            pb.inc(1);
                            if i == 50 {
                                log::info!("Halfway there!");
                            }
                        }
                        pb.finish();
                    }).await;
            });
            log::info!("This log goes below the progress bar");
            h.await.unwrap();
        }).await;
}
```

## Configuring Update Behavior

You can control how often the progress bar updates to reduce performance overhead:

```rust
use mtlog_progress::LogProgressBar;

let pb = LogProgressBar::new(1000000, "Large Task")
    .with_min_timestep_ms(200.0)           // Update at most every 200ms
    .with_min_percentage_change(1.0);      // Update at least every 1%

for _ in 0..1000000 {
    pb.inc(1);
}
pb.finish();
```

## Documentation

For detailed API documentation, visit [docs.rs/mtlog-progress](https://docs.rs/mtlog-progress).

## License

MIT
