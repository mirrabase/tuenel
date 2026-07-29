CREATE TABLE pending_registrations (
    email TEXT PRIMARY KEY CHECK (email = lower(email)),
    password_hash TEXT NOT NULL,
    tenant_name TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX pending_registrations_expiry_idx ON pending_registrations (expires_at);
