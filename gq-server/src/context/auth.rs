//! Authoriazation data

use std::collections::{HashMap, hash_map};

use base64::prelude::*;
use color_eyre::Result;
use color_eyre::eyre::{OptionExt, ensure, eyre};
use sha3::{Digest, Sha3_256};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::context::users::UserId;

/// Secret used as key for signing user tokens. For now it is a silly constant for testing
/// purposes, but it should be a secret fed from environment variable during the build.
const USER_TOKEN_APP_SECRET: &str = "AsyncDeckbuilderAppTokenSecret";

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

pub struct Auth {
    /// User tokens map
    user_tokens: RwLock<HashMap<Uuid, UserToken>>,
}

impl Auth {
    /// Creates new authorization storage
    pub fn new() -> Self {
        Self {
            user_tokens: RwLock::new(HashMap::new()),
        }
    }

    /// Creates new authorization token for an user
    pub async fn create_user_token(&self, user_id: UserId) -> String {
        let mut tokens = self.user_tokens.write().await;
        let (token_entry, token_id) = loop {
            let token_id = Uuid::new_v4();
            let entry = tokens.entry(token_id);
            if let hash_map::Entry::Vacant(entry) = entry {
                break (entry, token_id);
            }
        };

        let (user_token, token) = UserToken::generate(user_id);
        token_entry.insert(user_token);

        let token_id = BASE64_STANDARD.encode(token_id.as_bytes());
        format!("{token_id}.{token}")
    }

    /// Verifies an user token and returns the authorized user id on success
    pub async fn verify_user_token(&self, token: &str) -> Result<UserId> {
        let (token_id, token) = token
            .split_once('.')
            .ok_or_eyre("Invalid user token format")?;

        let token_id: [u8; 16] = base64::prelude::BASE64_STANDARD
            .decode(token_id)?
            .try_into()
            .map_err(|_| eyre!("Invalid token_id format"))?;
        let token_id = Uuid::from_bytes(token_id);

        let tokens = self.user_tokens.read().await;
        let user_token = tokens.get(&token_id).ok_or_eyre("Token doesn't exist")?;

        let user_id = user_token.verify(token)?;
        Ok(user_id)
    }
}

impl Default for Auth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::context::Users;

    use super::*;

    #[tokio::test]
    async fn verify_with_generated_token() {
        let auth = Auth::default();
        let users = Users::default();

        let user1 = users.create("user1").await;
        let token1 = auth.create_user_token(user1).await;
        let authorized_user = auth.verify_user_token(&token1).await.unwrap();

        assert_eq!(user1, authorized_user);

        // Checking if everything works when another users are added
        let user2 = users.create("user2").await;
        let token2 = auth.create_user_token(user2).await;
        // Also multiple tokens for single user;
        let token3 = auth.create_user_token(user2).await;

        let user4 = users.create("user1").await;
        let token4 = auth.create_user_token(user4).await;

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
        let auth = Auth::default();
        let _ = auth.verify_user_token("fake_token").await.unwrap_err();
    }

    #[tokio::test]
    async fn verify_with_invalid_key_fails() {
        let auth = Auth::default();
        let _ = auth
            .verify_user_token("U7PydAY1TsKmmVGf4LS3YA==.PUGKx45wSK+0rhl4F2TDdg==")
            .await
            .unwrap_err();
    }
}
