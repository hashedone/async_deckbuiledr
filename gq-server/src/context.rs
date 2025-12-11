//! Service global context

pub mod auth;
pub mod users;

use async_graphql::EmptySubscription;
pub use auth::Auth;
use derivative::Derivative;
pub use users::Users;

use crate::mutation::Mutation;
use crate::query::Query;
use crate::service::Schema;

#[derive(Derivative, Clone)]
#[derivative(Default = "new")]
pub struct Context {
    auth: Auth,
    users: Users,
}

/// Builds the schema with context included
impl Context {
    pub fn schema(&self) -> Schema {
        Schema::build(Query::new(), Mutation::new(), EmptySubscription)
            .data(self.users.clone())
            .data(self.auth.clone())
            .finish()
    }
}
