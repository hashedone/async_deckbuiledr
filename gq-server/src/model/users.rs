//! Serivce users storage

use async_graphql::SimpleObject;
use color_eyre::eyre::Result;
use sqlx::prelude::Type;
use thiserror::Error;

use crate::model::auth::{AdHocToken, Session};

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Invalid user id format")]
    InvalidUserId,
}

/// Newtype for user id
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Type)]
#[sqlx(transparent)]
pub struct UserId(i64);

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for UserId {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s.parse().map_err(|_| Error::InvalidUserId)?;
        Ok(Self(id))
    }
}

impl UserId {
    /// Fetches `User` with this id from database
    pub async fn fetch(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<Option<User>> {
        User::fetch(db, self).await
    }

    /// Creates an AdHoc token for this user
    pub async fn create_adhoc_token(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<AdHocToken> {
        AdHocToken::create(db, self).await
    }

    /// Creates a session for this user
    pub async fn create_session(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<Session> {
        Session::create(db, self).await
    }
}

/// User queryable data
#[derive(Debug, Clone, PartialEq, SimpleObject)]
pub struct User {
    /// How user is visible to others.
    pub nickname: String,
}

impl User {
    /// Helper to create an user
    pub fn new(nickname: impl Into<String>) -> Self {
        Self {
            nickname: nickname.into(),
        }
    }

    /// Fetches user from the database
    pub async fn fetch(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        user_id: UserId,
    ) -> Result<Option<Self>> {
        let row: Option<(String,)> = sqlx::query_as("select nickname from users where id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?;

        Ok(row.map(|(nickname,)| Self { nickname }))
    }

    /// Creates user in the database
    pub async fn create(
        self,
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
    ) -> Result<UserId> {
        let result = sqlx::query("insert into users(nickname) values (?)")
            .bind(self.nickname)
            .execute(db)
            .await?;

        Ok(UserId(result.last_insert_rowid()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("model/migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn users_empty_initially() {
        let pool = setup_pool().await;

        let (count,): (i64,) = sqlx::query_as("select count(*) from users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn creating_users() {
        let pool = setup_pool().await;

        let user1 = User::new("user1").create(&pool).await.unwrap();

        assert_eq!(
            user1.fetch(&pool).await.unwrap().unwrap(),
            User {
                nickname: "user1".to_owned()
            }
        );

        let user2 = User::new("user2").create(&pool).await.unwrap();
        assert_ne!(user1, user2);
        assert_eq!(
            user2.fetch(&pool).await.unwrap().unwrap(),
            User {
                nickname: "user2".to_owned()
            }
        );

        // Username *can* collide
        let user3 = User::new("user1").create(&pool).await.unwrap();
        assert_ne!(user3, user1);
        assert_eq!(
            user3.fetch(&pool).await.unwrap().unwrap(),
            User {
                nickname: "user1".to_owned()
            }
        );

        let (count,): (i64,) = sqlx::query_as("select count(*) from users")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 3);
    }
}
