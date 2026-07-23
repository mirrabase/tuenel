use std::sync::Arc;

use gateway_types::{InspectionContent, InspectionContext, SecurityFinding};

use crate::{SecurityDecision, SecurityInspector, SecurityPolicy, decide};

#[derive(Clone, Default)]
pub struct SecurityPipeline {
    inspectors: Arc<Vec<Arc<dyn SecurityInspector>>>,
}

impl SecurityPipeline {
    pub fn new(inspectors: Vec<Arc<dyn SecurityInspector>>) -> Self {
        Self {
            inspectors: Arc::new(inspectors),
        }
    }

    pub async fn inspect(
        &self,
        policy: &SecurityPolicy,
        context: &InspectionContext,
        content: &InspectionContent,
    ) -> SecurityDecision {
        if !policy.enabled {
            return decide(policy, Vec::new(), false);
        }
        let size = serde_json::to_vec(content)
            .map_or(policy.maximum_content_bytes.saturating_add(1), |value| {
                value.len()
            });
        if size > policy.maximum_content_bytes {
            return decide(policy, Vec::new(), true);
        }
        let mut findings = Vec::<SecurityFinding>::new();
        let mut failed = false;
        for inspector in self.inspectors.iter() {
            match inspector.inspect(context, content).await {
                Ok(mut values) => findings.append(&mut values),
                Err(_) => failed = true,
            }
        }
        decide(policy, findings, failed)
    }
}
