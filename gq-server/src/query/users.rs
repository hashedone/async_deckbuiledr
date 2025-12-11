//! User-related queries

use async_graphql::Object;

#[derive(Debug, Default)]
pub struct UsersQueries;

#[Object]
impl UsersQueries {
    async fn dummy(&self) -> &str {
        "Dummy"
    }
}
