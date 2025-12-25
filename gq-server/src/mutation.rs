//! Mutations main entry point

use async_graphql::Object;
use derivative::Derivative;

mod lobby;
mod users;

#[derive(Debug, Derivative)]
#[derivative(Default = "new")]
pub struct Mutation {
    users: users::UsersMutations,
    lobby: lobby::LobbyMutations,
}

#[Object]
impl Mutation {
    async fn users(&self) -> &users::UsersMutations {
        &self.users
    }

    async fn lobby(&self) -> &lobby::LobbyMutations {
        &self.lobby
    }
}
