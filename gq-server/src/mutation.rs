use juniper::{GraphQLObject, graphql_object};

use crate::context::Context;
use crate::context::users::UserId;

pub struct Mutation;

/// Type returned when the AD-hoc user is created
#[derive(Debug, Clone, GraphQLObject)]
#[graphql(context = Context)]
struct CreatedAdHocUser {
    /// Created user id
    user: UserId,
    /// Authorization token for this user
    token: String,
}

#[graphql_object(Context = Context)]
impl Mutation {
    /// Creates a short living user authorized with a token. Returns user authorization token.
    async fn create_adhoc_user(nickname: String, context: &Context) -> CreatedAdHocUser {
        let user_id = context.users().create_user(nickname).await;
        let token = context.auth().create_user_token(user_id).await;

        CreatedAdHocUser {
            user: user_id,
            token,
        }
    }
}
