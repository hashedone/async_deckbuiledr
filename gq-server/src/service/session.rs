//! Session management

use actix_web::body::MessageBody;
use actix_web::dev::ServiceResponse;
use actix_web::error::ErrorUnauthorized;
use actix_web::http::header::{self, HeaderName, HeaderValue};
use actix_web::middleware::Next;
use actix_web::web::Data;
use actix_web::{Error, HttpMessage};

use crate::context::Model;
use crate::context::session::Session;

const SESSION_TOKEN_HEADER: HeaderName = HeaderName::from_static("x-session-token");

pub async fn middleware<B>(
    req: actix_web::dev::ServiceRequest,
    next: Next<B>,
) -> Result<ServiceResponse<B>, Error>
where
    B: MessageBody + 'static,
{
    let mut new_session_token: Option<String> = None;
    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        let auth_header = auth_header
            .to_str()
            .map_err(|err| ErrorUnauthorized(err.to_string()))?;

        let (scheme, token) = auth_header
            .split_once(' ')
            .ok_or_else(|| ErrorUnauthorized("Invalid Authorization header"))?;

        let context: Data<Model> = req
            .app_data()
            .cloned()
            .ok_or_else(|| ErrorUnauthorized("Missing context"))?;

        let auth = context.auth();
        let session = match scheme {
            "AdHoc" => {
                let user_id = auth
                    .verify_user_token(token)
                    .await
                    .map_err(|err| ErrorUnauthorized(err.to_string()))?;

                let session = auth
                    .create_session(user_id)
                    .await
                    .map_err(|err| ErrorUnauthorized(err.to_string()))?;

                new_session_token = Some(session.token.clone());
                session
            }
            "Session" => auth
                .verify_session_token(token)
                .await
                .map_err(|err| ErrorUnauthorized(err.to_string()))?,
            _ => {
                return Err(ErrorUnauthorized("Invalid Authorization token scheme"));
            }
        };

        req.extensions_mut().insert::<Session>(session);
    }

    let mut response = next.call(req).await?;
    if let Some(session_token) = new_session_token {
        let header_value = HeaderValue::from_str(&session_token)
            .map_err(|err| ErrorUnauthorized(err.to_string()))?;
        response
            .headers_mut()
            .insert(SESSION_TOKEN_HEADER, header_value);
    }
    Ok(response)
}
