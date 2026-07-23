-- v0.2 additive schema. Existing v0.1 tables and immutable usage rows remain intact.
CREATE TABLE providers (
    id TEXT PRIMARY KEY,
    tenant_id TEXT REFERENCES tenants(id),
    provider_type TEXT NOT NULL,
    base_url TEXT NOT NULL,
    credential_ciphertext BYTEA,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE model_routes (
    id UUID PRIMARY KEY,
    tenant_id TEXT REFERENCES tenants(id),
    requested_model TEXT NOT NULL,
    operation TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE model_route_targets (
    id UUID PRIMARY KEY,
    route_id UUID NOT NULL REFERENCES model_routes(id) ON DELETE CASCADE,
    provider_id TEXT NOT NULL REFERENCES providers(id),
    upstream_model TEXT NOT NULL,
    priority INTEGER NOT NULL CHECK (priority > 0),
    enabled BOOLEAN NOT NULL DEFAULT true,
    UNIQUE (route_id, priority)
);

CREATE TABLE provider_health (
    provider_id TEXT PRIMARY KEY REFERENCES providers(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    consecutive_failures INTEGER NOT NULL DEFAULT 0,
    latest_success_at TIMESTAMPTZ,
    latest_failure_at TIMESTAMPTZ,
    rolling_latency_ms BIGINT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE policies (
    id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('global', 'tenant', 'project', 'principal', 'virtual_key')),
    scope_id TEXT NOT NULL,
    allowed_models TEXT[] NOT NULL DEFAULT '{}',
    denied_models TEXT[] NOT NULL DEFAULT '{}',
    allowed_operations TEXT[] NOT NULL DEFAULT '{}',
    max_output_tokens BIGINT,
    max_embedding_inputs BIGINT,
    max_body_bytes BIGINT,
    requests_per_minute BIGINT,
    concurrent_requests BIGINT,
    daily_token_limit BIGINT,
    monthly_token_limit BIGINT,
    daily_cost_limit NUMERIC(18,8),
    monthly_cost_limit NUMERIC(18,8),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (tenant_id, scope_kind, scope_id)
);

CREATE TABLE quota_limits (
    id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    scope_kind TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    period TEXT NOT NULL,
    token_limit BIGINT,
    cost_limit NUMERIC(18,8),
    concurrent_limit BIGINT,
    requests_per_minute BIGINT,
    UNIQUE (tenant_id, scope_kind, scope_id, period)
);

CREATE TABLE quota_reservation_scopes (
    reservation_id UUID NOT NULL REFERENCES quota_reservations(reservation_id) ON DELETE CASCADE,
    scope_kind TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    reserved_tokens BIGINT NOT NULL DEFAULT 0,
    reserved_cost NUMERIC(18,8) NOT NULL DEFAULT 0,
    PRIMARY KEY (reservation_id, scope_kind, scope_id)
);

CREATE TABLE usage_daily (
    bucket_date DATE NOT NULL,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    user_id TEXT,
    provider_id TEXT NOT NULL,
    requested_model TEXT NOT NULL,
    upstream_model TEXT NOT NULL,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    provider_cost NUMERIC(18,8) NOT NULL DEFAULT 0,
    billable_cost NUMERIC(18,8) NOT NULL DEFAULT 0,
    event_count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (bucket_date, tenant_id, project_id, user_id, provider_id, requested_model, upstream_model)
);

CREATE TABLE usage_hourly (LIKE usage_daily INCLUDING ALL);
CREATE TABLE usage_monthly (LIKE usage_daily INCLUDING ALL);

CREATE TABLE model_prices (
    price_id UUID PRIMARY KEY,
    provider_id TEXT NOT NULL REFERENCES providers(id),
    upstream_model TEXT NOT NULL,
    input_cost_per_million NUMERIC(18,8) NOT NULL CHECK (input_cost_per_million >= 0),
    output_cost_per_million NUMERIC(18,8) NOT NULL CHECK (output_cost_per_million >= 0),
    cached_input_cost_per_million NUMERIC(18,8),
    embedding_cost_per_million NUMERIC(18,8),
    effective_from TIMESTAMPTZ NOT NULL,
    effective_until TIMESTAMPTZ,
    CHECK (effective_until IS NULL OR effective_until > effective_from)
);

CREATE TABLE billing_webhooks (
    webhook_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    url TEXT NOT NULL,
    secret_ciphertext BYTEA NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    maximum_attempts INTEGER NOT NULL DEFAULT 10,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE billing_outbox (
    event_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    webhook_id UUID NOT NULL REFERENCES billing_webhooks(webhook_id),
    payload JSONB NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE billing_delivery_attempts (
    attempt_id UUID PRIMARY KEY,
    event_id UUID NOT NULL REFERENCES billing_outbox(event_id) ON DELETE CASCADE,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    status_code INTEGER,
    error TEXT
);

CREATE INDEX usage_daily_tenant_idx ON usage_daily (tenant_id, bucket_date);
CREATE INDEX usage_daily_project_idx ON usage_daily (project_id, bucket_date);
CREATE INDEX billing_outbox_pending_idx ON billing_outbox (next_attempt_at) WHERE delivered_at IS NULL;
CREATE INDEX model_prices_lookup_idx ON model_prices (provider_id, upstream_model, effective_from);
