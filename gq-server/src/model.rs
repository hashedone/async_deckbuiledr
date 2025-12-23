//! Service global context

use std::path::PathBuf;

use color_eyre::Result;

pub mod auth;
pub mod users;

use async_graphql::EmptySubscription;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use thiserror::Error;

use crate::config;
use crate::model::auth::Session;
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
pub struct Model {
    /// Database access
    db: sqlx::SqlitePool,
}

impl Model {
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

    /// Accesses the DB pool
    pub fn db(&self) -> &sqlx::SqlitePool {
        &self.db
    }

    /// Performs cleanup on the model
    pub async fn cleanup(&self) -> Result<()> {
        Session::cleanup(&self.db).await
    }
}
