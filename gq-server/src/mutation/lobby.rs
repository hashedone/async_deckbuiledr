//! Lobby related mutations

use async_graphql::{Context, Object, Result};
use tracing::{info, instrument};

use crate::model::Model;
use crate::model::auth::Session;
use crate::model::game::{GameId, LobbyGame};

#[derive(Debug, Default)]
pub struct LobbyMutations;

#[Object]
impl LobbyMutations {
    /// Creates a new game in the lobby. Returns created game id. Game id should be passed to players
    /// so they can join the game.
    #[instrument(skip(self, ctx))]
    pub async fn create_game(&self, ctx: &Context<'_>) -> Result<GameId> {
        let session: &Session = ctx.data_opt().ok_or("Unauthorized")?;
        let model: &Model = ctx.data()?;
        let db = model.db();

        let game = LobbyGame::create(db, session.user_id).await?;
        info!(?game, "Created game in the lobby");

        Ok(game.id())
    }

    /// Takes a seat in the lobby game.
    ///
    /// Game id is returned as a result.
    #[instrument(skip(self, ctx))]
    pub async fn join_game(&self, ctx: &Context<'_>, game_id: GameId) -> Result<GameId> {
        let session: &Session = ctx.data_opt().ok_or("Unauthorized")?;
        let model: &Model = ctx.data()?;
        let db = model.db();
        let mut game = LobbyGame::fetch(db, game_id)
            .await?
            .ok_or("Game not found")?;

        if game.player1.is_none() {
            game.player1 = Some(session.user_id);
        } else if game.player2.is_none() {
            game.player2 = Some(session.user_id);
        } else {
            return Err("Game is full".into());
        }

        game.update(db).await?;

        info!(?game, "Joined game in the lobby");

        if game.player1.is_some() && game.player2.is_some() {
            info!(?game, "Game is ready to start");
        }

        Ok(game.id())
    }
}
