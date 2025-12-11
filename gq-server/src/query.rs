//! Main query entry point

use async_graphql::Object;
use derivative::Derivative;

mod users;

#[derive(Debug, Derivative)]
#[derivative(Default = "new")]
pub struct Query {
    /// User related queries
    users: users::UsersQueries,
}

#[Object]
impl Query {
    async fn users(&self) -> &users::UsersQueries {
        &self.users
    }
}
