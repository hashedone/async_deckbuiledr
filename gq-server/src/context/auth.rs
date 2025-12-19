//! Authoriazation data

use std::time::Duration;

use base64::prelude::*;
use chrono::{DateTime, Utc};
use color_eyre::Result;
use color_eyre::eyre::{OptionExt, ensure};
use pasetors::claims::{Claims, ClaimsValidationRules};
use pasetors::footer::Footer;
use pasetors::keys::{AsymmetricKeyPair, AsymmetricPublicKey, Generate};
use pasetors::paserk::{self, FormatAsPaserk};
use pasetors::token::UntrustedToken;
use pasetors::version4::V4;
use pasetors::{Public, public};
use sha3::{Digest, Sha3_256};
use sqlx::Executor;
use thiserror::Error;
use uuid::Uuid;

use crate::context::session::Session;

#[derive(Debug, Error)]
enum Error {
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
}

/// Secret used as key for signing user tokens. For now it is a silly constant for testing
/// purposes, but it should be a secret fed from environment variable during the build.
const USER_TOKEN_APP_SECRET: &str = "AsyncDeckbuilderAppTokenSecret";

/// PASETO implicit assertion for session tokens
const SESSION_APP_SECRET: &[u8] = b"AsyncDeckbuilderAppSessionTokenSecret";

/// The short-living user token
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
#[derive(Debug)]
struct UserToken {
    /// Authorized user identifier
    user_id: i64,
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
    fn generate(user_id: i64) -> (Self, String) {
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
    fn verify(&self, token: &str) -> Result<i64> {
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
    pub user_id: i64,
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

// Authoriation data access
pub struct Auth<'a, Db> {
    /// Database connection
    db: &'a Db,
}

impl<'a, Db> Auth<'a, Db> {
    /// Creates new users accessor
    pub(super) fn new(db: &'a Db) -> Self {
        Self { db }
    }
}

impl<'a, Db> Auth<'a, Db>
where
    &'a Db: Executor<'a, Database = sqlx::Sqlite>,
{
    /// Creates new authorization token for an user
    pub async fn create_user_token(&self, user_id: i64) -> Result<String> {
        let (user_token, token) = UserToken::generate(user_id);

        // Looping to retry in case of unlikely token id collision. We avoid using auto incrementing
        // for token id generation to make tokens less predictable
        loop {
            let token_id = Uuid::new_v4();

            let insertion = sqlx::query(
                "insert into user_tokens (id, user_id, secret, signature) values (?, ?, ?, ?) on conflict(id) do nothing",
            )
            .bind(token_id)
            .bind(user_id)
            .bind(user_token.secret)
            .bind(user_token.signature.as_slice())
            .execute(self.db)
            .await?;

            if insertion.rows_affected() == 1 {
                let token_id = BASE64_STANDARD.encode(token_id.as_bytes());
                return Ok(format!("{token_id}.{token}"));
            }
        }
    }

    /// Creates new for an user
    pub async fn create_session(&self, user_id: i64) -> Result<Session> {
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
        sqlx::query("insert into session_tokens (id, public_key, expires_at) values (?, ?, ?)")
            .bind(kid)
            .bind(pk)
            .bind(expires_at)
            .execute(self.db)
            .await?;

        let mut footer = Footer::new();
        footer.key_id(&key_id);

        let token = public::sign(
            &key_pair.secret,
            &claims,
            Some(&footer),
            Some(SESSION_APP_SECRET),
        )?;

        Ok(Session {
            user_id,
            token,
            expires_at,
        })
    }

    /// Verifies an user token and returns the authorized user id on success
    pub async fn verify_user_token(&self, token: &str) -> Result<i64> {
        let (token_id, token) = token
            .split_once('.')
            .ok_or_eyre(Error::InvalidTokenFormat)?;

        let token_id: [u8; 16] = base64::prelude::BASE64_STANDARD
            .decode(token_id)?
            .try_into()
            .map_err(|_| Error::InvalidTokenFormat)?;
        let token_id = Uuid::from_bytes(token_id);

        let (user_id, secret, signature): (i64, Uuid, Vec<u8>) =
            sqlx::query_as("select user_id, secret, signature from user_tokens where id = ?")
                .bind(token_id)
                .fetch_optional(self.db)
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

    pub async fn verify_session_token(&self, token_str: impl Into<String>) -> Result<Session> {
        let token_str = token_str.into();
        let token = UntrustedToken::<Public, V4>::try_from(&token_str)?;
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
                .fetch_optional(self.db)
                .await?
                .ok_or_eyre(Error::NonExistingToken)?;

        let key = AsymmetricPublicKey::<V4>::try_from(key.as_str())?;

        let rules = ClaimsValidationRules::new();
        let token = public::verify(&key, &token, &rules, None, Some(SESSION_APP_SECRET))?;

        let claims = token.payload_claims().ok_or_eyre(Error::MissingClaims)?;
        let session = SessionData::from_claims(claims)?;

        Ok(Session {
            user_id: session.user_id,
            token: token_str,
            expires_at: expires_at(&claims)?,
        })
    }

    /// Expires session for given token
    ///
    /// The function doesn't verify if the token is properly sign, it only parses the token to retrieve the
    /// token id and remove its entry from database.
    pub async fn expire_session(&self, token: &str) -> Result<()> {
        let token = UntrustedToken::<Public, V4>::try_from(token)?;
        let mut footer = Footer::new();
        footer.parse_bytes(token.untrusted_footer())?;

        let key_id = footer
            .get_claim("kid")
            .ok_or_eyre(Error::MissingTokenId)?
            .as_str()
            .ok_or_eyre(Error::MissingTokenId)?;

        sqlx::query("delete from session_tokens where id = ?")
            .bind(key_id)
            .execute(self.db)
            .await?;
        Ok(())
    }

    /// Cleans expired sessions from database.
    pub async fn clean_sessions(&self) -> Result<()> {
        let now = Utc::now();
        sqlx::query("delete from session_tokens where expires_at < ?")
            .bind(now)
            .execute(self.db)
            .await?;
        Ok(())
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

    use crate::context::Users;
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
            let auth = Auth::new(&pool);
            let users = Users::new(&pool);

            let user1 = users.create("user1").await.unwrap();
            let token1 = auth.create_user_token(user1).await.unwrap();
            let authorized_user = auth.verify_user_token(&token1).await.unwrap();

            assert_eq!(user1, authorized_user);

            // Checking if everything works when another users are added
            let user2 = users.create("user2").await.unwrap();
            let token2 = auth.create_user_token(user2).await.unwrap();
            // Also multiple tokens for single user;
            let token3 = auth.create_user_token(user2).await.unwrap();

            let user4 = users.create("user1").await.unwrap();
            let token4 = auth.create_user_token(user4).await.unwrap();

            let authorized_user = auth.verify_user_token(&token1).await.unwrap();
            assert_eq!(user1, authorized_user);

            let authorized_user = auth.verify_user_token(&token2).await.unwrap();
            assert_eq!(user2, authorized_user);

            let authorized_user = auth.verify_user_token(&token3).await.unwrap();
            assert_eq!(user2, authorized_user);

            let authorized_user = auth.verify_user_token(&token4).await.unwrap();
            assert_eq!(user4, authorized_user);
        }

        #[tokio::test]
        async fn verify_with_random_data_fails() {
            let pool = setup_pool().await;
            let auth = Auth::new(&pool);
            let _ = auth.verify_user_token("fake_token").await.unwrap_err();
        }

        #[tokio::test]
        async fn verify_with_invalid_key_fails() {
            let pool = setup_pool().await;
            let auth = Auth::new(&pool);
            let _ = auth
                .verify_user_token("U7PydAY1TsKmmVGf4LS3YA==.PUGKx45wSK+0rhl4F2TDdg==")
                .await
                .unwrap_err();
        }
    }

    mod session_token {
        use super::*;

        #[tokio::test]
        async fn verify_with_generated_token() {
            let pool = setup_pool().await;
            let auth = Auth::new(&pool);
            let users = Users::new(&pool);

            let user1 = users.create("user1").await.unwrap();
            let token1 = auth.create_session(user1).await.unwrap().token;

            let session = auth.verify_session_token(&token1).await.unwrap();
            assert_eq!(user1, session.user_id);

            let user2 = users.create("user2").await.unwrap();
            let token2 = auth.create_session(user2).await.unwrap().token;
            // Also multiple tokens for single user;
            let token3 = auth.create_session(user2).await.unwrap().token;

            let user4 = users.create("user1").await.unwrap();
            let token4 = auth.create_session(user4).await.unwrap().token;

            let session = auth.verify_session_token(token1).await.unwrap();
            assert_eq!(user1, session.user_id);

            let session = auth.verify_session_token(token2).await.unwrap();
            assert_eq!(user2, session.user_id);

            let session = auth.verify_session_token(token3).await.unwrap();
            assert_eq!(user2, session.user_id);

            let session = auth.verify_session_token(token4).await.unwrap();
            assert_eq!(user4, session.user_id);
        }

        #[tokio::test]
        async fn verify_with_random_data_fails() {
            let pool = setup_pool().await;
            let auth = Auth::new(&pool);
            let _ = auth.verify_session_token("fake_token").await.unwrap_err();
        }
    }
}
