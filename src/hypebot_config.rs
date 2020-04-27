use chrono_tz::Tz;
use config::{Config, ConfigError, File};
use serde::de::{self, Error, Visitor};
use serde::{Deserialize, Deserializer};
use serenity::prelude::TypeMapKey;
use std::fmt;

#[derive(Debug, Deserialize, Clone)]
pub struct HypeBotConfig {
    pub db_url: String,
    pub default_thumbnail_link: String,
    pub discord_key: String,
    pub prefix: String,
    pub event_channel: u64,
    pub event_roles: Vec<u64>,
    #[serde(deserialize_with = "from_tz_string")]
    pub event_timezone: Tz,
    pub log_path: String,
}

struct ConfigValueVisitor;
impl<'de> Visitor<'de> for ConfigValueVisitor {
    type Value = String;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Unable to parse timezone field.")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(s.to_string())
    }
}

fn from_tz_string<'de, D>(deserializer: D) -> Result<Tz, D::Error>
where
    D: Deserializer<'de>,
{
    let string = deserializer.deserialize_struct("Value", &["into_str"], ConfigValueVisitor)?;

    let tz: Tz = string.parse().ok().ok_or(D::Error::custom(
        "Unable to parse datetime, should be in format \"Country/City\"",
    ))?;

    Ok(tz)
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
