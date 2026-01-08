//! Game model

use async_graphql::scalar;
use color_eyre::Result;
use color_eyre::eyre::ensure;
use serde::{Deserialize, Serialize};
use sqlx::prelude::Type;
use thiserror::Error;
use uuid::Uuid;

use crate::model::users::UserId;

#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Starting game {0} failed")]
    CannotStartGame(GameId),
    #[error("Missing player")]
    MissingPlayer,
}

/// Game ID newtype
#[derive(Debug, Clone, Copy, PartialEq, Type, Serialize, Deserialize)]
#[sqlx(transparent)]
#[serde(transparent)]
pub struct GameId(Uuid);

impl std::fmt::Display for GameId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

scalar!(GameId);

/// Game in the lobby
#[derive(Debug, Clone)]
pub struct LobbyGame {
    /// Game ID
    id: GameId,
    /// Created by user ID
    created_by: UserId,
    /// Player 1 ID
    pub player1: Option<UserId>,
    /// Player 2 ID
    pub player2: Option<UserId>,
}

impl LobbyGame {
    /// Returns the game ID
    pub fn id(&self) -> GameId {
        self.id
    }

    /// Returns the game creator id
    pub fn created_by(&self) -> UserId {
        self.created_by
    }

    /// Checks if the user is involved in a game
    pub fn is_involved(&self, user_id: UserId) -> bool {
        self.created_by == user_id || self.player1 == Some(user_id) || self.player2 == Some(user_id)
    }

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

    /// Fetches the lobby game by it's id
    pub async fn fetch(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        id: GameId,
    ) -> Result<Option<Self>> {
        let row = sqlx::query_as("select id, created_by, player1, player2 from lobby where id = ?")
            .bind(&id)
            .fetch_optional(db)
            .await?;

        Ok(row.map(|(id, created_by, player1, player2)| Self {
            id,
            created_by,
            player1,
            player2,
        }))
    }

    /// Updates the game state in DB
    pub async fn update(&self, db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>) -> Result<()> {
        sqlx::query("update lobby set player1 = ?, player2 = ? where id = ?")
            .bind(self.player1)
            .bind(self.player2)
            .bind(self.id)
            .execute(db)
            .await?;

        Ok(())
    }

    /// Starts the game - creates an entry in `games` table and removing it from the `lobby`
    pub async fn start(self, db: impl sqlx::Acquire<'_, Database = sqlx::Sqlite>) -> Result<Game> {
        let Self {
            id,
            created_by,
            player1,
            player2,
        } = self;

        let player1 = player1.ok_or(Error::MissingPlayer)?;
        let player2 = player2.ok_or(Error::MissingPlayer)?;

        let id = Game::start(db, id).await?;

        Ok(Game {
            id,
            created_by,
            player1,
            player2,
        })
    }
}

/// Ongoing game
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    /// Game ID
    id: GameId,
    /// User that created the game
    created_by: UserId,
    /// Player 1 ID
    player1: UserId,
    /// Player 2 ID
    player2: UserId,
}

impl Game {
    pub fn id(&self) -> GameId {
        self.id
    }

    pub fn created_by(&self) -> UserId {
        self.created_by
    }

    pub fn player1(&self) -> UserId {
        self.player1
    }

    pub fn player2(&self) -> UserId {
        self.player2
    }

    /// Starts a game without fetching it first from a lobby.
    ///
    /// The function still makes sure that the game exists in the lobby and will fail otherwise. Return started game id.
    pub async fn start(
        db: impl sqlx::Acquire<'_, Database = sqlx::Sqlite>,
        id: GameId,
    ) -> Result<GameId> {
        let mut tx = db.begin().await?;

        let insert = sqlx::query(
            "insert into games (id, created_by, player1, player2)\
             select id, created_by, player1, player2 from lobby where id = ?",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;

        ensure!(insert.rows_affected() == 1, Error::CannotStartGame(id));

        let delete = sqlx::query("delete from lobby where id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;

        ensure!(delete.rows_affected() == 1, Error::CannotStartGame(id));

        tx.commit().await?;

        Ok(id)
    }

    pub async fn fetch(
        db: impl sqlx::Executor<'_, Database = sqlx::Sqlite>,
        id: GameId,
    ) -> Result<Option<Self>> {
        let game =
            sqlx::query_as("select id, created_by, player1, player2 from games where id = ?")
                .bind(&id)
                .fetch_optional(db)
                .await?
                .map(|(id, created_by, player1, player2)| Game {
                    id,
                    created_by,
                    player1,
                    player2,
                });

        Ok(game)
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

        let fetched1 = LobbyGame::fetch(&pool, game1.id.clone()).await.unwrap();
        let fetched1 = fetched1.expect("game1 should exist");
        assert_eq!(fetched1.created_by, user);
        assert_eq!(fetched1.player1, None);
        assert_eq!(fetched1.player2, None);
        assert_eq!(fetched1.id, game1.id);

        let game2 = LobbyGame::create(&pool, user).await.unwrap();
        assert_eq!(game2.created_by, user);
        assert_eq!(game2.player1, None);
        assert_eq!(game2.player2, None);

        let fetched2 = LobbyGame::fetch(&pool, game2.id.clone()).await.unwrap();
        let fetched2 = fetched2.expect("game2 should exist");
        assert_eq!(fetched2.created_by, user);
        assert_eq!(fetched2.player1, None);
        assert_eq!(fetched2.player2, None);
        assert_eq!(fetched2.id, game2.id);

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

    #[tokio::test]
    async fn starting_game_from_lobby() {
        let pool = setup_pool().await;

        let player1 = User::new("player1").create(&pool).await.unwrap();
        let player2 = User::new("player2").create(&pool).await.unwrap();

        let mut lobby_game = LobbyGame::create(&pool, player1).await.unwrap();
        lobby_game.player1 = Some(player1);
        lobby_game.player2 = Some(player2);
        lobby_game.update(&pool).await.unwrap();

        let game_id = lobby_game.id();
        let started_game = lobby_game.start(&pool).await.unwrap();
        assert_eq!(started_game.id(), game_id);
        assert_eq!(started_game.created_by(), player1);
        assert_eq!(started_game.player1(), player1);
        assert_eq!(started_game.player2(), player2);

        let lobby_row: Option<(GameId,)> = sqlx::query_as("select id from lobby where id = ?")
            .bind(game_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(lobby_row.is_none());

        let game_row: (GameId, UserId, UserId, UserId) =
            sqlx::query_as("select id, created_by, player1, player2 from games where id = ?")
                .bind(game_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(game_row, (game_id, player1, player1, player2));

        let fetched_game = Game::fetch(&pool, game_id).await.unwrap();
        let fetched_game = fetched_game.unwrap();
        assert_eq!(fetched_game.id(), game_id);
        assert_eq!(fetched_game.created_by(), player1);
        assert_eq!(fetched_game.player1(), player1);
        assert_eq!(fetched_game.player2(), player2);
    }

    #[tokio::test]
    async fn starting_game_directly() {
        let pool = setup_pool().await;

        let player1 = User::new("player1").create(&pool).await.unwrap();
        let player2 = User::new("player2").create(&pool).await.unwrap();

        let mut lobby_game = LobbyGame::create(&pool, player1).await.unwrap();
        lobby_game.player1 = Some(player1);
        lobby_game.player2 = Some(player2);
        lobby_game.update(&pool).await.unwrap();

        let game_id = lobby_game.id();
        let started_game_id = Game::start(&pool, game_id).await.unwrap();
        assert_eq!(started_game_id, game_id);

        let lobby_row: Option<(GameId,)> = sqlx::query_as("select id from lobby where id = ?")
            .bind(game_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
        assert!(lobby_row.is_none());

        let game_row: (GameId, UserId, UserId, UserId) =
            sqlx::query_as("select id, created_by, player1, player2 from games where id = ?")
                .bind(game_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(game_row, (game_id, player1, player1, player2));

        let fetched_game = Game::fetch(&pool, game_id).await.unwrap();
        let fetched_game = fetched_game.unwrap();
        assert_eq!(fetched_game.id(), game_id);
        assert_eq!(fetched_game.created_by(), player1);
        assert_eq!(fetched_game.player1(), player1);
        assert_eq!(fetched_game.player2(), player2);
    }

    #[tokio::test]
    async fn arbitrary_game_is_not_fetched() {
        let pool = setup_pool().await;

        let game_id = GameId(Uuid::new_v4());

        let lobby_game = LobbyGame::fetch(&pool, game_id).await.unwrap();
        assert!(lobby_game.is_none());

        let game = Game::fetch(&pool, game_id).await.unwrap();
        assert!(game.is_none());
    }
}
