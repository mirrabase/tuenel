use chrono::{DateTime, Utc};
use gateway_types::{IncidentId, IncidentStatus};

#[derive(Clone, Debug, serde::Serialize)]
pub struct IncidentTimelineEntry {
    pub entry_id: uuid::Uuid,
    pub incident_id: IncidentId,
    pub status: IncidentStatus,
    pub actor: String,
    pub sanitized_note: Option<String>,
    pub occurred_at: DateTime<Utc>,
}
