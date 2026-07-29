-- Provider-neutral durable state used by optional commercial extensions.
-- PostgreSQL remains the source of truth; Redis may only cache derived decisions.
CREATE TABLE entitlement_grants (
    grant_id UUID PRIMARY KEY,
    tenant_id TEXT REFERENCES tenants(id) ON DELETE CASCADE,
    capability TEXT NOT NULL CHECK (capability IN ('browser_sso', 'audit_export')),
    enabled BOOLEAN NOT NULL,
    valid_until TIMESTAMPTZ,
    reason_code TEXT NOT NULL,
    source TEXT NOT NULL,
    provider_updated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX entitlement_grants_scope_capability_idx
    ON entitlement_grants (COALESCE(tenant_id, ''), capability);

CREATE TABLE instance_license_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton),
    installation_id UUID NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'not_configured',
    key_nonce BYTEA,
    key_ciphertext BYTEA,
    expires_at TIMESTAMPTZ,
    last_known_good_at TIMESTAMPTZ,
    last_checked_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((key_nonce IS NULL) = (key_ciphertext IS NULL))
);
INSERT INTO instance_license_state (singleton, installation_id)
SELECT true, installation_id FROM installation_state WHERE singleton = true;

CREATE TABLE external_identities (
    identity_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    subject TEXT NOT NULL,
    normalized_email TEXT NOT NULL CHECK (normalized_email = lower(normalized_email)),
    email_verified BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_login_at TIMESTAMPTZ,
    UNIQUE (tenant_id, provider, subject),
    UNIQUE (tenant_id, user_id, provider)
);

CREATE TABLE oidc_configurations (
    tenant_id TEXT PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    issuer_url TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret_nonce BYTEA NOT NULL,
    client_secret_ciphertext BYTEA NOT NULL,
    allowed_domains TEXT[] NOT NULL DEFAULT '{}',
    enabled BOOLEAN NOT NULL DEFAULT false,
    jit_enabled BOOLEAN NOT NULL DEFAULT false,
    version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE oidc_login_attempts (
    attempt_id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    state_hash BYTEA NOT NULL UNIQUE,
    nonce_hash BYTEA NOT NULL,
    pkce_verifier_nonce BYTEA NOT NULL,
    pkce_verifier_ciphertext BYTEA NOT NULL,
    redirect_uri TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX oidc_login_attempts_expiry_idx
    ON oidc_login_attempts (expires_at) WHERE consumed_at IS NULL;

CREATE TABLE commerce_webhook_inbox (
    inbox_id UUID PRIMARY KEY,
    provider TEXT NOT NULL,
    provider_event_id TEXT,
    body_hash BYTEA NOT NULL,
    payload_nonce BYTEA NOT NULL,
    payload_ciphertext BYTEA NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'processing', 'processed', 'failed', 'ignored')),
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    provider_updated_at TIMESTAMPTZ,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    last_error_code TEXT,
    UNIQUE (provider, body_hash)
);
CREATE UNIQUE INDEX commerce_webhook_provider_event_idx
    ON commerce_webhook_inbox (provider, provider_event_id)
    WHERE provider_event_id IS NOT NULL;
CREATE INDEX commerce_webhook_pending_idx
    ON commerce_webhook_inbox (received_at, inbox_id)
    WHERE status IN ('pending', 'failed');
