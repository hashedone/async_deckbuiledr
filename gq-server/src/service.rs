//! Utilities for services building

use actix_web::web::{Data, ServiceConfig};
use actix_web::{HttpResponse, Result, get, post, web};
use async_graphql::EmptySubscription;
use async_graphql::http::GraphiQLSource;
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};

#[cfg(test)]
mod tests;

use crate::context::Context;
use crate::mutation::Mutation;
use crate::query::Query;

/// Root GraphQL schema
pub type Schema = async_graphql::Schema<Query, Mutation, EmptySubscription>;

/// Noop endpoint existing only to refresh session if needed
#[get("/refresh")]
async fn refresh() -> &'static str {
    ""
}

/// ActixWeb GraphQL endpoint
#[post("/api")]
async fn api(schema: web::Data<Schema>, request: GraphQLRequest) -> GraphQLResponse {
    schema.execute(request.into_inner()).await.into()
}

/// ActixWeb GraphQLi endpoint
#[get("/pg")]
async fn graphiql() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(GraphiQLSource::build().endpoint("/api").finish()))
}

/// Returns configuration function for the ActixWeb services
pub async fn configure(
    graphiql_enabled: bool,
    context: Context,
) -> color_eyre::Result<impl Fn(&mut web::ServiceConfig) + Clone> {
    Ok(move |cfg: &mut ServiceConfig| {
        cfg.app_data(Data::new(context.schema()))
            .service(api)
            .service(refresh);
        if graphiql_enabled {
            cfg.service(graphiql);
        }
    })
}
