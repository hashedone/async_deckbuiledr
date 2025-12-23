//! Session management

use actix_web::body::MessageBody;
use actix_web::dev::ServiceResponse;
use actix_web::error::ErrorUnauthorized;
use actix_web::http::header::{self, HeaderName};
use actix_web::middleware::Next;
use actix_web::web::Data;
use actix_web::{Error, HttpMessage};
use chrono::{Duration, Utc};

use crate::model::Model;
use crate::model::auth::{Authorization, SessionToken};

const SESSION_TOKEN_HEADER: HeaderName = HeaderName::from_static("x-session-token");

pub async fn middleware<B>(
    req: actix_web::dev::ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<B>, Error>
where
    B: MessageBody + 'static,
{
    let context: Data<Model> = req
        .app_data()
        .cloned()
        .ok_or_else(|| ErrorUnauthorized("Missing context"))?;

    let mut new_session_token: Option<SessionToken> = None;
    let db = context.db();
    let mut tx = db
        .begin()
        .await
        .map_err(|_| ErrorUnauthorized("Failed to start DB transaction"))?;

    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        let auth_header = auth_header
            .to_str()
            .map_err(|err| ErrorUnauthorized(err.to_string()))?;

        let token: Authorization = auth_header
            .parse()
            .map_err(|_| ErrorUnauthorized("Cannot parse authorization token"))?;

        let session = match token {
            Authorization::AdHoc(token) => {
                let user_id = token
                    .authenticate(&mut *tx)
                    .await
                    .map_err(|err| ErrorUnauthorized(err.to_string()))?;

                let session = user_id
                    .create_session(&mut *tx)
                    .await
                    .map_err(|err| ErrorUnauthorized(err.to_string()))?;

                new_session_token = Some(session.token.clone());
                session
            }

            Authorization::Session(token) => {
                let mut session = token
                    .authenticate(&mut *tx)
                    .await
                    .map_err(|err| ErrorUnauthorized(err.to_string()))?;

                if session.expires_at < Utc::now() + Duration::minutes(10) {
                    session = session
                        .refresh(&mut *tx)
                        .await
                        .map_err(|_| ErrorUnauthorized("Refershing session failed"))?;
                    new_session_token = Some(session.token.clone());
                }

                session
            }
        };

        req.extensions_mut().insert(session);
    }

    let mut response = next.call(req).await?;
    if let Some(session_token) = new_session_token {
        let header_value = session_token
            .into_header()
            .map_err(|err| ErrorUnauthorized(err.to_string()))?;
        response
            .headers_mut()
            .insert(SESSION_TOKEN_HEADER, header_value);
    }

    tx.commit()
        .await
        .map_err(|_| ErrorUnauthorized("Committing transaction failed"))?;

    Ok(response)
}
