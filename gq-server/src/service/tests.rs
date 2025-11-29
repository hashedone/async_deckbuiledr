//! Services integration tests

use serde_json::Value;
use warp::Filter;
use warp::reject::Rejection;
use warp::reply::Reply;
use warp::test::{RequestBuilder, request};

use crate::service::api;

mod users;

/// Setup all services for testing
fn setup() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    api()
}

/// Prepares graphql API request
fn gql(query: &str, variables: Value) -> RequestBuilder {
    let query: String = query.lines().collect();
    let body = format!(r#"{{ "query": "{query}", "variables": {variables} }}"#);
    request().method("POST").path("/api").body(dbg!(body))
}
