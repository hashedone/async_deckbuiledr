//! Main query entry point
use derivative::Derivative;
use juniper::GraphQLObject;

use crate::context::Context;
mod users;

#[derive(Debug, GraphQLObject, Derivative)]
#[derivative(Default(new = "true"))]
#[graphql(context = Context)]
pub struct Query {
    /// User related queries
    users: users::UsersQueries,
}
