//! GraphQL async deckbuilder interface

use clap::Parser;
use color_eyre::Result;
use juniper::{EmptySubscription, RootNode};
use std::io::read_to_string;
use tracing::info;
use warp::Filter;

use crate::config::{Config, LogFormat};
use crate::context::Context;
use crate::mutation::Mutation;
use crate::opt::Opt;
use crate::query::Query;

mod config;
pub mod context;
mod mutation;
mod opt;
mod query;

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

#[derive(Debug)]
struct PlaygroundDisabled;

impl warp::reject::Reject for PlaygroundDisabled {}

/// Root GraphQL schema
type Schema = RootNode<Query, Mutation, EmptySubscription<Context>>;

/// Builds the schema
fn schema() -> Schema {
    RootNode::new(
        Query::new(),
        Mutation::new(),
        EmptySubscription::<Context>::new(),
    )
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

    let api = {
        let context = Context::new();
        let schema = schema();

        warp::post()
            .and(warp::path("api"))
            .and(juniper_warp::make_graphql_filter(
                schema,
                warp::any().map(move || context.clone()),
            ))
            .with(warp::trace(
                |info| tracing::info_span!("api", method=%info.method(), path=%info.path()),
            ))
    };

    let playground = warp::get().and(warp::path("pg"));
    let playground = match &config.graphiql {
        true => playground
            .and(juniper_warp::graphiql_filter("/api", None))
            .with(warp::trace(
                |info| tracing::info_span!("playground", method=%info.method(), path=%info.path()),
            ))
            .boxed(),
        false => playground
            .and_then(|| async { Err(warp::reject::custom(PlaygroundDisabled)) })
            .boxed(),
    };

    let routes = api.or(playground);

    info!(
        addr = ?config.host,
        "Service configured, serving"
    );
    warp::serve(routes).run(config.host).await;

    info!("Service stopped, tearing down");
    Ok(())
}
