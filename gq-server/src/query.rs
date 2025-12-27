//! Main query entry point

use async_graphql::{Context, Object, Result};

use crate::model::{
    Model,
    users::{User, UserId},
};

#[derive(Debug, Default)]
pub struct Query;

#[Object]
impl Query {
    /// Gets user by their id
    pub async fn user<'c>(&self, ctx: &Context<'c>, id: UserId) -> Result<Option<User>> {
        let model: &Model = ctx.data()?;
        let db = model.db();

        let user = User::fetch(db, id).await?;
        Ok(user)
    }
}
