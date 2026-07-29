CREATE TABLE pending_registrations (
    email TEXT PRIMARY KEY CHECK (email = lower(email)),
    password_hash TEXT NOT NULL,
    tenant_name TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX pending_registrations_expiry_idx ON pending_registrations (expires_at);

CREATE TABLE installation_state (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton),
    installation_id UUID NOT NULL DEFAULT gen_random_uuid() UNIQUE,
    initialized_at TIMESTAMPTZ,
    initialized_by UUID REFERENCES users(id)
);

INSERT INTO installation_state (singleton, initialized_at, initialized_by)
SELECT true,
       CASE WHEN EXISTS (SELECT 1 FROM users) THEN now() ELSE NULL END,
       (SELECT id FROM users ORDER BY created_at, id LIMIT 1);

CREATE TABLE auth_email_outbox (
    event_id UUID PRIMARY KEY,
    delivery_kind TEXT NOT NULL CHECK (delivery_kind IN ('verification', 'invitation')),
    recipient TEXT NOT NULL CHECK (recipient = lower(recipient)),
    nonce BYTEA NOT NULL,
    ciphertext BYTEA NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0 CHECK (attempt_count >= 0),
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at TIMESTAMPTZ,
    last_error_code TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX auth_email_delivery_idx
    ON auth_email_outbox (next_attempt_at, created_at)
    WHERE delivered_at IS NULL;
