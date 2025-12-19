//! User-related mutations

use async_graphql::{Context, Object, Result, SimpleObject};
use tracing::{info, instrument};

use crate::context::Model;
use crate::context::users::User;

/// Type returned when the AD-hoc user is created
#[derive(Debug, Clone, SimpleObject)]
struct CreatedAdHocUser {
    /// Created user info
    user: User,
    /// Authorization token for this user
    token: String,
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
        let users = context.users();
        let user_id = users.create(&nickname).await?;

        let auth = context.auth();
        let token = auth.create_user_token(user_id).await?;

        info!(%user_id, nickname, "Created ad-hoc user");

        Ok(CreatedAdHocUser {
            user: User { nickname },
            token,
        })
    }
}
