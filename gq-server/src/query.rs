//! Main query entry point

use async_graphql::{Context, Object, Result, SimpleObject};

use crate::model::Model;
use crate::model::game::{GameId, LobbyGame};
use crate::model::users::{User, UserId};

#[derive(Debug, Default)]
pub struct Query;

#[derive(Debug, Clone, SimpleObject)]
pub struct GameInfo {
    pub created_by: UserId,
    pub players: Vec<UserId>,
}

#[Object]
impl Query {
    /// Gets user by their id
    pub async fn user<'c>(&self, ctx: &Context<'c>, id: UserId) -> Result<Option<User>> {
        let model: &Model = ctx.data()?;
        let db = model.db();

        let user = User::fetch(db, id).await?;
        Ok(user)
    }

    /// Gets the game in lobby info by it's id
    pub async fn lobby<'c>(&self, ctx: &Context<'c>, id: GameId) -> Result<Option<GameInfo>> {
        let model: &Model = ctx.data()?;
        let db = model.db();

        let game = LobbyGame::fetch(db, id).await?;
        let info = game.map(|game| GameInfo {
            created_by: game.created_by,
            players: [game.player1.into_iter(), game.player2.into_iter()]
                .into_iter()
                .flatten()
                .collect(),
        });

        Ok(info)
    }
}
