//! Mutations main entry point

use async_graphql::SimpleObject;
use derivative::Derivative;

mod users;

#[derive(Debug, Derivative, SimpleObject)]
#[derivative(Default = "new")]
pub struct Mutation {
    users: users::UsersMutations,
}
