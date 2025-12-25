//! Game model

use async_graphql::scalar;
use color_eyre::Result;
use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;
use uuid::Uuid;

use crate::model::users::UserId;

/// Game ID newtype
#[derive(Debug, Clone, PartialEq, Type, Serialize, Deserialize)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct GameId(Uuid);

scalar!(GameId);

/// Game in the lobby
#[derive(Debug, Clone)]
pub struct LobbyGame {
    /// Game ID
    pub id: GameId,
    /// Created by user ID
    pub created_by: UserId,
    /// Player 1 ID
    pub player1: Option<UserId>,
    /// Player 2 ID
    pub player2: Option<UserId>,
}

impl LobbyGame {
    /// Creates a new game in the lobby
    pub async fn create(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        created_by: UserId,
    ) -> Result<Self> {
        let id = GameId(Uuid::new_v4());
        sqlx::query("insert into lobby(id, created_by) values (?, ?)")
            .bind(&id)
            .bind(created_by)
            .execute(db)
            .await?;

        Ok(Self {
            id,
            created_by,
            player1: None,
            player2: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::model::users::User;

    use super::*;
    use sqlx::SqlitePool;

    async fn setup_pool() -> SqlitePool {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("model/migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn create_game() {
        let pool = setup_pool().await;

        let user = User::new("user1").create(&pool).await.unwrap();

        let game1 = LobbyGame::create(&pool, user).await.unwrap();
        assert_eq!(game1.created_by, user);
        assert_eq!(game1.player1, None);
        assert_eq!(game1.player2, None);

        let game2 = LobbyGame::create(&pool, user).await.unwrap();
        assert_eq!(game2.created_by, user);
        assert_eq!(game2.player1, None);
        assert_eq!(game2.player2, None);

        assert_ne!(game1.id, game2.id);

        let (created_by, player1, player2): (UserId, Option<UserId>, Option<UserId>) =
            sqlx::query_as("select created_by, player1, player2 from lobby where id = ?")
                .bind(&game1.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((user, None, None), (created_by, player1, player2));

        let (created_by, player1, player2): (UserId, Option<UserId>, Option<UserId>) =
            sqlx::query_as("select created_by, player1, player2 from lobby where id = ?")
                .bind(&game2.id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!((user, None, None), (created_by, player1, player2));

        let (count,): (i64,) = sqlx::query_as("select count(*) from lobby")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 2);
    }
}
