use std::sync::LazyLock;

use derive_from_env::FromEnv;

#[derive(FromEnv)]
#[from_env(prefix = "MTLOG")]
#[allow(non_snake_case)]
pub struct MTLogConfig {
    #[from_env(default = "100")]
    pub FLUSH_INTERVAL_MS: u64,
}

pub static MTLOG_CONFIG: LazyLock<MTLogConfig> = LazyLock::new(|| MTLogConfig::from_env().unwrap());
