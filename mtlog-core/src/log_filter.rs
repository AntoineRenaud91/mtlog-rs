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
#[derive(Clone, Debug, Default)]
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
            .allow_target("^myapp")
            .unwrap()
            .deny_target("^myapp::noisy")
            .unwrap();
        assert!(filter.is_match("myapp::db", "hello"));
        assert!(!filter.is_match("myapp::noisy", "spam"));
        assert!(!filter.is_match("hyper", "hello"));
    }

    #[test]
    fn deny_overrides_allow_message() {
        let filter = LogFilter::new()
            .allow_message("important")
            .unwrap()
            .deny_message("ignore_this")
            .unwrap();
        assert!(filter.is_match("any", "important update"));
        assert!(!filter.is_match("any", "important but ignore_this"));
        assert!(!filter.is_match("any", "routine check"));
    }

    #[test]
    fn combined_target_and_message_filters() {
        let filter = LogFilter::new()
            .allow_target("^myapp")
            .unwrap()
            .deny_target("^myapp::noisy")
            .unwrap()
            .allow_message("query")
            .unwrap()
            .deny_message("heartbeat")
            .unwrap();

        assert!(filter.is_match("myapp::db", "query executed"));
        assert!(!filter.is_match("myapp::noisy", "query executed"));
        assert!(!filter.is_match("myapp::db", "heartbeat query"));
        assert!(!filter.is_match("hyper", "query executed"));
        assert!(!filter.is_match("myapp::db", "connection opened"));
    }

    #[test]
    fn multiple_allow_targets_any_match_passes() {
        let filter = LogFilter::new()
            .allow_target("^myapp::db")
            .unwrap()
            .allow_target("^myapp::api")
            .unwrap();
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
