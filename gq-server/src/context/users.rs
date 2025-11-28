//! Serivce users storage

use std::collections::{HashMap, hash_map};
use std::fmt;
use std::str::FromStr;

use base64::prelude::*;
use color_eyre::Report;
use color_eyre::eyre::eyre;
use derivative::Derivative;
use juniper::{FromContext, GraphQLObject, graphql_object};
use tokio::sync::{RwLock, RwLockReadGuard};
use uuid::Uuid;

use crate::context::Context;

/// User id
#[derive(Debug, Clone, Copy)]
pub struct UserId(Uuid);

#[graphql_object]
impl UserId {
    #[graphql(ignore)]
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    async fn user(&self, context: &Users) -> Option<User> {
        context.user(*self).await
    }
}

impl FromStr for UserId {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let bytes: [u8; 16] = BASE64_STANDARD
            .decode(s)?
            .try_into()
            .map_err(|_| eyre!("Invalid user id format"))?;

        Ok(Self(Uuid::from_bytes(bytes)))
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", BASE64_STANDARD.encode(self.0.as_bytes()))
    }
}

/// User queryable data
#[derive(Debug, Clone, GraphQLObject)]
pub struct User {
    /// How user is visible to others.
    pub nickname: String,
}

/// User data
pub struct UserData {
    /// How user is visible to others.
    pub nickname: String,
}

// Users storage.
#[derive(Derivative)]
#[derivative(Default(new = "true"))]
pub struct Users {
    /// Users map
    users: RwLock<HashMap<Uuid, UserData>>,
}

impl Users {
    /// Returns single user by their id
    pub async fn user(&self, user_id: UserId) -> Option<User> {
        self.users.read().await.get(&user_id.0).map(|data| User {
            nickname: data.nickname.clone(),
        })
    }

    /// Returns users hashmap locked for read.
    pub async fn users(&self) -> RwLockReadGuard<'_, HashMap<Uuid, UserData>> {
        self.users.read().await
    }

    /// Create a new user returning created user id.
    pub async fn create(&self, nickname: String) -> UserId {
        let mut users = self.users.write().await;
        loop {
            let user_id = Uuid::new_v4();
            let entry = users.entry(user_id);
            if let hash_map::Entry::Vacant(entry) = entry {
                entry.insert(UserData { nickname });
                return UserId(user_id);
            }
        }
    }
}

impl juniper::Context for Users {}

impl FromContext<Context> for Users {
    fn from(context: &Context) -> &Self {
        context.users()
    }
}
