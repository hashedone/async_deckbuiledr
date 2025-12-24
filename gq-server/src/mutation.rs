//! Mutations main entry point

use async_graphql::Object;
use derivative::Derivative;

mod users;

#[derive(Debug, Derivative)]
#[derivative(Default = "new")]
pub struct Mutation {
    users: users::UsersMutations,
}

#[Object]
impl Mutation {
    async fn users(&self) -> &users::UsersMutations {
        &self.users
    }
}
