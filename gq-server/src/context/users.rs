//! Serivce users storage

use std::collections::{HashMap, hash_map};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

use async_graphql::connection::CursorType;
use async_graphql::{Result, SimpleObject};
use base64::prelude::*;
use derivative::Derivative;
use thiserror::Error;
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Invalid user id format")]
    InvalidUserId,
}

/// User id
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UserId(Uuid);

impl UserId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl FromStr for UserId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes: [u8; 16] = BASE64_STANDARD
            .decode(s)
            .map_err(|_| Error::InvalidUserId)?
            .try_into()
            .map_err(|_| Error::InvalidUserId)?;

        Ok(Self(Uuid::from_bytes(bytes)))
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", BASE64_STANDARD.encode(self.0.as_bytes()))
    }
}

impl CursorType for UserId {
    type Error = <Self as FromStr>::Err;

    fn decode_cursor(s: &str) -> std::result::Result<Self, Self::Error> {
        s.parse()
    }

    fn encode_cursor(&self) -> String {
        self.to_string()
    }
}

/// User queryable data
#[derive(Debug, Clone, PartialEq, SimpleObject)]
pub struct User {
    /// How user is visible to others.
    pub nickname: String,
}

// Users storage.
#[derive(Derivative)]
#[derivative(Default(new = "true"))]
struct UsersInner {
    /// Users map
    users: RwLock<HashMap<Uuid, User>>,
}

#[derive(Derivative, Clone)]
#[derivative(Default(new = "true"))]
pub struct Users(Arc<UsersInner>);

impl Users {
    /// Returns single user by their id
    pub async fn user(&self, user_id: UserId) -> Option<User> {
        self.0.users.read().await.get(&user_id.0).cloned()
    }

    /// Returns users hashmap locked for read.
    pub async fn users(&self) -> RwLockReadGuard<'_, HashMap<Uuid, User>> {
        self.0.users.read().await
    }

    /// Create a new user returning created user id.
    pub async fn create(&self, nickname: impl Into<String>) -> UserId {
        let nickname = nickname.into();

        let mut users = self.0.users.write().await;
        loop {
            let user_id = Uuid::new_v4();
            let entry = users.entry(user_id);
            if let hash_map::Entry::Vacant(entry) = entry {
                entry.insert(User { nickname });
                return UserId(user_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn users_empty_initially() {
        let users = Users::default();

        assert!(users.users().await.is_empty())
    }

    #[tokio::test]
    async fn creating_users() {
        let users = Users::default();

        let user1 = users.create("user1").await;
        {
            let users = users.users().await;
            assert_eq!(users.len(), 1);
            assert_eq!(
                &User {
                    nickname: "user1".to_owned()
                },
                users.get(&user1.0).unwrap()
            );
        }

        let user2 = users.create("user2").await;
        {
            let users = users.users().await;
            assert_eq!(users.len(), 2);
            assert_eq!(
                &User {
                    nickname: "user1".to_owned()
                },
                users.get(&user1.0).unwrap()
            );
            assert_eq!(
                &User {
                    nickname: "user2".to_owned()
                },
                users.get(&user2.0).unwrap()
            );
        }

        // Username *can* collide
        let user3 = users.create("user1").await;
        {
            let users = users.users().await;
            assert_eq!(users.len(), 3);
            assert_eq!(
                &User {
                    nickname: "user1".to_owned()
                },
                users.get(&user1.0).unwrap()
            );
            assert_eq!(
                &User {
                    nickname: "user2".to_owned()
                },
                users.get(&user2.0).unwrap()
            );
            assert_eq!(
                &User {
                    nickname: "user1".to_owned()
                },
                users.get(&user3.0).unwrap()
            );
        }
    }
}
