//! User-related queries

use juniper::{FieldResult, graphql_object};

use crate::context::Users as UsersContext;
use crate::context::users::UserId;

#[derive(Debug, Default)]
pub struct UsersQueries;

#[graphql_object]
impl UsersQueries {
    /// Queries for all user ids
    async fn all(context: &UsersContext) -> Vec<UserId> {
        context
            .users()
            .await
            .keys()
            .copied()
            .map(UserId::new)
            .collect()
    }

    /// Builds user id for further queries
    fn id(user_id: String) -> FieldResult<UserId> {
        user_id.parse().map_err(Into::into)
    }
}
