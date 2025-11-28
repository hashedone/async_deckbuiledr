//! GraphQL async deckbuilder interface

use color_eyre::Result;
use juniper::{EmptySubscription, RootNode};
use structopt::StructOpt;
use tokio::fs::read_to_string;
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

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let config = read_to_string(&opt.config).await?;
    let config: Config = toml::from_str(&config)?;

    setup_tracing(config.logging);
    color_eyre::install()?;

    info!(
        config = ?opt.config,
        "Tracing initialized, setting up a service"
    );

    // Defining routes
    let context = Context::new();
    let schema = RootNode::new(Query, Mutation, EmptySubscription::<Context>::new());

    let api = {
        let context = context.clone();

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
