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
    async fn create_adhoc(nickname: String, context: &Context) -> CreatedAdHocUser {
        let user_id = context.users().create(nickname).await;
        let token = context.auth().create_user_token(user_id).await;

        CreatedAdHocUser {
            user: user_id,
            token,
        }
    }
}
