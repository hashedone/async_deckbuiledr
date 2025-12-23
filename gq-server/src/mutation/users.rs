//! User-related mutations

use async_graphql::{Context, Object, Result, SimpleObject};
use tracing::{info, instrument};

use crate::model::Model;
use crate::model::auth::AdHocToken;
use crate::model::users::User;

/// Type returned when the AD-hoc user is created
#[derive(Debug, Clone, SimpleObject)]
struct CreatedAdHocUser {
    /// Created user info
    user: User,
    /// Authorization token for this user
    token: AdHocToken,
}

#[derive(Debug, Default)]
pub struct UsersMutations;

#[Object]
impl UsersMutations {
    /// Creates a short living user authorized with a token. Returns user authorization token.
    ///
    /// Ad hoc users are users that are cannot grant any priviliges - they are created ad hoc, when
    /// the basic user authentication is needed. The ad hoc user can be removed if not assigned to
    /// anything.
    #[instrument(skip(self, context))]
    async fn create_adhoc<'c>(
        &self,
        context: &Context<'c>,
        nickname: String,
    ) -> Result<CreatedAdHocUser> {
        let context: &Model = context.data()?;
        let db = context.db();

        let user = User { nickname };
        let user_id = user.clone().create(db).await?;
        let token = user_id.create_adhoc_token(db).await?;

        info!(%user_id, nickname=user.nickname, "Created ad-hoc user");

        Ok(CreatedAdHocUser { user, token })
    }
}
