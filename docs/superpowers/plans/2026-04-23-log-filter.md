# Log Filter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add regex-based log filtering to mtlog with allow/deny rules on message content and module targets.

**Architecture:** A `LogFilter` type in mtlog-core holds compiled regex patterns in four vectors (allow_targets, deny_targets, allow_messages, deny_messages). It's passed into `ConfigBuilder` via `with_filter()` and checked in `MTLogger::log()` before message allocation. Deny rules take precedence over allow rules.

**Tech Stack:** Rust, `regex` crate, existing mtlog workspace (mtlog-core, mtlog, mtlog-tokio, mtlog-progress)

---

## File Map

| Action | File | Responsibility |
|--------|------|----------------|
| Create | `mtlog-core/src/log_filter.rs` | `LogFilter` type, builder methods, `is_match` logic, unit tests |
| Modify | `mtlog-core/Cargo.toml` | Add `regex` dep, bump to 0.3.0 |
| Modify | `mtlog-core/src/lib.rs` | Add `mod log_filter` + re-export `LogFilter` |
| Modify | `mtlog/Cargo.toml` | Bump to 0.4.0, bump mtlog-core dep to 0.3.0 |
| Modify | `mtlog/src/lib.rs` | Add `LogFilter` re-export, `filter` field, `with_filter()`, filtering in `log()` |
| Modify | `mtlog-tokio/Cargo.toml` | Bump to 0.4.0, bump mtlog-core dep to 0.3.0 |
| Modify | `mtlog-tokio/src/lib.rs` | Same changes as mtlog |
| Modify | `mtlog-progress/Cargo.toml` | Bump to 0.2.2 |

---

### Task 1: Create LogFilter with unit tests in mtlog-core

**Files:**
- Modify: `mtlog-core/Cargo.toml`
- Create: `mtlog-core/src/log_filter.rs`
- Modify: `mtlog-core/src/lib.rs`

- [ ] **Step 1: Add `regex` dependency to mtlog-core**

In `mtlog-core/Cargo.toml`, add under `[dependencies]`:

```toml
regex = "1"
```

Do NOT bump the version yet (that happens in Task 4).

- [ ] **Step 2: Create `mtlog-core/src/log_filter.rs` with full implementation and tests**

```rust
use regex::Regex;

/// A composable log filter supporting allow/deny rules on message content and module targets.
///
/// Deny rules always take precedence over allow rules.
///
/// # Filtering logic
/// 1. If any deny target matches → reject
/// 2. If any deny message matches → reject
/// 3. If allow targets is non-empty and none match → reject
/// 4. If allow messages is non-empty and none match → reject
/// 5. Otherwise → accept
///
/// An empty filter accepts everything.
///
/// # Example
/// ```
/// use mtlog_core::LogFilter;
///
/// let filter = LogFilter::new()
///     .deny_target("^hyper").unwrap()
///     .allow_message("important").unwrap();
///
/// assert!(!filter.is_match("hyper::client", "important request"));
/// assert!(filter.is_match("myapp::db", "important query"));
/// assert!(!filter.is_match("myapp::db", "routine check"));
/// ```
#[derive(Clone, Default)]
pub struct LogFilter {
    allow_targets: Vec<Regex>,
    deny_targets: Vec<Regex>,
    allow_messages: Vec<Regex>,
    deny_messages: Vec<Regex>,
}

impl LogFilter {
    /// Creates an empty filter that accepts all log records.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allow log records whose target (module path) matches this regex.
    /// When at least one allow_target is set, only matching targets pass.
    pub fn allow_target(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.allow_targets.push(Regex::new(pattern)?);
        Ok(self)
    }

    /// Deny log records whose target matches this regex. Takes precedence over allow.
    pub fn deny_target(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.deny_targets.push(Regex::new(pattern)?);
        Ok(self)
    }

    /// Allow log records whose message content matches this regex.
    /// When at least one allow_message is set, only matching messages pass.
    pub fn allow_message(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.allow_messages.push(Regex::new(pattern)?);
        Ok(self)
    }

    /// Deny log records whose message content matches this regex. Takes precedence over allow.
    pub fn deny_message(mut self, pattern: &str) -> Result<Self, regex::Error> {
        self.deny_messages.push(Regex::new(pattern)?);
        Ok(self)
    }

    /// Returns `true` if the record passes the filter.
    pub fn is_match(&self, target: &str, message: &str) -> bool {
        if self.deny_targets.iter().any(|r| r.is_match(target)) {
            return false;
        }
        if self.deny_messages.iter().any(|r| r.is_match(message)) {
            return false;
        }
        if !self.allow_targets.is_empty() && !self.allow_targets.iter().any(|r| r.is_match(target))
        {
            return false;
        }
        if !self.allow_messages.is_empty()
            && !self.allow_messages.iter().any(|r| r.is_match(message))
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_filter_accepts_everything() {
        let filter = LogFilter::new();
        assert!(filter.is_match("any::module", "any message"));
        assert!(filter.is_match("", ""));
    }

    #[test]
    fn deny_target_rejects_match() {
        let filter = LogFilter::new().deny_target("^hyper").unwrap();
        assert!(!filter.is_match("hyper::client", "hello"));
        assert!(!filter.is_match("hyper", "hello"));
        assert!(filter.is_match("myapp::hyper_wrapper", "hello"));
        assert!(filter.is_match("myapp", "hello"));
    }

    #[test]
    fn deny_message_rejects_match() {
        let filter = LogFilter::new().deny_message("heartbeat").unwrap();
        assert!(!filter.is_match("myapp", "sending heartbeat ping"));
        assert!(filter.is_match("myapp", "processing query"));
    }

    #[test]
    fn allow_target_only_passes_matches() {
        let filter = LogFilter::new().allow_target("^myapp::db").unwrap();
        assert!(filter.is_match("myapp::db", "query"));
        assert!(filter.is_match("myapp::db::pool", "connect"));
        assert!(!filter.is_match("myapp::api", "request"));
        assert!(!filter.is_match("hyper", "connect"));
    }

    #[test]
    fn allow_message_only_passes_matches() {
        let filter = LogFilter::new().allow_message("query.*SELECT").unwrap();
        assert!(filter.is_match("any", "query: SELECT * FROM users"));
        assert!(!filter.is_match("any", "query: INSERT INTO users"));
        assert!(!filter.is_match("any", "heartbeat"));
    }

    #[test]
    fn deny_overrides_allow_target() {
        let filter = LogFilter::new()
            .allow_target("^myapp").unwrap()
            .deny_target("^myapp::noisy").unwrap();
        assert!(filter.is_match("myapp::db", "hello"));
        assert!(!filter.is_match("myapp::noisy", "spam"));
        assert!(!filter.is_match("hyper", "hello"));
    }

    #[test]
    fn deny_overrides_allow_message() {
        let filter = LogFilter::new()
            .allow_message("important").unwrap()
            .deny_message("ignore_this").unwrap();
        assert!(filter.is_match("any", "important update"));
        assert!(!filter.is_match("any", "important but ignore_this"));
        assert!(!filter.is_match("any", "routine check"));
    }

    #[test]
    fn combined_target_and_message_filters() {
        let filter = LogFilter::new()
            .allow_target("^myapp").unwrap()
            .deny_target("^myapp::noisy").unwrap()
            .allow_message("query").unwrap()
            .deny_message("heartbeat").unwrap();

        // Passes: target matches allow, message matches allow, no deny hit
        assert!(filter.is_match("myapp::db", "query executed"));
        // Fails: target denied
        assert!(!filter.is_match("myapp::noisy", "query executed"));
        // Fails: message denied
        assert!(!filter.is_match("myapp::db", "heartbeat query"));
        // Fails: target not in allow list
        assert!(!filter.is_match("hyper", "query executed"));
        // Fails: message not in allow list
        assert!(!filter.is_match("myapp::db", "connection opened"));
    }

    #[test]
    fn multiple_allow_targets_any_match_passes() {
        let filter = LogFilter::new()
            .allow_target("^myapp::db").unwrap()
            .allow_target("^myapp::api").unwrap();
        assert!(filter.is_match("myapp::db", "hello"));
        assert!(filter.is_match("myapp::api", "hello"));
        assert!(!filter.is_match("myapp::auth", "hello"));
    }

    #[test]
    fn invalid_regex_returns_error() {
        assert!(LogFilter::new().allow_target("[invalid").is_err());
        assert!(LogFilter::new().deny_target("[invalid").is_err());
        assert!(LogFilter::new().allow_message("[invalid").is_err());
        assert!(LogFilter::new().deny_message("[invalid").is_err());
    }
}
```

- [ ] **Step 3: Wire up the module in `mtlog-core/src/lib.rs`**

Add these two lines:

```rust
mod log_filter;
pub use log_filter::LogFilter;
```

The full file becomes:

```rust
//! # mtlog-core
//! Core utilities for mtlog - shared logging infrastructure.

mod config;
mod log_filter;
mod log_rotation;
mod log_writer;
mod utils;

pub use config::MTLOG_CONFIG;
pub use log_filter::LogFilter;
pub use log_rotation::{
    FileLogger, LogFileSizeRotation, LogFileTimeRotation, SizeRotationConfig, TimeRotationConfig,
};
pub use log_writer::{LogFile, LogStdout, LogWriter};
pub use utils::{
    LogMessage, LogSender, LoggerGuard, spawn_log_thread_file, spawn_log_thread_stdout,
};
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p mtlog-core`

Expected: All new tests pass (empty_filter, deny_target, deny_message, allow_target, allow_message, deny_overrides_allow_target, deny_overrides_allow_message, combined, multiple_allow, invalid_regex).

- [ ] **Step 5: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -p mtlog-core`

Expected: No warnings.

- [ ] **Step 6: Commit**

```bash
git add mtlog-core/Cargo.toml mtlog-core/src/log_filter.rs mtlog-core/src/lib.rs
git commit -m "feat(mtlog-core): add LogFilter with regex-based allow/deny rules"
```

---

### Task 2: Integrate LogFilter into mtlog (sync crate)

**Files:**
- Modify: `mtlog/src/lib.rs`

- [ ] **Step 1: Add `LogFilter` re-export, `filter` field, and `with_filter()` method**

In `mtlog/src/lib.rs`, make these changes:

**a) Add re-export** — change the `pub use` line:

```rust
pub use mtlog_core::{LogFilter, LoggerGuard, SizeRotationConfig, TimeRotationConfig};
```

**b) Add `filter` field to `LogConfig`:**

```rust
struct LogConfig {
    sender_file: Option<Arc<LogSender>>,
    sender_stdout: Option<Arc<LogSender>>,
    name: Option<String>,
    level: LevelFilter,
    filter: Option<LogFilter>,
}
```

**c) Update `GLOBAL_LOG_CONFIG` initialization** to include `filter: None`:

```rust
static GLOBAL_LOG_CONFIG: LazyLock<Arc<RwLock<LogConfig>>> = LazyLock::new(|| {
    log::set_boxed_logger(Box::new(MTLogger)).unwrap();
    log::set_max_level(LevelFilter::Info);
    let sender = spawn_log_thread_stdout(LogStdout::default());
    Arc::new(RwLock::new(LogConfig {
        sender_stdout: Some(Arc::new(sender)),
        sender_file: None,
        name: None,
        level: LevelFilter::Info,
        filter: None,
    }))
});
```

**d) Add filtering in `MTLogger::log()`** — after the level check:

```rust
fn log(&self, record: &log::Record) {
    LOG_CONFIG.with(|local_config| {
        let local_config = local_config.borrow();
        let config = if local_config.is_some() {
            local_config.as_ref().unwrap()
        } else {
            &*GLOBAL_LOG_CONFIG.read().unwrap()
        };
        let level = record.level();
        if level > config.level {
            return;
        }
        if let Some(ref filter) = config.filter {
            if !filter.is_match(record.target(), &record.args().to_string()) {
                return;
            }
        }
        let log_message = Arc::new(LogMessage {
            level,
            name: config.name.clone(),
            message: record.args().to_string(),
        });
        if let Some(sender) = &config.sender_stdout {
            sender.send(log_message.clone()).ok();
        }
        if let Some(sender) = &config.sender_file {
            sender.send(log_message).ok();
        }
    });
}
```

**e) Add `filter` field to `ConfigBuilder`:**

```rust
pub struct ConfigBuilder {
    log_file: Option<FileLogger>,
    no_stdout: bool,
    no_file: bool,
    log_level: LevelFilter,
    name: Option<String>,
    filter: Option<LogFilter>,
}
```

**f) Update `Default` impl for `ConfigBuilder`:**

```rust
impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            log_file: None,
            no_stdout: false,
            no_file: false,
            log_level: LevelFilter::Info,
            name: None,
            filter: None,
        }
    }
}
```

**g) Update `ConfigBuilder::build()` to pass filter through:**

```rust
fn build(self) -> LogConfig {
    let Self {
        log_file,
        no_stdout,
        no_file,
        log_level,
        name,
        filter,
    } = self;
    let sender_file = if no_file {
        None
    } else if let Some(log_file) = log_file {
        let sender = spawn_log_thread_file(log_file);
        Some(Arc::new(sender))
    } else {
        GLOBAL_LOG_CONFIG.read().unwrap().sender_file.clone()
    };
    let sender_stdout = if no_stdout {
        None
    } else {
        GLOBAL_LOG_CONFIG.read().unwrap().sender_stdout.clone()
    };
    LogConfig {
        sender_file,
        sender_stdout,
        name,
        level: log_level,
        filter,
    }
}
```

**h) Add `with_filter()` method** on `ConfigBuilder` (after `maybe_with_name`):

```rust
/// Sets a log filter for pattern-based message filtering.
/// See [`LogFilter`] for details.
pub fn with_filter(self, filter: LogFilter) -> Self {
    Self {
        filter: Some(filter),
        ..self
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mtlog`

Expected: All existing tests pass. (Integration tests for filtering will be in Task 2 step 3 if tests exist, otherwise behavior is verified by mtlog-core unit tests.)

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -p mtlog`

Expected: No warnings.

- [ ] **Step 4: Commit**

```bash
git add mtlog/src/lib.rs
git commit -m "feat(mtlog): integrate LogFilter into ConfigBuilder and MTLogger"
```

---

### Task 3: Integrate LogFilter into mtlog-tokio (async crate)

**Files:**
- Modify: `mtlog-tokio/src/lib.rs`

- [ ] **Step 1: Apply the same changes as Task 2, adapted for tokio**

**a) Add re-export:**

```rust
pub use mtlog_core::{LogFilter, SizeRotationConfig, TimeRotationConfig};
```

**b) Add `filter` field to `LogConfig`:**

```rust
#[derive(Clone)]
struct LogConfig {
    sender_file: Option<Arc<LogSender>>,
    sender_stdout: Option<Arc<LogSender>>,
    name: Option<String>,
    level: LevelFilter,
    filter: Option<LogFilter>,
}
```

**c) Update `GLOBAL_LOG_CONFIG` initialization** to include `filter: None`:

```rust
static GLOBAL_LOG_CONFIG: LazyLock<Arc<RwLock<LogConfig>>> = LazyLock::new(|| {
    log::set_boxed_logger(Box::new(MTLogger)).unwrap();
    log::set_max_level(LevelFilter::Info);
    let sender = spawn_log_thread_stdout(LogStdout::default());
    Arc::new(RwLock::new(LogConfig {
        sender_stdout: Some(Arc::new(sender)),
        sender_file: None,
        name: None,
        level: LevelFilter::Info,
        filter: None,
    }))
});
```

**d) Add filtering in `MTLogger::log()`:**

```rust
fn log(&self, record: &log::Record) {
    LOG_CONFIG.with(|config| {
        let level = record.level();
        if level > config.level {
            return;
        }
        if let Some(ref filter) = config.filter {
            if !filter.is_match(record.target(), &record.args().to_string()) {
                return;
            }
        }
        let log_message = Arc::new(LogMessage {
            level,
            name: config.name.clone(),
            message: record.args().to_string(),
        });
        if let Some(sender) = &config.sender_stdout {
            sender
                .send(log_message.clone())
                .expect("Unable to send log message to stdout logging thread");
        }
        if let Some(sender) = &config.sender_file {
            sender
                .send(log_message)
                .expect("Unable to send log message to file logging thread");
        }
    });
}
```

**e) Add `filter` to `ConfigBuilder`, its `Default`, `build()`, and `with_filter()`:**

```rust
pub struct ConfigBuilder {
    log_file: Option<FileLogger>,
    no_stdout: bool,
    no_file: bool,
    log_level: LevelFilter,
    name: Option<String>,
    filter: Option<LogFilter>,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self {
            log_file: None,
            no_stdout: false,
            no_file: false,
            log_level: LevelFilter::Info,
            name: None,
            filter: None,
        }
    }
}
```

In `build()`:

```rust
fn build(self) -> LogConfig {
    let Self {
        log_file,
        no_stdout,
        no_file,
        log_level,
        name,
        filter,
    } = self;
    let sender_file = if no_file {
        None
    } else if let Some(log_file) = log_file {
        let sender = spawn_log_thread_file(log_file);
        Some(Arc::new(sender))
    } else {
        GLOBAL_LOG_CONFIG.read().unwrap().sender_file.clone()
    };
    let sender_stdout = if no_stdout {
        None
    } else {
        GLOBAL_LOG_CONFIG.read().unwrap().sender_stdout.clone()
    };
    LogConfig {
        sender_file,
        sender_stdout,
        name,
        level: log_level,
        filter,
    }
}
```

Add method:

```rust
/// Sets a log filter for pattern-based message filtering.
/// See [`LogFilter`] for details.
pub fn with_filter(self, filter: LogFilter) -> Self {
    Self {
        filter: Some(filter),
        ..self
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p mtlog-tokio`

Expected: All existing tests pass.

- [ ] **Step 3: Run clippy and fmt**

Run: `cargo fmt && cargo clippy -p mtlog-tokio`

Expected: No warnings.

- [ ] **Step 4: Commit**

```bash
git add mtlog-tokio/src/lib.rs
git commit -m "feat(mtlog-tokio): integrate LogFilter into ConfigBuilder and MTLogger"
```

---

### Task 4: Version bumps

**Files:**
- Modify: `mtlog-core/Cargo.toml` (0.2.0 -> 0.3.0)
- Modify: `mtlog/Cargo.toml` (0.3.0 -> 0.4.0, mtlog-core dep 0.2.0 -> 0.3.0)
- Modify: `mtlog-tokio/Cargo.toml` (0.3.0 -> 0.4.0, mtlog-core dep 0.2.0 -> 0.3.0)
- Modify: `mtlog-progress/Cargo.toml` (0.2.1 -> 0.2.2)

- [ ] **Step 1: Bump mtlog-core**

In `mtlog-core/Cargo.toml`, change:
```toml
version = "0.3.0"
```

- [ ] **Step 2: Bump mtlog**

In `mtlog/Cargo.toml`, change:
```toml
version = "0.4.0"
```
And update the mtlog-core dependency:
```toml
mtlog-core = { version="0.3.0", path = "../mtlog-core" }
```

- [ ] **Step 3: Bump mtlog-tokio**

In `mtlog-tokio/Cargo.toml`, change:
```toml
version = "0.4.0"
```
And update the mtlog-core dependency:
```toml
mtlog-core = {version="0.3.0", path = "../mtlog-core" }
```

- [ ] **Step 4: Bump mtlog-progress**

In `mtlog-progress/Cargo.toml`, change:
```toml
version = "0.2.2"
```

- [ ] **Step 5: Run full test suite**

Run: `cargo test`

Expected: All tests across all crates pass.

- [ ] **Step 6: Run clippy and fmt on all crates**

Run: `cargo fmt --check && cargo clippy`

Expected: No warnings, no formatting issues.

- [ ] **Step 7: Commit**

```bash
git add mtlog-core/Cargo.toml mtlog/Cargo.toml mtlog-tokio/Cargo.toml mtlog-progress/Cargo.toml
git commit -m "chore: bump versions for log filter release"
```

---

### Task 5: Push, tag, and publish

- [ ] **Step 1: Push to remote**

```bash
git push
```

- [ ] **Step 2: Create and push tags**

```bash
git tag mtlog-core-v0.3.0
git tag mtlog-v0.4.0
git tag mtlog-tokio-v0.4.0
git tag mtlog-progress-v0.2.2
git push --tags
```

- [ ] **Step 3: Publish crates in dependency order**

```bash
cargo publish -p mtlog-core
```

Wait for it to be available on crates.io (~30 seconds), then:

```bash
cargo publish -p mtlog
cargo publish -p mtlog-tokio
cargo publish -p mtlog-progress
```
