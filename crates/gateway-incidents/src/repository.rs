use async_trait::async_trait;
use gateway_types::{IncidentId, IncidentStatus, SecurityIncident};

use crate::{IncidentError, IncidentTimelineEntry};

#[async_trait]
pub trait IncidentRepository: Send + Sync {
    async fn insert_incident(&self, incident: SecurityIncident) -> Result<(), IncidentError>;
    async fn incident(
        &self,
        tenant_id: &str,
        incident_id: IncidentId,
    ) -> Result<Option<SecurityIncident>, IncidentError>;
    async fn list_incidents(
        &self,
        tenant_id: &str,
        status: Option<IncidentStatus>,
        limit: u32,
    ) -> Result<Vec<SecurityIncident>, IncidentError>;
    async fn update_incident(
        &self,
        tenant_id: &str,
        entry: IncidentTimelineEntry,
    ) -> Result<SecurityIncident, IncidentError>;
}
