//! Utilities for services building

use actix_web::error::ErrorInternalServerError;
use actix_web::web::{Data, ServiceConfig};
use actix_web::{HttpMessage, delete, middleware};
use actix_web::{HttpRequest, HttpResponse, Result, get, post, web};
use async_graphql::EmptySubscription;
use async_graphql::http::GraphiQLSource;
use async_graphql_actix_web::{GraphQLRequest, GraphQLResponse};

#[cfg(test)]
mod tests;

mod session;

use crate::model::Model;
use crate::model::auth::Session;
use crate::mutation::Mutation;
use crate::query::Query;

/// Root GraphQL schema
pub type Schema = async_graphql::Schema<Query, Mutation, EmptySubscription>;

/// Noop endpoint existing only to refresh session if needed
#[get("/refresh")]
async fn refresh() -> &'static str {
    ""
}

/// Closes current session
#[delete("/session")]
async fn expire_session(req: HttpRequest, model: Data<Model>) -> Result<()> {
    if let Some(session) = req.extensions_mut().remove::<Session>() {
        let db = model.db();
        session
            .expire(db)
            .await
            .map_err(|_| ErrorInternalServerError("Cannot close session"))?;
    }

    Ok(())
}

/// ActixWeb GraphQL endpoint
#[post("/api")]
async fn api(
    schema: web::Data<Schema>,
    req: HttpRequest,
    request: GraphQLRequest,
) -> GraphQLResponse {
    let mut request = request.into_inner();
    if let Some(session) = req.extensions_mut().remove::<Session>() {
        request = request.data(session);
    }
    schema.execute(request).await.into()
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
    context: Model,
) -> color_eyre::Result<impl Fn(&mut web::ServiceConfig) + Clone> {
    let cfg = move |cfg: &mut ServiceConfig| {
        let session_aware = {
            web::scope("")
                .wrap(middleware::from_fn(session::middleware))
                .service(api)
                .service(refresh)
                .service(expire_session)
        };

        cfg.app_data(Data::new(context.schema()))
            .app_data(Data::new(context.clone()))
            .service(session_aware);

        if graphiql_enabled {
            cfg.service(graphiql);
        }
    };

    Ok(cfg)
}
