//! Service configuration

use std::net::SocketAddr;

use serde::{Deserialize, Deserializer};
use tracing_subscriber::filter::Directive;

/// Logging output format
#[derive(Debug, Clone, Copy, Deserialize)]
pub enum LogFormat {
    Compact,
    Pretty,
}

impl Default for LogFormat {
    fn default() -> Self {
        Self::Compact
    }
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Logging {
    /// Additional filtering directives
    #[serde(deserialize_with = "Logging::deserialize_filters")]
    pub filters: Vec<Directive>,

    /// Logging format
    pub format: LogFormat,
}

impl Logging {
    fn deserialize_filters<'de, D>(deserializer: D) -> Result<Vec<Directive>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let dirs: Vec<String> = Deserialize::deserialize(deserializer)?;
        dirs.into_iter()
            .map(|dir| dir.parse().map_err(serde::de::Error::custom))
            .collect()
    }
}

/// Top level service configuration
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Address where to host the service
    #[serde(default = "Config::default_host")]
    pub host: SocketAddr,

    /// Enables GraphiQL endpoint
    #[serde(default)]
    pub graphiql: bool,

    /// Logging configuration
    #[serde(default)]
    pub logging: Logging,
}

impl Config {
    fn default_host() -> SocketAddr {
        ([127, 0, 0, 1], 3030).into()
    }
}
