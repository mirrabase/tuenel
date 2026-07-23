use std::{sync::Arc, time::Instant};

use gateway_approval::{ApprovalService, ExecutionClaim};
use gateway_events::AuditService;
use gateway_security::SecurityEnforcer;
use gateway_types::{
    ApprovalId, ApprovalResourceType, GatewayMcpInvocation, GatewayMcpResult, InspectionContent,
    InspectionContext, McpUsageDetails, Principal, SecurityAction,
};
use sha2::{Digest, Sha256};
use tokio::time::{Duration, timeout};
use tracing::Instrument;
use uuid::Uuid;

use crate::{
    McpError, McpPolicyRepository, McpQuota, McpRegistry, McpUsageEvent, McpUsageRepository,
    classify_tool,
};

#[derive(Clone, Debug, Default)]
pub struct ApprovalReference {
    pub approval_id: Option<ApprovalId>,
    pub idempotency_key: Option<String>,
}

#[derive(Clone)]
pub struct McpInvocationService {
    registry: McpRegistry,
    policies: Arc<dyn McpPolicyRepository>,
    quota: Arc<dyn McpQuota>,
    usage: Arc<dyn McpUsageRepository>,
    approval: ApprovalService,
    audit: AuditService,
    approval_enabled: bool,
    security: SecurityEnforcer,
}

impl McpInvocationService {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registry: McpRegistry,
        policies: Arc<dyn McpPolicyRepository>,
        quota: Arc<dyn McpQuota>,
        usage: Arc<dyn McpUsageRepository>,
        approval: ApprovalService,
        approval_enabled: bool,
        audit: AuditService,
        security: SecurityEnforcer,
    ) -> Self {
        Self {
            registry,
            policies,
            quota,
            usage,
            approval,
            approval_enabled,
            audit,
            security,
        }
    }

    pub async fn invoke(
        &self,
        request_id: Uuid,
        principal: Principal,
        invocation: GatewayMcpInvocation,
        approval: ApprovalReference,
    ) -> Result<GatewayMcpResult, McpError> {
        let started = Instant::now();
        self.registry.server_for(&principal, invocation.server_id).instrument(tracing::info_span!("gateway.mcp.resolve_server",request_id=%request_id,tenant_id=%principal.tenant_id,mcp_server_id=%invocation.server_id)).await?;
        let tool = self
            .registry
            .tools(&principal, Some(invocation.server_id))
            .await?
            .into_iter()
            .find(|tool| tool.tool_name == invocation.tool_name)
            .ok_or(McpError::ToolUnavailable)?;
        let policy = self.policies.resolved_policy(&principal).await?;
        let request_bytes = serde_json::to_vec(&invocation.arguments)
            .map_err(|_| McpError::Invalid)?
            .len() as u64;
        let risk = policy
            .risk_overrides
            .get(&invocation.tool_name)
            .copied()
            .unwrap_or_else(|| classify_tool(&tool));
        if let Err(error) = policy.authorize(
            invocation.server_id,
            &invocation.tool_name,
            &invocation.arguments,
        ) {
            gateway_observability::metrics().mcp_policy_denials.inc();
            self.record(
                request_id,
                &principal,
                &invocation,
                risk,
                request_bytes,
                0,
                false,
                "policy_denied",
                started,
            )
            .await;
            self.audit(
                request_id,
                &principal,
                &invocation,
                "mcp.tool.denied",
                "policy",
            )
            .await;
            return Err(error);
        }
        tracing::info_span!("gateway.mcp.authorize_tool",request_id=%request_id,mcp_server_id=%invocation.server_id,mcp_tool_name=%invocation.tool_name,tool_risk_level=?risk).in_scope(||{});
        if policy
            .maximum_request_bytes
            .is_some_and(|limit| request_bytes > limit)
        {
            return Err(McpError::TooLarge);
        }
        let context = InspectionContext {
            request_id,
            tenant_id: principal.tenant_id.clone(),
            project_id: principal.project_id.clone(),
            principal_id: principal.principal_id.clone(),
            stage: "mcp_arguments".into(),
            tool_risk: Some(risk),
        };
        let (arguments, decision) = self.security.assess(context.clone(), InspectionContent::ToolArguments(invocation.arguments.clone())).instrument(tracing::info_span!("gateway.mcp.inspect_arguments",request_id=%request_id,mcp_server_id=%invocation.server_id,mcp_tool_name=%invocation.tool_name)).await.map_err(|_| McpError::Unavailable)?;
        if decision.action == SecurityAction::Block {
            return Err(McpError::ToolNotAllowed);
        }
        let mut invocation = invocation;
        if let InspectionContent::ToolArguments(value) = arguments {
            invocation.arguments = value;
        }
        let request_hash = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&invocation).map_err(|_| McpError::Invalid)?)
        );
        let needs_approval = policy.action(&tool, risk) == SecurityAction::RequireApproval
            || decision.action == SecurityAction::RequireApproval;
        if needs_approval {
            if !self.approval_enabled {
                return Err(McpError::ToolNotAllowed);
            }
            match (approval.approval_id, approval.idempotency_key.as_deref()) {
                (Some(approval_id), Some(key)) => match self
                    .approval
                    .authorize_retry(&principal, approval_id, key, &request_hash)
                    .await
                    .map_err(|error| map_approval(error, approval_id))?
                {
                    ExecutionClaim::Completed(value) => {
                        return serde_json::from_value(value).map_err(|_| McpError::Unavailable);
                    }
                    ExecutionClaim::Indeterminate => return Err(McpError::Transport),
                    ExecutionClaim::Claimed => {}
                },
                _ => {
                    let request = self
                        .approval
                        .create(
                            &principal,
                            request_id,
                            ApprovalResourceType::McpTool,
                            invocation.server_id.to_string(),
                            invocation.tool_name.clone(),
                            serde_json::json!({"arguments":"[SANITIZED]"}),
                            risk,
                            request_hash.clone(),
                        )
                        .await
                        .map_err(|_| McpError::Unavailable)?;
                    gateway_observability::metrics().mcp_approval_requests.inc();
                    self.audit(
                        request_id,
                        &principal,
                        &invocation,
                        "mcp.tool.approval_required",
                        "pending",
                    )
                    .await;
                    self.audit(
                        request_id,
                        &principal,
                        &invocation,
                        "approval.requested",
                        "pending",
                    )
                    .await;
                    return Err(McpError::ApprovalRequired(request.approval_id));
                }
            }
        }
        let reservation = self
            .quota
            .reserve(
                &principal,
                invocation.server_id,
                &invocation.tool_name,
                &policy,
            )
            .await?;
        let result = timeout(Duration::from_millis(policy.maximum_execution_ms.unwrap_or(30_000)), self.registry.invoke(&principal, invocation.clone())).instrument(tracing::info_span!("gateway.mcp.invoke",request_id=%request_id,mcp_server_id=%invocation.server_id,mcp_tool_name=%invocation.tool_name,approval_required=needs_approval)).await.map_err(|_| McpError::Timeout).and_then(|value| value);
        let release = self.quota.release(reservation).await;
        if release.is_err() {
            tracing::warn!(request_id = %request_id, "failed to release MCP quota reservation");
        }
        let mut result = match result {
            Ok(result) => result,
            Err(error) => {
                self.record(
                    request_id,
                    &principal,
                    &invocation,
                    risk,
                    request_bytes,
                    0,
                    needs_approval,
                    "failed",
                    started,
                )
                .await;
                self.audit(
                    request_id,
                    &principal,
                    &invocation,
                    "mcp.tool.failed",
                    "failed",
                )
                .await;
                if let (Some(approval_id), Some(key)) =
                    (approval.approval_id, approval.idempotency_key.as_deref())
                {
                    let _ = self.approval.fail(approval_id, key, true).await;
                }
                return Err(error);
            }
        };
        let response_bytes = serde_json::to_vec(&result)
            .map_err(|_| McpError::Invalid)?
            .len() as u64;
        if policy
            .maximum_response_bytes
            .is_some_and(|limit| response_bytes > limit)
        {
            return Err(McpError::TooLarge);
        }
        let result_value = serde_json::to_value(&result).map_err(|_| McpError::Invalid)?;
        let (inspected_result, result_decision) = self.security.assess(InspectionContext { stage: "mcp_result".into(), ..context }, InspectionContent::ToolResult(result_value)).instrument(tracing::info_span!("gateway.mcp.inspect_result",request_id=%request_id,mcp_server_id=%invocation.server_id,mcp_tool_name=%invocation.tool_name)).await.map_err(|_| McpError::Unavailable)?;
        match result_decision.action {
            SecurityAction::Block | SecurityAction::RequireApproval => {
                self.record(
                    request_id,
                    &principal,
                    &invocation,
                    risk,
                    request_bytes,
                    response_bytes,
                    needs_approval,
                    "result_blocked",
                    started,
                )
                .await;
                return Err(McpError::ToolNotAllowed);
            }
            SecurityAction::Redact => {
                if let InspectionContent::ToolResult(value) = inspected_result {
                    result = serde_json::from_value(value).map_err(|_| McpError::Invalid)?;
                }
            }
            SecurityAction::Warn => {
                result
                    .metadata
                    .insert("security_warning".into(), serde_json::Value::Bool(true));
            }
            SecurityAction::Allow => {}
        }
        let details = McpUsageDetails {
            server_id: invocation.server_id,
            tool_name: invocation.tool_name.clone(),
            invocation_count: 1,
            duration_ms: started.elapsed().as_millis() as u64,
            request_bytes,
            response_bytes,
            risk_level: risk,
            approval_required: needs_approval,
        };
        self.usage
            .record_mcp_usage(McpUsageEvent::new(
                request_id,
                &principal,
                details,
                if result.is_error {
                    "failed"
                } else {
                    "succeeded"
                },
            ))
            .await?;
        let metrics = gateway_observability::metrics();
        let status = if result.is_error {
            "failed"
        } else {
            "succeeded"
        };
        let risk_label = format!("{:?}", risk).to_ascii_lowercase();
        metrics
            .mcp_invocations
            .with_label_values(&[status, &risk_label])
            .inc();
        metrics
            .mcp_duration
            .with_label_values(&[status])
            .observe(started.elapsed().as_secs_f64());
        if result.is_error {
            metrics
                .mcp_failures
                .with_label_values(&["tool_error"])
                .inc();
        }
        self.audit(
            request_id,
            &principal,
            &invocation,
            if result.is_error {
                "mcp.tool.failed"
            } else {
                "mcp.tool.invoked"
            },
            if result.is_error {
                "failed"
            } else {
                "succeeded"
            },
        )
        .await;
        if let (Some(approval_id), Some(key)) =
            (approval.approval_id, approval.idempotency_key.as_deref())
        {
            self.approval
                .complete(
                    approval_id,
                    key,
                    serde_json::to_value(&result).map_err(|_| McpError::Invalid)?,
                )
                .await
                .map_err(|error| map_approval(error, approval_id))?;
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    async fn record(
        &self,
        request_id: Uuid,
        principal: &Principal,
        invocation: &GatewayMcpInvocation,
        risk: gateway_types::ToolRiskLevel,
        request_bytes: u64,
        response_bytes: u64,
        approval_required: bool,
        status: &str,
        started: Instant,
    ) {
        let details = McpUsageDetails {
            server_id: invocation.server_id,
            tool_name: invocation.tool_name.clone(),
            invocation_count: 0,
            duration_ms: started.elapsed().as_millis() as u64,
            request_bytes,
            response_bytes,
            risk_level: risk,
            approval_required,
        };
        let metrics = gateway_observability::metrics();
        let risk_label = format!("{:?}", risk).to_ascii_lowercase();
        metrics
            .mcp_invocations
            .with_label_values(&[status, &risk_label])
            .inc();
        metrics
            .mcp_duration
            .with_label_values(&[status])
            .observe(started.elapsed().as_secs_f64());
        if status != "succeeded" {
            metrics.mcp_failures.with_label_values(&[status]).inc();
        }
        if let Err(error) = self
            .usage
            .record_mcp_usage(McpUsageEvent::new(request_id, principal, details, status))
            .await
        {
            tracing::warn!(request_id=%request_id, error=%error, "failed to record MCP usage");
        }
    }

    async fn audit(
        &self,
        request_id: Uuid,
        principal: &Principal,
        invocation: &GatewayMcpInvocation,
        event: &str,
        status: &str,
    ) {
        let key = format!("{request_id}:{event}:{}", invocation.tool_name);
        let payload = serde_json::json!({"server_id":invocation.server_id,"tool_name":invocation.tool_name,"status":status});
        if let Err(error) = self
            .audit
            .emit(key, event, principal, Some(request_id), payload)
            .await
        {
            tracing::warn!(request_id=%request_id,error=%error,"failed to record MCP audit event");
        }
    }
}

fn map_approval(error: gateway_approval::ApprovalError, approval_id: ApprovalId) -> McpError {
    match error {
        gateway_approval::ApprovalError::Expired => McpError::ApprovalExpired,
        gateway_approval::ApprovalError::Rejected => McpError::ApprovalRejected,
        gateway_approval::ApprovalError::Forbidden | gateway_approval::ApprovalError::Replay => {
            McpError::ToolNotAllowed
        }
        gateway_approval::ApprovalError::Pending => McpError::ApprovalRequired(approval_id),
        _ => McpError::Unavailable,
    }
}
