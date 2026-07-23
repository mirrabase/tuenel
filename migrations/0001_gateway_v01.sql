CREATE TABLE tenants (
    id TEXT PRIMARY KEY,
    daily_token_limit BIGINT NOT NULL CHECK (daily_token_limit > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE virtual_keys (
    id UUID PRIMARY KEY,
    lookup_prefix TEXT NOT NULL UNIQUE,
    secret_hash TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    user_id TEXT,
    scopes TEXT[] NOT NULL DEFAULT '{}',
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    daily_token_limit BIGINT NOT NULL CHECK (daily_token_limit > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ
);

CREATE TABLE usage_events (
    event_id UUID PRIMARY KEY,
    request_id UUID NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    principal_id TEXT NOT NULL,
    user_id TEXT,
    provider TEXT NOT NULL,
    requested_model TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    prompt_tokens BIGINT NOT NULL CHECK (prompt_tokens >= 0),
    completion_tokens BIGINT NOT NULL CHECK (completion_tokens >= 0),
    total_tokens BIGINT NOT NULL CHECK (total_tokens = prompt_tokens + completion_tokens),
    estimated_cost NUMERIC(18, 8) NOT NULL CHECK (estimated_cost >= 0),
    usage_estimated BOOLEAN NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('succeeded', 'provider_failed', 'interrupted')),
    occurred_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX usage_events_tenant_day_idx ON usage_events (tenant_id, occurred_at);
CREATE INDEX usage_events_principal_day_idx ON usage_events (principal_id, occurred_at);

CREATE TABLE quota_reservations (
    reservation_id UUID PRIMARY KEY,
    request_id UUID NOT NULL UNIQUE,
    owner_kind TEXT NOT NULL CHECK (owner_kind IN ('tenant', 'virtual_key')),
    owner_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    principal_id TEXT NOT NULL,
    user_id TEXT,
    provider TEXT NOT NULL,
    requested_model TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    prompt_tokens BIGINT NOT NULL CHECK (prompt_tokens >= 0),
    completion_tokens BIGINT NOT NULL CHECK (completion_tokens >= 0),
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX quota_reservations_owner_idx ON quota_reservations (owner_kind, owner_id);
CREATE INDEX quota_reservations_expiry_idx ON quota_reservations (expires_at);

CREATE FUNCTION reject_usage_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'usage_events is append-only';
END;
$$;

CREATE TRIGGER usage_events_immutable
BEFORE UPDATE OR DELETE ON usage_events
FOR EACH ROW EXECUTE FUNCTION reject_usage_event_mutation();
