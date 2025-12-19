//! Session information

use chrono::{DateTime, Utc};

/// Session data
#[derive(Debug, Clone)]
pub struct Session {
    /// User ID for this session
    pub user_id: i64,
    /// Session token
    pub token: String,
    /// Session expiration time
    pub expires_at: DateTime<Utc>,
}
