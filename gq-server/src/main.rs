//! GraphQL async deckbuilder interface

use actix_web::{App, HttpServer};
use clap::Parser;
use color_eyre::Result;
use std::io::read_to_string;
use tracing::info;
use tracing_actix_web::TracingLogger;

use crate::config::{Config, LogFormat};
use crate::context::Model;
use crate::opt::Opt;

mod config;
pub mod context;
mod mutation;
mod opt;
mod query;
mod service;

/// Initializes tracing collection
fn setup_tracing(config: config::Logging) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{EnvFilter, fmt};

    let fmt_layer = match config.format {
        LogFormat::Pretty => fmt::layer().pretty().boxed(),
        LogFormat::Compact => fmt::layer().compact().boxed(),
    };

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    let filter_layer = config
        .filters
        .into_iter()
        .fold(filter_layer, |layer, filter| layer.add_directive(filter));

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    let Opt {
        config: mut config_file,
    } = Opt::parse();

    let config = read_to_string(&mut config_file)?;
    let config: Config = toml::from_str(&config)?;

    setup_tracing(config.logging);
    color_eyre::install()?;

    info!(
        config = ?config_file.path().path(),
        "Tracing initialized, setting up a service"
    );

    let graphiql_enabled = config.graphiql;
    let context = Model::with_config(config.db).await?;
    let service_config = service::configure(graphiql_enabled, context).await?;
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .configure(service_config.clone())
    })
    .bind(config.host)?
    .run()
    .await?;

    info!("Service stopped, tearing down");
    Ok(())
}
