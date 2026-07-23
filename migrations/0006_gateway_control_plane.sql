-- Durable, versioned resources administered by the web control plane.
CREATE TABLE admin_resources (
    kind TEXT NOT NULL CHECK (kind IN (
        'projects', 'providers', 'model_routes', 'model_prices', 'policies', 'quota_limits'
    )),
    id TEXT NOT NULL,
    tenant_id TEXT REFERENCES tenants(id),
    body JSONB NOT NULL DEFAULT '{}',
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    enabled BOOLEAN NOT NULL DEFAULT true,
    retired_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, id)
);
CREATE INDEX admin_resources_list_idx
    ON admin_resources (kind, tenant_id, updated_at DESC, id DESC)
    WHERE retired_at IS NULL;

ALTER TABLE virtual_keys ADD COLUMN display_name TEXT;

ALTER TABLE providers ADD COLUMN secret_ref TEXT REFERENCES secret_material(secret_ref);
ALTER TABLE providers ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);
ALTER TABLE providers ADD COLUMN retired_at TIMESTAMPTZ;
ALTER TABLE providers DROP COLUMN credential_ciphertext;
CREATE INDEX providers_active_idx ON providers (tenant_id, updated_at DESC)
    WHERE enabled AND retired_at IS NULL;

ALTER TABLE model_routes ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);
ALTER TABLE model_routes ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE model_routes ADD COLUMN retired_at TIMESTAMPTZ;
CREATE INDEX model_routes_active_idx ON model_routes (tenant_id, requested_model, operation)
    WHERE enabled AND retired_at IS NULL;

ALTER TABLE model_prices ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);
ALTER TABLE model_prices ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE model_prices ADD COLUMN retired_at TIMESTAMPTZ;

ALTER TABLE policies ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);
ALTER TABLE policies ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE policies ADD COLUMN retired_at TIMESTAMPTZ;

ALTER TABLE quota_limits ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);
ALTER TABLE quota_limits ADD COLUMN enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE quota_limits ADD COLUMN retired_at TIMESTAMPTZ;

ALTER TABLE billing_webhooks ALTER COLUMN secret_ciphertext DROP NOT NULL;
CREATE INDEX virtual_keys_tenant_created_idx ON virtual_keys (tenant_id, created_at DESC);
CREATE INDEX usage_events_tenant_time_desc_idx ON usage_events (tenant_id, occurred_at DESC);
CREATE INDEX billing_outbox_tenant_time_idx ON billing_outbox (tenant_id, created_at DESC);

ALTER TABLE security_custom_patterns ADD COLUMN version BIGINT NOT NULL DEFAULT 1;
ALTER TABLE security_custom_patterns ADD COLUMN updated_at TIMESTAMPTZ NOT NULL DEFAULT now();
CREATE TABLE security_incident_timeline (
    entry_id UUID PRIMARY KEY,
    incident_id UUID NOT NULL REFERENCES security_incidents(incident_id),
    tenant_id TEXT NOT NULL REFERENCES tenants(id),
    status TEXT NOT NULL,
    actor TEXT NOT NULL,
    sanitized_note TEXT CHECK (length(sanitized_note) <= 512),
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX security_incident_timeline_idx
    ON security_incident_timeline (incident_id, occurred_at, entry_id);
