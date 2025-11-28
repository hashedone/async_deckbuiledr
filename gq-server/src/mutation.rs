//! Mutations main entry point
//!
use derivative::Derivative;
use juniper::GraphQLObject;

mod users;

use crate::context::Context;

#[derive(Debug, GraphQLObject, Derivative)]
#[derivative(Default(new = "true"))]
#[graphql(context = Context)]
pub struct Mutation {
    users: users::UsersMutations,
}
