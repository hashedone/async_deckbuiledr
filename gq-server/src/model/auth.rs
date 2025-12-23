//! Authoriazation data

use std::str::FromStr;
use std::time::Duration;

use actix_web::http::header::HeaderValue;
use async_graphql::scalar;
use base64::prelude::*;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::{OptionExt, bail, ensure};
use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::footer::Footer;
use pasetors::keys::{AsymmetricKeyPair, AsymmetricPublicKey, Generate};
use pasetors::paserk::{self, FormatAsPaserk};
use pasetors::token::UntrustedToken;
use pasetors::version4::V4;
use pasetors::{Public, public};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use sqlx::prelude::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::model::users::UserId;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid token format")]
    InvalidTokenFormat,
    #[error("Token doesn't exist")]
    NonExistingToken,
    #[error("Missing user id on a token")]
    MissingUserId,
    #[error("Missing token id on a token")]
    MissingTokenId,
    #[error("Missing session data")]
    MissingClaims,
    #[error("Signature malformed in the database")]
    InvalidSignatureStored,
    #[error("Invalid session claim {0}")]
    InvalidSessionClaim(&'static str),
    #[error("Token ID collision")]
    TokenIdCollision,
    #[error("Invalid authorization format")]
    InvalidAuthorization,
    #[error("Invalid authorization scheme")]
    InvalidAuthorizationScheme,
}

/// Secret used as key for signing user tokens. For now it is a silly constant for testing
/// purposes, but it should be a secret fed from environment variable during the build.
const USER_TOKEN_APP_SECRET: &str = "AsyncDeckbuilderAppTokenSecret";

/// PASETO implicit assertion for session tokens
const SESSION_APP_SECRET: &[u8] = b"AsyncDeckbuilderAppSessionTokenSecret";

/// Authentication method based on `Authorization` HTTP header
#[derive(Debug, Clone)]
pub enum Authorization {
    /// AdHoc token
    AdHoc(AdHocToken),
    /// Session token
    Session(SessionToken),
}

impl FromStr for Authorization {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (scheme, token) = s.split_once(' ').ok_or(Error::InvalidAuthorization)?;

        match scheme {
            "AdHoc" => Ok(Self::AdHoc(AdHocToken(token.to_owned()))),
            "Session" => Ok(Self::Session(SessionToken(token.to_owned()))),
            _ => Err(Error::InvalidAuthorizationScheme),
        }
    }
}

/// Newtype for the ad-hoc tokens string
#[derive(Debug, Clone, PartialEq, Eq, Hash, Type, Serialize, Deserialize)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct AdHocToken(String);

scalar!(AdHocToken);

impl AdHocToken {
    /// Creates new authorization token for an user storing it in the database
    ///
    /// Returned string is a encoded token to be used in authorization as the `Authorization: AdHoc [token]`
    pub async fn create(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        user_id: UserId,
    ) -> Result<Self> {
        let (user_token, token) = UserToken::generate(user_id);

        let token_id = Uuid::new_v4();

        let insertion = sqlx::query(
                "insert into adhoc_tokens (id, user_id, secret, signature) values (?, ?, ?, ?) on conflict(id) do nothing",
            )
            .bind(token_id)
            .bind(user_id)
            .bind(user_token.secret)
            .bind(user_token.signature.as_slice())
            .execute(db)
            .await?;

        if insertion.rows_affected() == 0 {
            bail!(Error::TokenIdCollision);
        }

        let token_id = BASE64_STANDARD.encode(token_id.as_bytes());
        Ok(Self(format!("{token_id}.{token}")))
    }

    /// Verifies an user token, returnign the authenticated user id on success
    ///
    /// The argument is the user token string extracted from the `Authorization` header
    pub async fn authenticate(
        &self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<UserId> {
        let (token_id, token) = self
            .0
            .split_once('.')
            .ok_or_eyre(Error::InvalidTokenFormat)?;

        let token_id: [u8; 16] = base64::prelude::BASE64_STANDARD
            .decode(token_id)?
            .try_into()
            .map_err(|_| Error::InvalidTokenFormat)?;
        let token_id = Uuid::from_bytes(token_id);

        let (user_id, secret, signature): (UserId, Uuid, Vec<u8>) =
            sqlx::query_as("select user_id, secret, signature from adhoc_tokens where id = ?")
                .bind(token_id)
                .fetch_optional(db)
                .await?
                .ok_or_eyre(Error::NonExistingToken)?;
        let signature: [u8; 32] = signature
            .try_into()
            .map_err(|_| Error::InvalidSignatureStored)?;

        UserToken {
            user_id,
            secret,
            signature,
        }
        .verify(token)
    }
}

/// The short-living user token data
///
/// Used to quick login for short-living/temporary users. When temporary user is created, to
/// confirm his identify in the future, the user token is generated. User token has a structure of:
/// `{token_id}.{token}`. `token_id` part is an identifier unique for the system, that serves as
/// the index in the tokens storage. `token` itself is used to authorize the user.
/// However token is never stored in the system directly. Instead, tokens serves to generate
/// signature. The entry in the state holds the `secret`, `user_id` and the `signature`. `user_id`
/// is simply identifier of an user authorized by this token.
/// To authorize the user, next step is to prepare the info data - it is formatted as
/// `{APP_KEY}.{user_id}.{secret}.{token}'` - `APP_KEY` is a hardcoded value that eventually should be provided during
/// compilation, and it makes it difficult to figure out the key even if the `secret` would leak.
/// On the other hand `secret` and `user_id` are random Uuids (however `user_id` is not considered
/// secret, it is exposed by a public API. Finally `token` is token provided
/// for authentication. Composing the data this way ensures, that the stored signature is hash
/// function of data from three separate sources: user provided token, application state and the
/// application builtin. Leak of single component is not enough to reverse engineer the
/// authentication token.
///
/// To improve the entropy of hashed data we all the Uuids used in the hashed data are compressed
/// with Base64 algorithm instead of using their standard Uuid representation. That also shortens
/// tokens for more convenient usage.
///
/// We avoid using JWT or similar for this solution, as the relevant part of it is to have rather
/// short keys that are easier to store. This is less secure implementation, and also with less
/// capabilities as tokens are not really self-contained, but this is only purposed for
/// short-living users, and security is a secondary need here. For true authorization of long
/// living users with higher privilidges (eg. creating games), the user would be authorized ideally
/// by some 3rd-party authorization solution.
#[derive(Debug, Clone)]
struct UserToken {
    /// Authorized user identifier
    user_id: UserId,
    /// Secret to build the signing key
    secret: Uuid,
    /// Expected hash
    signature: [u8; 32],
}

impl UserToken {
    /// Generate new token for the given user
    ///
    /// Returns pair of generated `UserToken` and `token` part of the Authentication Token that
    /// will be needed to pass for verification.
    fn generate(user_id: UserId) -> (Self, String) {
        let secret = Uuid::new_v4();
        let token = Uuid::new_v4();
        let token = BASE64_STANDARD.encode(token.as_bytes());

        let secret_base64 = BASE64_STANDARD.encode(secret.as_bytes());

        let data = format!("{USER_TOKEN_APP_SECRET}.{user_id}.{secret_base64}.{token}");

        let mut hasher = Sha3_256::new();
        hasher.update(data.as_bytes());
        let signature = hasher.finalize().into();

        let user_token = UserToken {
            user_id,
            secret,
            signature,
        };

        (user_token, token)
    }

    /// Verifies the user token returning user id if verification is successfull
    fn verify(&self, token: &str) -> Result<UserId> {
        let secret = BASE64_STANDARD.encode(self.secret.as_bytes());
        let user_id = self.user_id;

        let data = format!("{USER_TOKEN_APP_SECRET}.{user_id}.{secret}.{token}");

        let mut hasher = Sha3_256::new();
        hasher.update(data.as_bytes());
        let signature: [u8; _] = hasher.finalize().into();

        ensure!(signature == self.signature, "Token signature doesn't match");
        Ok(user_id)
    }
}

/// Session data atached to Paseto session token
///
/// Session token is what actually gives access to any priviledges - any authorization method is
/// there only to obdain the session token.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionData {
    /// User authorized by this token
    pub user_id: UserId,
}

impl SessionData {
    //// Appends data to the session claims
    fn append(&self, mut claims: Claims) -> Result<Claims> {
        claims.issuer(&self.user_id.to_string())?;
        Ok(claims)
    }

    /// Builds session data from token claims
    fn from_claims(claims: &Claims) -> Result<Self> {
        let user_id = claims.get_claim("iss").ok_or_eyre(Error::MissingUserId)?;

        Ok(SessionData {
            user_id: user_id.as_str().ok_or_eyre(Error::MissingUserId)?.parse()?,
        })
    }
}

/// Newtype for session token string
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionToken(String);

scalar!(SessionToken);

impl std::fmt::Display for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl SessionToken {
    /// Authenticates a token returning session
    pub async fn authenticate(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<Session> {
        Session::authenticate(db, self).await
    }

    pub fn into_header(self) -> Result<HeaderValue> {
        HeaderValue::from_str(&self.0).map_err(Into::into)
    }
}

/// Session data
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    /// User ID for this session
    pub user_id: UserId,
    /// Session token
    pub token: SessionToken,
    /// Session expiration time
    pub expires_at: DateTime<Utc>,
}

impl Session {
    /// Creates a new session for given user storing it in DB
    pub async fn create(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        user_id: UserId,
    ) -> Result<Self> {
        let (session, kid, pk) = Self::new(user_id)?;

        sqlx::query("insert into session_tokens (id, public_key, expires_at) values (?, ?, ?)")
            .bind(kid)
            .bind(pk)
            .bind(session.expires_at)
            .execute(db)
            .await?;

        Ok(session)
    }

    /// Verifies a session token, returnign the authenticated user id on success
    ///
    /// The argument is the user token string extracted from the `Authorization` header
    pub async fn authenticate(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        session_token: SessionToken,
    ) -> Result<Self> {
        let token = UntrustedToken::<Public, V4>::try_from(&session_token.0)?;
        let mut footer = Footer::new();
        footer.parse_bytes(token.untrusted_footer())?;

        let key_id = footer
            .get_claim("kid")
            .ok_or_eyre(Error::MissingTokenId)?
            .as_str()
            .ok_or_eyre(Error::MissingTokenId)?;

        let (key,): (String,) =
            sqlx::query_as("select public_key from session_tokens where id = ?")
                .bind(key_id)
                .fetch_optional(db)
                .await?
                .ok_or_eyre(Error::NonExistingToken)?;

        let key = AsymmetricPublicKey::<V4>::try_from(key.as_str())?;

        let rules = ClaimsValidationRules::new();
        let token = public::verify(&key, &token, &rules, None, Some(SESSION_APP_SECRET))?;

        let claims = token.payload_claims().ok_or_eyre(Error::MissingClaims)?;
        let session = SessionData::from_claims(claims)?;

        Ok(Self {
            user_id: session.user_id,
            token: session_token,
            expires_at: expires_at(&claims)?,
        })
    }

    /// Expires session removing it's entry in database
    pub async fn expire(self, db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>) -> Result<()> {
        let token = UntrustedToken::<Public, V4>::try_from(&self.token.0)?;
        let mut footer = Footer::new();
        footer.parse_bytes(token.untrusted_footer())?;

        let key_id = footer
            .get_claim("kid")
            .ok_or_eyre(Error::MissingTokenId)?
            .as_str()
            .ok_or_eyre(Error::MissingTokenId)?;

        sqlx::query("delete from session_tokens where id = ?")
            .bind(key_id)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Refreshes the session creating a new one, and updating the database entry to use new `key_id` and `public_key`.
    pub async fn refresh(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<Self> {
        let token = UntrustedToken::<Public, V4>::try_from(&self.token.0)?;
        let mut footer = Footer::new();
        footer.parse_bytes(token.untrusted_footer())?;

        let prev_kid = footer
            .get_claim("kid")
            .ok_or_eyre(Error::MissingTokenId)?
            .as_str()
            .ok_or_eyre(Error::MissingTokenId)?;

        let (session, kid, pk) = Self::new(self.user_id)?;

        sqlx::query(
            "update session_tokens set id = ?, public_key = ?, expires_at = ? where id = ?",
        )
        .bind(kid)
        .bind(pk)
        .bind(session.expires_at)
        .bind(prev_kid)
        .execute(db)
        .await?;

        Ok(session)
    }

    /// Cleans expired sessions from database.
    pub async fn cleanup(db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>) -> Result<()> {
        let now = Utc::now();
        sqlx::query("delete from session_tokens where expires_at < ?")
            .bind(now)
            .execute(db)
            .await?;
        Ok(())
    }

    /// Creates new session for an user.
    ///
    /// The session data are not stored in the database. The `(session, key_id, public_key)` tuple is returned instead
    /// for the purpose of storing the session.
    fn new(user_id: UserId) -> Result<(Self, String, String)> {
        let key_pair = AsymmetricKeyPair::<V4>::generate()?;
        let key_id = paserk::Id::from(&key_pair.public);

        let session = SessionData { user_id };
        let valid_duration = Duration::from_hours(24);

        let claims = Claims::new_expires_in(&valid_duration).unwrap();
        let claims = session.append(claims)?;
        let expires_at = expires_at(&claims)?;

        // For paseko session tokens we ignore any possibility of key collilsion - it is
        // extremply unlikely, and if it happens the worse result is that someones else session
        // expires.
        let mut kid = String::new();
        key_id.fmt(&mut kid)?;

        let mut pk = String::new();
        key_pair.public.fmt(&mut pk)?;

        let mut footer = Footer::new();
        footer.key_id(&key_id);

        let token = public::sign(
            &key_pair.secret,
            &claims,
            Some(&footer),
            Some(SESSION_APP_SECRET),
        )?;

        let session = Self {
            user_id,
            token: SessionToken(token),
            expires_at,
        };

        Ok((session, kid, pk))
    }
}

/// Retrieves `expires_at` from the session claims.
fn expires_at(claims: &Claims) -> Result<DateTime<Utc>> {
    let expires_at = claims
        .get_claim("exp")
        .and_then(|issued_at| issued_at.as_str())
        .ok_or(Error::InvalidSessionClaim("exp"))?;
    expires_at.parse().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::model::users::User;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("model/migrations").run(&pool).await.unwrap();
        pool
    }

    mod user_token {
        use super::*;

        #[tokio::test]
        async fn verify_with_generated_token() {
            let pool = setup_pool().await;

            let user1 = User::new("user1").create(&pool).await.unwrap();
            let token1 = user1.create_adhoc_token(&pool).await.unwrap();
            let authorized_user = token1.authenticate(&pool).await.unwrap();

            assert_eq!(user1, authorized_user);

            // Checking if everything works when another users are added
            let user2 = User::new("user2").create(&pool).await.unwrap();
            let token2 = user2.create_adhoc_token(&pool).await.unwrap();

            // Also multiple tokens for single user;
            let token3 = user2.create_adhoc_token(&pool).await.unwrap();

            let user4 = User::new("user1").create(&pool).await.unwrap();
            let token4 = user4.create_adhoc_token(&pool).await.unwrap();

            let authorized_user = token1.authenticate(&pool).await.unwrap();
            assert_eq!(user1, authorized_user);

            let authorized_user = token2.authenticate(&pool).await.unwrap();
            assert_eq!(user2, authorized_user);

            let authorized_user = token3.authenticate(&pool).await.unwrap();
            assert_eq!(user2, authorized_user);

            let authorized_user = token4.authenticate(&pool).await.unwrap();
            assert_eq!(user4, authorized_user);
        }

        #[tokio::test]
        async fn verify_with_random_data_fails() {
            let pool = setup_pool().await;

            let _ = AdHocToken("fake_token".into())
                .authenticate(&pool)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn verify_with_invalid_key_fails() {
            let pool = setup_pool().await;

            let _ = AdHocToken("U7PydAY1TsKmmVGf4LS3YA==.PUGKx45wSK+0rhl4F2TDdg==".into())
                .authenticate(&pool)
                .await
                .unwrap_err();
        }
    }

    mod session_token {
        use super::*;

        #[tokio::test]
        async fn verify_with_generated_token() {
            let pool = setup_pool().await;

            let user1 = User::new("user1").create(&pool).await.unwrap();
            let token1 = user1.create_session(&pool).await.unwrap().token;

            let session = token1.clone().authenticate(&pool).await.unwrap();
            assert_eq!(user1, session.user_id);

            let user2 = User::new("user2").create(&pool).await.unwrap();
            let token2 = user2.create_session(&pool).await.unwrap().token;

            // Also multiple tokens for single user;
            let token3 = user2.create_session(&pool).await.unwrap().token;

            let user4 = User::new("user1").create(&pool).await.unwrap();
            let token4 = user4.create_session(&pool).await.unwrap().token;

            let session = token1.authenticate(&pool).await.unwrap();
            assert_eq!(user1, session.user_id);

            let session = token2.authenticate(&pool).await.unwrap();
            assert_eq!(user2, session.user_id);

            let session = token3.authenticate(&pool).await.unwrap();
            assert_eq!(user2, session.user_id);

            let session = token4.authenticate(&pool).await.unwrap();
            assert_eq!(user4, session.user_id);
        }

        #[tokio::test]
        async fn verify_with_random_data_fails() {
            let pool = setup_pool().await;
            let _ = SessionToken("fake_token".into())
                .authenticate(&pool)
                .await
                .unwrap_err();
        }

        #[tokio::test]
        async fn verify_with_expired_session_fails() {
            let pool = setup_pool().await;

            let user_id = User::new("user1").create(&pool).await.unwrap();
            let session = user_id.create_session(&pool).await.unwrap();
            let token = session.token.clone();

            session.expire(&pool).await.unwrap();

            let _ = token.authenticate(&pool).await.unwrap_err();
        }

        #[tokio::test]
        async fn session_refresh() {
            let pool = setup_pool().await;

            let user_id = User::new("user1").create(&pool).await.unwrap();
            let old_session = user_id.create_session(&pool).await.unwrap();
            let session = old_session.clone().refresh(&pool).await.unwrap();

            assert_ne!(old_session.token, session.token);

            let _ = old_session.token.authenticate(&pool).await.unwrap_err();
            let authenticated = session.token.clone().authenticate(&pool).await.unwrap();
            assert_eq!(session, authenticated);
        }
    }
}
