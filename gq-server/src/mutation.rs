use juniper::graphql_object;

use crate::context::Context;

pub struct Mutation;

#[graphql_object(Context = Context)]
impl Mutation {
    /// Dummy mutation to make the GraphQL setup online
    fn dummy() -> &'static str {
        ""
    }
}
