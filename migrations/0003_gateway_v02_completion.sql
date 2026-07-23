-- Complete the additive v0.2 storage model without changing prior migrations.
CREATE TABLE secret_material (
    secret_ref TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    purpose TEXT NOT NULL,
    nonce BYTEA NOT NULL,
    ciphertext BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at TIMESTAMPTZ
);

ALTER TABLE usage_events
    ADD COLUMN operation TEXT NOT NULL DEFAULT 'chat_completion',
    ADD COLUMN project_id TEXT,
    ADD COLUMN metadata JSONB NOT NULL DEFAULT '{}';

ALTER TABLE quota_reservations
    ADD COLUMN project_id TEXT;

CREATE TABLE audit_events (
    event_id UUID PRIMARY KEY,
    idempotency_key TEXT NOT NULL UNIQUE,
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    project_id TEXT,
    principal_id TEXT,
    request_id UUID,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}',
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE billing_webhooks
    ADD COLUMN secret_ref TEXT REFERENCES secret_material(secret_ref);

ALTER TABLE billing_outbox
    ADD COLUMN source_request_id UUID;
CREATE UNIQUE INDEX billing_outbox_request_webhook_idx
    ON billing_outbox (webhook_id, source_request_id)
    WHERE source_request_id IS NOT NULL;

CREATE INDEX audit_events_tenant_time_idx ON audit_events (tenant_id, occurred_at DESC);
CREATE INDEX audit_events_request_idx ON audit_events (request_id) WHERE request_id IS NOT NULL;
CREATE INDEX audit_events_principal_idx ON audit_events (principal_id, occurred_at DESC) WHERE principal_id IS NOT NULL;
CREATE UNIQUE INDEX model_prices_window_idx ON model_prices (provider_id, upstream_model, effective_from);
CREATE INDEX provider_health_status_idx ON provider_health (status, updated_at);
CREATE INDEX usage_events_project_day_idx ON usage_events (project_id, occurred_at) WHERE project_id IS NOT NULL;

CREATE TRIGGER audit_events_immutable
BEFORE UPDATE OR DELETE ON audit_events
FOR EACH ROW EXECUTE FUNCTION reject_usage_event_mutation();
