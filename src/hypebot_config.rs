use config::{ConfigError, Config, File};
use serenity::prelude::TypeMapKey;

#[derive(Debug, Deserialize)]
pub struct HypeBotConfig {
    pub db_url: String,
    pub default_thumbnail_link: String,
    pub discord_key: String,
    pub prefix: String,
    pub event_channel: u64,
    pub event_roles: Vec<u64>,
    pub event_timezone: String,
}

impl HypeBotConfig {
    pub fn new(config_path: &str) -> Result<Self, ConfigError> {
        let mut cfg = Config::new();
        cfg.merge(File::with_name(config_path))?;

        cfg.try_into()
    }
}

impl TypeMapKey for HypeBotConfig {
    type Value = HypeBotConfig;
}
