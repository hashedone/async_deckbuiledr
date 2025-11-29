//! User-related mutations

use juniper::{GraphQLObject, graphql_object};

use crate::context::Context;
use crate::context::users::{UserId, Users as UsersContext};

/// Type returned when the AD-hoc user is created
#[derive(Debug, Clone, GraphQLObject)]
#[graphql(context = UsersContext)]
struct CreatedAdHocUser {
    /// Created user id
    user: UserId,
    /// Authorization token for this user
    token: String,
}

#[derive(Debug, Default)]
pub struct UsersMutations;

#[graphql_object]
impl UsersMutations {
    /// Creates a short living user authorized with a token. Returns user authorization token.
    ///
    /// Ad hoc users are users that are cannot grant any priviliges - they are created ad hoc, when
    /// the basic user authentication is needed. The ad hoc user can be removed if not assigned to
    /// anything.
    async fn create_adhoc(nickname: String, context: &Context) -> CreatedAdHocUser {
        let user_id = context.users().create(nickname).await;
        let token = context.auth().create_user_token(user_id).await;

        CreatedAdHocUser {
            user: user_id,
            token,
        }
    }
}
