//! GraphQL async deckbuilder interface

use color_eyre::Result;
use juniper::{EmptySubscription, RootNode};
use tracing::info;
use warp::Filter;

use crate::context::Context;
use crate::mutation::Mutation;
use crate::query::Query;

mod context;
mod mutation;
mod query;

/// Initializes tracing collection
fn setup_tracing() {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{EnvFilter, fmt};

    let fmt_layer = fmt::layer();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[tokio::main]
async fn main() -> Result<()> {
    setup_tracing();
    color_eyre::install()?;

    info!("Tracing initialized, setting up a service");

    // Defining routes
    let schema = RootNode::new(Query, Mutation, EmptySubscription::<Context>::new());

    let api = warp::post()
        .and(warp::path("api"))
        .and(juniper_warp::make_graphql_filter(
            schema,
            warp::any().map(|| Context),
        ))
        .with(warp::trace(
            |info| tracing::info_span!("api", method=%info.method(), path=%info.path()),
        ));

    let playground = warp::get()
        .and(warp::path("pg"))
        .and(juniper_warp::graphiql_filter("/api", None))
        .with(warp::trace(
            |info| tracing::info_span!("playground", method=%info.method(), path=%info.path()),
        ));

    let routes = api.or(playground);

    info!(
        addr = "127.0.0.1",
        port = 3030,
        "Service configured, serving"
    );
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;

    info!("Service stopped, tearing down");
    Ok(())
}
