//! Service global context

use std::path::PathBuf;

use color_eyre::Result;

pub mod auth;
pub mod users;

use async_graphql::EmptySubscription;
pub use auth::Auth;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thiserror::Error;
pub use users::Users;

use crate::config;
use crate::mutation::Mutation;
use crate::query::Query;
use crate::service::Schema;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Cannot create in-memory database")]
    CannotCreateInMemoryDb,
    #[error("Cannot create SQLite database at {path}")]
    CannotCreateSqLite { path: PathBuf },
    #[error("Invalid SQLite path: {path}")]
    InvalidSQLitePath { path: PathBuf },
}

/// Context for GraphQL schema
#[derive(Clone)]
pub struct Context {
    /// Database access
    db: sqlx::SqlitePool,
}

impl Context {
    /// Context for testing purposes - using the in-memory SQLite database
    pub async fn test() -> Result<Self> {
        let opts = SqliteConnectOptions::new()
            .filename(":memory:")
            .create_if_missing(true)
            .foreign_keys(true)
            .shared_cache(true);

        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_lazy_with(opts);

        sqlx::migrate!("model/migrations").run(&db).await.unwrap();

        Ok(Self { db })
    }

    /// Context from configuration
    ///
    /// If the database is created in-memory, the migrations are being executed automatically. If database is
    /// file based migrations would be executed only if requested by configuration.
    pub async fn with_config(config: config::Database) -> Result<Self> {
        use config::Database::*;

        let db = match config {
            Memory { max_connections } => {
                let opts = SqliteConnectOptions::new()
                    .filename(":memory:")
                    .create_if_missing(true)
                    .foreign_keys(true)
                    .shared_cache(true);

                let pool = SqlitePoolOptions::new()
                    .max_connections(max_connections)
                    .connect_lazy_with(opts);

                sqlx::migrate!("model/migrations").run(&pool).await.unwrap();
                pool
            }

            SqLite {
                path,
                max_connections,
                migrate,
            } => {
                let path = path
                    .as_path()
                    .to_str()
                    .ok_or_else(|| Error::InvalidSQLitePath { path: path.clone() })?;

                let opts = SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(true);

                let pool = SqlitePoolOptions::new()
                    .max_connections(max_connections)
                    .connect_lazy_with(opts);

                if migrate {
                    sqlx::migrate!("model/migrations").run(&pool).await.unwrap();
                }

                pool
            }
        };

        Ok(Self { db })
    }

    /// Buids schema with attached context
    pub fn schema(&self) -> Schema {
        Schema::build(Query::new(), Mutation::new(), EmptySubscription)
            .data(self.clone())
            .finish()
    }

    /// Returns users accessor
    pub fn users(&self) -> Users<'_, sqlx::SqlitePool> {
        Users::new(&self.db)
    }

    /// Returns authorization accessor
    pub fn auth(&self) -> Auth<'_, sqlx::SqlitePool> {
        Auth::new(&self.db)
    }
}
