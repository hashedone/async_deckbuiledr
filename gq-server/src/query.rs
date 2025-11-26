use juniper::graphql_object;

use crate::context::Context;

pub struct Query;

#[graphql_object(Context = Context)]
impl Query {
    /// Dummy query to make the GraphQL setup online
    fn dummy() -> &'static str {
        ""
    }
}
