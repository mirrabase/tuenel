-- v0.3 identity-aware MCP and AI security schema. All records are additive.
CREATE TABLE mcp_servers (
    server_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    name TEXT NOT NULL,
    description TEXT,
    transport_type TEXT NOT NULL CHECK (transport_type IN ('stdio', 'streamable_http')),
    endpoint TEXT,
    command TEXT,
    arguments JSONB NOT NULL DEFAULT '[]',
    environment_secret_refs JSONB NOT NULL DEFAULT '[]',
    credential_ref TEXT REFERENCES secret_material(secret_ref),
    enabled BOOLEAN NOT NULL DEFAULT true,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name),
    CHECK ((transport_type = 'stdio' AND command IS NOT NULL AND endpoint IS NULL)
        OR (transport_type = 'streamable_http' AND endpoint IS NOT NULL AND command IS NULL))
);

CREATE TABLE mcp_server_credentials (
    credential_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    server_id UUID NOT NULL REFERENCES mcp_servers(server_id) ON DELETE CASCADE,
    secret_ref TEXT NOT NULL REFERENCES secret_material(secret_ref),
    credential_kind TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (server_id, credential_kind)
);

CREATE TABLE mcp_tools (
    server_id UUID NOT NULL REFERENCES mcp_servers(server_id) ON DELETE CASCADE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    tool_name TEXT NOT NULL,
    description TEXT,
    input_schema JSONB NOT NULL,
    annotations JSONB NOT NULL DEFAULT '{}',
    discovered_at TIMESTAMPTZ NOT NULL,
    schema_hash TEXT NOT NULL,
    active BOOLEAN NOT NULL DEFAULT true,
    PRIMARY KEY (server_id, tool_name)
);

CREATE TABLE mcp_tool_annotations (
    annotation_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    server_id UUID NOT NULL,
    tool_name TEXT NOT NULL,
    risk_level TEXT NOT NULL CHECK (risk_level IN ('read_only', 'mutating', 'destructive', 'privileged', 'unknown')),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (server_id, tool_name),
    FOREIGN KEY (server_id, tool_name) REFERENCES mcp_tools(server_id, tool_name) ON DELETE CASCADE
);

CREATE TABLE mcp_health_checks (
    health_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    server_id UUID NOT NULL REFERENCES mcp_servers(server_id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    latency_ms BIGINT,
    sanitized_error TEXT,
    checked_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE mcp_sessions (
    session_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    server_id UUID NOT NULL REFERENCES mcp_servers(server_id) ON DELETE CASCADE,
    protocol_version TEXT NOT NULL,
    status TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    closed_at TIMESTAMPTZ
);

CREATE TABLE mcp_invocations (
    invocation_id UUID PRIMARY KEY,
    request_id UUID NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT NOT NULL,
    server_id UUID NOT NULL REFERENCES mcp_servers(server_id),
    tool_name TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    request_bytes BIGINT NOT NULL CHECK (request_bytes >= 0),
    response_bytes BIGINT CHECK (response_bytes >= 0),
    duration_ms BIGINT,
    risk_level TEXT NOT NULL,
    approval_required BOOLEAN NOT NULL DEFAULT false,
    status TEXT NOT NULL,
    result JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    FOREIGN KEY (server_id, tool_name) REFERENCES mcp_tools(server_id, tool_name)
);

CREATE TABLE mcp_policies (
    policy_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    rules JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);

CREATE TABLE mcp_policy_bindings (
    binding_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    policy_id UUID NOT NULL REFERENCES mcp_policies(policy_id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('global', 'tenant', 'project', 'principal', 'virtual_key')),
    scope_id TEXT NOT NULL,
    UNIQUE (tenant_id, policy_id, scope_kind, scope_id)
);

CREATE TABLE security_policies (
    policy_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    rules JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);

CREATE TABLE security_policy_bindings (
    binding_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    policy_id UUID NOT NULL REFERENCES security_policies(policy_id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('global', 'tenant', 'project', 'principal', 'virtual_key')),
    scope_id TEXT NOT NULL,
    UNIQUE (tenant_id, policy_id, scope_kind, scope_id)
);

CREATE TABLE security_findings (
    finding_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT,
    request_id UUID NOT NULL,
    inspector_id TEXT NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    confidence REAL NOT NULL CHECK (confidence >= 0 AND confidence <= 1),
    evidence JSONB NOT NULL DEFAULT '[]',
    recommended_action TEXT NOT NULL,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE security_events (
    event_id UUID PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT,
    request_id UUID NOT NULL,
    event_type TEXT NOT NULL,
    action TEXT NOT NULL,
    risk_score SMALLINT NOT NULL CHECK (risk_score BETWEEN 0 AND 100),
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE security_incidents (
    incident_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT,
    request_id UUID NOT NULL,
    category TEXT NOT NULL,
    severity TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('open', 'acknowledged', 'resolved', 'ignored')),
    risk_score SMALLINT NOT NULL CHECK (risk_score BETWEEN 0 AND 100),
    sanitized_summary TEXT NOT NULL CHECK (length(sanitized_summary) <= 2048),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);

CREATE TABLE security_custom_patterns (
    pattern_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    pattern TEXT NOT NULL CHECK (length(pattern) <= 4096),
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, name)
);

CREATE TABLE approval_requests (
    approval_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT NOT NULL,
    request_id UUID NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    action TEXT NOT NULL,
    sanitized_arguments JSONB NOT NULL,
    risk_level TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'approved', 'rejected', 'expired', 'cancelled')),
    request_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE approval_decisions (
    decision_id UUID PRIMARY KEY,
    approval_id UUID NOT NULL REFERENCES approval_requests(approval_id),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    decided_by TEXT NOT NULL,
    decision TEXT NOT NULL CHECK (decision IN ('approved', 'rejected')),
    sanitized_reason TEXT,
    decided_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (approval_id)
);

CREATE TABLE approval_executions (
    execution_id UUID PRIMARY KEY,
    approval_id UUID NOT NULL REFERENCES approval_requests(approval_id),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    principal_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('claimed', 'completed', 'failed', 'indeterminate')),
    result JSONB,
    claimed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    UNIQUE (tenant_id, approval_id, idempotency_key),
    UNIQUE (approval_id)
);

CREATE INDEX mcp_servers_tenant_idx ON mcp_servers (tenant_id, updated_at DESC);
CREATE INDEX mcp_servers_project_idx ON mcp_servers (project_id, updated_at DESC) WHERE project_id IS NOT NULL;
CREATE INDEX mcp_tools_name_idx ON mcp_tools (server_id, tool_name);
CREATE INDEX mcp_health_server_idx ON mcp_health_checks (server_id, checked_at DESC);
CREATE INDEX mcp_invocations_tenant_idx ON mcp_invocations (tenant_id, created_at DESC);
CREATE INDEX mcp_invocations_project_idx ON mcp_invocations (project_id, created_at DESC) WHERE project_id IS NOT NULL;
CREATE INDEX mcp_invocations_principal_idx ON mcp_invocations (principal_id, created_at DESC);
CREATE INDEX security_findings_tenant_idx ON security_findings (tenant_id, created_at DESC);
CREATE INDEX security_findings_request_idx ON security_findings (request_id);
CREATE INDEX security_findings_category_idx ON security_findings (category, created_at DESC);
CREATE INDEX security_incidents_tenant_idx ON security_incidents (tenant_id, created_at DESC);
CREATE INDEX security_incidents_project_idx ON security_incidents (project_id, created_at DESC) WHERE project_id IS NOT NULL;
CREATE INDEX security_incidents_status_idx ON security_incidents (status, severity, created_at DESC);
CREATE INDEX security_incidents_request_idx ON security_incidents (request_id);
CREATE INDEX security_incidents_principal_idx ON security_incidents (principal_id, created_at DESC) WHERE principal_id IS NOT NULL;
CREATE INDEX approvals_status_expiry_idx ON approval_requests (status, expires_at);
CREATE INDEX approvals_tenant_idx ON approval_requests (tenant_id, created_at DESC);
CREATE INDEX approvals_request_idx ON approval_requests (request_id);

CREATE FUNCTION audit_expired_approval() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.status = 'pending' AND NEW.status = 'expired' THEN
        INSERT INTO audit_events(event_id,idempotency_key,tenant_id,project_id,principal_id,request_id,event_type,payload,occurred_at)
        VALUES(NEW.approval_id,'approval.expired:'||NEW.approval_id,NEW.tenant_id,NEW.project_id,NEW.principal_id,NEW.request_id,'approval.expired',jsonb_build_object('approval_id',NEW.approval_id),now())
        ON CONFLICT(idempotency_key) DO NOTHING;
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER approval_expired_audit AFTER UPDATE OF status ON approval_requests
FOR EACH ROW EXECUTE FUNCTION audit_expired_approval();

CREATE FUNCTION record_incident_security_event() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE event_name TEXT; event_key TEXT; event_uuid UUID;
BEGIN
    IF TG_OP='INSERT' THEN event_name:='security.incident.created';event_key:=event_name||':'||NEW.incident_id;event_uuid:=NEW.incident_id;
    ELSE event_name:='security.incident.updated';event_key:=event_name||':'||NEW.incident_id||':'||NEW.status;event_uuid:=md5(event_key)::uuid;
    END IF;
    INSERT INTO security_events(event_id,idempotency_key,tenant_id,project_id,principal_id,request_id,event_type,action,risk_score,metadata,created_at)
    VALUES(event_uuid,event_key,NEW.tenant_id,NEW.project_id,NEW.principal_id,NEW.request_id,event_name,CASE WHEN TG_OP='INSERT' THEN 'block' ELSE 'warn' END,NEW.risk_score,jsonb_build_object('incident_status',NEW.status),now())
    ON CONFLICT(idempotency_key) DO NOTHING;
    RETURN NEW;
END $$;
CREATE TRIGGER security_incident_event AFTER INSERT OR UPDATE OF status ON security_incidents
FOR EACH ROW EXECUTE FUNCTION record_incident_security_event();
CREATE INDEX approvals_principal_idx ON approval_requests (principal_id, created_at DESC);

CREATE TRIGGER security_events_immutable
BEFORE UPDATE OR DELETE ON security_events
FOR EACH ROW EXECUTE FUNCTION reject_usage_event_mutation();
