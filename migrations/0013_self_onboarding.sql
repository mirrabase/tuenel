-- Existing organizations keep their current console behavior. Organizations
-- created after this migration opt into the first-visit setup guide.
ALTER TABLE tenants
    ADD COLUMN onboarding_auto_open BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE tenants
    ALTER COLUMN onboarding_auto_open SET DEFAULT true;

-- Display preference is personal. Completion is intentionally derived from
-- live resources so retiring a route or revoking a key cannot leave stale
-- onboarding progress behind.
ALTER TABLE tenant_memberships
    ADD COLUMN onboarding_seen_at TIMESTAMPTZ,
    ADD COLUMN onboarding_collapsed_at TIMESTAMPTZ;
