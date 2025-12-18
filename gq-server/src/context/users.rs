//! Serivce users storage

use async_graphql::{Result, SimpleObject};
use sqlx::Executor;
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Invalid user id format")]
    InvalidUserId,
}

/// User queryable data
#[derive(Debug, Clone, PartialEq, SimpleObject)]
pub struct User {
    /// How user is visible to others.
    pub nickname: String,
}

// Users access
pub struct Users<'a, Db> {
    /// Database connection
    db: &'a Db,
}

impl<'a, Db> Users<'a, Db> {
    /// Creates new users accessor
    pub(super) fn new(db: &'a Db) -> Self {
        Self { db }
    }
}

impl<'a, Db> Users<'a, Db>
where
    &'a Db: Executor<'a, Database = sqlx::Sqlite>,
{
    /// Returns single user by their id
    pub async fn user(&self, user_id: i64) -> Result<Option<User>> {
        let row: Option<(String,)> = sqlx::query_as("select nickname from users where id = ?")
            .bind(user_id)
            .fetch_optional(self.db)
            .await?;

        Ok(row.map(|(nickname,)| User { nickname }))
    }

    /// Create a new user returning created user id.
    pub async fn create(&self, nickname: impl Into<String>) -> Result<i64> {
        let nickname = nickname.into();
        let result = sqlx::query("insert into users(nickname) values (?)")
            .bind(nickname)
            .execute(self.db)
            .await?;

        Ok(result.last_insert_rowid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    const USERS_MIGRATION: &str = include_str!("../../model/migrations/0_users.sql");

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(USERS_MIGRATION).execute(&pool).await.unwrap();
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
        let users = Users::new(&pool);

        let user1 = users.create("user1").await.unwrap();
        assert_eq!(
            users.user(user1).await.unwrap().unwrap(),
            User {
                nickname: "user1".to_owned()
            }
        );

        let user2 = users.create("user2").await.unwrap();
        assert_ne!(user1, user2);
        assert_eq!(
            users.user(user2).await.unwrap().unwrap(),
            User {
                nickname: "user2".to_owned()
            }
        );

        // Username *can* collide
        let user3 = users.create("user1").await.unwrap();
        assert_ne!(user3, user1);
        assert_eq!(
            users.user(user3).await.unwrap().unwrap(),
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
