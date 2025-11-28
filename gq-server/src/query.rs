use juniper::{FieldResult, graphql_object};

use crate::context::Context;
use crate::context::users::UserId;

pub struct Query;

#[graphql_object]
impl Query {
    /// Queries all user IDs
    async fn users(context: &Context) -> Vec<UserId> {
        context
            .users()
            .users()
            .await
            .keys()
            .copied()
            .map(UserId::new)
            .collect()
    }

    /// Constructs `UserId` from input for future queries
    fn user_id(user_id: String) -> FieldResult<UserId> {
        user_id.parse().map_err(Into::into)
    }
}
