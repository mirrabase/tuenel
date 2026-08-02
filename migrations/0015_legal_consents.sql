ALTER TABLE pending_registrations
    ADD COLUMN terms_accepted_at TIMESTAMPTZ,
    ADD COLUMN privacy_acknowledged_at TIMESTAMPTZ;

-- Existing pending registrations predate mandatory legal consent and are
-- allowed to expire naturally; new public signups must populate both fields.
CREATE TABLE user_legal_consents (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    policy TEXT NOT NULL CHECK (policy IN ('terms_of_service', 'privacy_policy')),
    policy_url TEXT NOT NULL,
    policy_version TEXT NOT NULL,
    accepted_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, policy)
);

CREATE INDEX user_legal_consents_accepted_at_idx
    ON user_legal_consents (accepted_at);

UPDATE pending_registrations
SET terms_accepted_at = created_at,
    privacy_acknowledged_at = created_at
WHERE terms_accepted_at IS NULL
   OR privacy_acknowledged_at IS NULL;

ALTER TABLE pending_registrations
    ALTER COLUMN terms_accepted_at SET NOT NULL,
    ALTER COLUMN privacy_acknowledged_at SET NOT NULL;
