//! Utilities for services building

use juniper::{EmptySubscription, RootNode};
use warp::Filter;
use warp::reject::Rejection;
use warp::reply::Reply;

#[cfg(test)]
mod tests;

use crate::context::Context;
use crate::mutation::Mutation;
use crate::query::Query;

/// Root GraphQL schema
pub type Schema = RootNode<Query, Mutation, EmptySubscription<Context>>;

/// Builds the schema
fn schema() -> Schema {
    RootNode::new(
        Query::new(),
        Mutation::new(),
        EmptySubscription::<Context>::new(),
    )
}

/// Builds API filter without tracing
pub fn api() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let context = Context::new();
    let schema = schema();

    warp::post()
        .and(warp::path("api"))
        .and(juniper_warp::make_graphql_filter(
            schema,
            warp::any().map(move || context.clone()),
        ))
}

/// Builds api filter with tracing enabled
pub fn api_traced() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    api().with(warp::trace(
        move |info| tracing::info_span!("api", method=%info.method(), path=%info.path()),
    ))
}

/// Builds playground filter if it is supposed to be enabled.
///
/// If service is supposed be enabled, the `/pg` endpoint is still enabled, but rejecting all the
/// trafic.
pub fn playground() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::get()
        .and(warp::path("pg"))
        .and(juniper_warp::graphiql_filter("/api", None))
}

/// Builds playground filter if it is supposed to be enabled with tracing attached
pub fn playground_traced() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    playground().with(warp::trace(
        |info| tracing::info_span!("playground", method=%info.method(), path=%info.path()),
    ))
}
