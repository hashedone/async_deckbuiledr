//! Service configuration

use std::net::SocketAddr;
use std::path::PathBuf;

use serde::{Deserialize, Deserializer};
use tracing_subscriber::filter::Directive;

/// Logging output format
#[derive(Debug, Clone, Copy, Deserialize, Default)]
pub enum LogFormat {
    #[default]
    Compact,
    Pretty,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Logging {
    /// Additional filtering directives
    #[serde(default, deserialize_with = "Logging::deserialize_filters")]
    pub filters: Vec<Directive>,

    /// Logging format
    #[serde(default)]
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

/// Database configuration
#[derive(Debug, Clone, Deserialize)]
pub enum Database {
    /// In-memory SQLite database
    Memory {
        /// Max number of concurrent connections to the DB
        #[serde(default = "Database::default_max_connections")]
        max_connections: u32,
    },
    /// FS SQLite database
    SqLite {
        /// Database file path
        path: PathBuf,
        /// Max number of concurrent connections to the DB
        #[serde(default = "Database::default_max_connections")]
        max_connections: u32,
        /// If true, the migrations will be executing when creating the pool. Not advised for production use,
        /// but can be helpful for testing/development.
        #[serde(default)]
        migrate: bool,
    },
}

impl Default for Database {
    fn default() -> Self {
        Self::SqLite {
            path: "dbgq/data/db.sqlite".to_owned().into(),
            max_connections: Database::default_max_connections(),
            migrate: false,
        }
    }
}

impl Database {
    fn default_max_connections() -> u32 {
        5
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

    /// Database configuration
    #[serde(default)]
    pub db: Database,
}

impl Config {
    fn default_host() -> SocketAddr {
        ([127, 0, 0, 1], 3030).into()
    }
}
