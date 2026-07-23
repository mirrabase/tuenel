use chrono::{DateTime, Utc};

pub fn expired(expires_at: DateTime<Utc>, now: DateTime<Utc>) -> bool {
    expires_at <= now
}
