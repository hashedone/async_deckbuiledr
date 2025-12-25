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

        Ok(game.id)
    }
}
