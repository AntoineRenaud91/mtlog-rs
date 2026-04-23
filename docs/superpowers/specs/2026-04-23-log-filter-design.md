# Log Filter Design Spec

## Summary

Add regex-based log filtering to mtlog, allowing users to filter log messages by module target and message content using allow/deny rules. Deny rules take precedence over allow rules.

## Decisions

- **Filter targets**: Both message content and module target (`record.target()`)
- **Filter mode**: Allow-list and deny-list, deny takes precedence
- **Scope**: Global + per-thread/task overrides (replace semantics, not merge)
- **Regex engine**: `regex` crate (linear-time guarantees)
- **Output scope**: Same filters for both stdout and file
- **API style**: Separate `LogFilter` builder object passed to `ConfigBuilder`

## LogFilter Type

Lives in `mtlog-core`. New file: `mtlog-core/src/log_filter.rs`.

```rust
use regex::Regex;

#[derive(Clone)]
pub struct LogFilter {
    allow_targets: Vec<Regex>,
    deny_targets: Vec<Regex>,
    allow_messages: Vec<Regex>,
    deny_messages: Vec<Regex>,
}

impl Default for LogFilter { /* empty filter = allow all */ }

impl LogFilter {
    pub fn new() -> Self; // delegates to Default
    pub fn allow_target(mut self, pattern: &str) -> Result<Self, regex::Error>;
    pub fn deny_target(mut self, pattern: &str) -> Result<Self, regex::Error>;
    pub fn allow_message(mut self, pattern: &str) -> Result<Self, regex::Error>;
    pub fn deny_message(mut self, pattern: &str) -> Result<Self, regex::Error>;
    pub fn is_match(&self, target: &str, message: &str) -> bool;
}
```

### Filtering logic (`is_match`)

1. If any `deny_targets` matches target -> **reject**
2. If any `deny_messages` matches message -> **reject**
3. If `allow_targets` is non-empty and none match target -> **reject**
4. If `allow_messages` is non-empty and none match message -> **reject**
5. Otherwise -> **accept**

Empty filter = everything passes. Only deny = everything except denied. Only allow = only allowed. Both = allow then deny carves out exceptions.

## ConfigBuilder Integration

Both `mtlog` and `mtlog-tokio` get:

```rust
pub struct ConfigBuilder {
    // ... existing fields ...
    filter: Option<LogFilter>,
}

impl ConfigBuilder {
    pub fn with_filter(self, filter: LogFilter) -> Self;
}
```

`LogConfig` (internal) gets a matching `filter: Option<LogFilter>` field.

## Filtering Location

In `MTLogger::log()`, after the level check, before creating `LogMessage`:

```rust
if let Some(ref filter) = config.filter {
    if !filter.is_match(record.target(), &record.args().to_string()) {
        return;
    }
}
```

This avoids `Arc<LogMessage>` allocation and channel send for rejected messages. The `record.args().to_string()` cost only occurs when a filter is configured.

## Per-thread/Task Behavior

Follows existing replace semantics. If a thread/task sets its own config with a filter, that filter fully replaces the global one. If it sets a config without a filter, no filtering for that thread/task.

## Re-exports

- `mtlog-core` exports `LogFilter`
- `mtlog` re-exports: `pub use mtlog_core::LogFilter;`
- `mtlog-tokio` re-exports: `pub use mtlog_core::LogFilter;`

## Usage Examples

```rust
use mtlog::{logger_config, LogFilter};

// Silence noisy dependencies
let filter = LogFilter::new()
    .deny_target("^hyper")?
    .deny_target("^reqwest")?
    .deny_message("heartbeat")?;

let _guard = logger_config()
    .with_filter(filter)
    .init_global();

// Per-thread: DB worker only wants DB logs
std::thread::spawn(|| {
    let filter = LogFilter::new()
        .allow_target("^myapp::db").unwrap();

    logger_config()
        .with_name("db-worker")
        .with_filter(filter)
        .init_local();

    log::info!("This shows - target matches");
});
```

## Testing Strategy

- Unit tests for `LogFilter::is_match()` covering all logic branches:
  - Empty filter passes everything
  - Deny-only rejects matches, passes rest
  - Allow-only passes matches, rejects rest
  - Combined: deny overrides allow
  - Invalid regex returns `Err`
- Integration tests in `mtlog` and `mtlog-tokio`:
  - Global filter suppresses matching messages
  - Per-thread/task filter replaces global
  - Filter + level filter work together

## Version Bumps

| Crate | Current | New | Reason |
|-------|---------|-----|--------|
| mtlog-core | 0.2.0 | 0.3.0 | New public type, new `regex` dependency |
| mtlog | 0.3.0 | 0.4.0 | New API method, re-export, dep bump |
| mtlog-tokio | 0.3.0 | 0.4.0 | Same as mtlog |
| mtlog-progress | 0.2.1 | 0.2.2 | Only bumps mtlog-core dep |

## Release Sequence

1. Implement feature across all crates
2. Version bumps in all `Cargo.toml` files
3. Commit all changes
4. Push to remote
5. Git tags: `mtlog-core-v0.3.0`, `mtlog-v0.4.0`, `mtlog-tokio-v0.4.0`, `mtlog-progress-v0.2.2`
6. Push tags
7. `cargo publish` in dependency order: mtlog-core -> mtlog -> mtlog-tokio -> mtlog-progress

## Files to Create/Modify

### New files
- `mtlog-core/src/log_filter.rs` — `LogFilter` type and tests

### Modified files
- `mtlog-core/Cargo.toml` — add `regex` dependency, bump version
- `mtlog-core/src/lib.rs` — add `mod log_filter; pub use log_filter::LogFilter;`
- `mtlog/Cargo.toml` — bump version, bump mtlog-core dep
- `mtlog/src/lib.rs` — add `LogFilter` re-export, `filter` field to `LogConfig`/`ConfigBuilder`, filtering in `MTLogger::log()`
- `mtlog-tokio/Cargo.toml` — bump version, bump mtlog-core dep
- `mtlog-tokio/src/lib.rs` — same changes as mtlog
- `mtlog-progress/Cargo.toml` — bump version, bump mtlog-core dep
