//! Service global context

pub mod auth;
pub mod users;

use auth::Auth;
use std::sync::Arc;
pub use users::Users;

struct ContextInner {
    /// Users manager
    users: Users,
    /// Authorisation manager
    auth: Auth,
}

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

impl Context {
    pub fn new() -> Context {
        Self(Arc::new(ContextInner {
            users: Users::new(),
            auth: Auth::new(),
        }))
    }

    /// Access to `Users`
    pub fn users(&self) -> &Users {
        &self.0.users
    }

    /// Access to auth
    pub fn auth(&self) -> &Auth {
        &self.0.auth
    }
}

impl juniper::Context for Context {}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}
