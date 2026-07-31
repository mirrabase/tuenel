-- Provider-neutral managed plan projection. Rows exist only for managed tenants;
-- self-hosted installations without a commercial projection remain unrestricted.
CREATE TABLE IF NOT EXISTS tenant_plan_profiles (
    tenant_id TEXT PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    tier TEXT NOT NULL CHECK (tier IN ('free','core','pro')),
    billing_interval TEXT CHECK (billing_interval IN ('monthly','annual')),
    limits JSONB NOT NULL,
    features JSONB NOT NULL,
    source TEXT NOT NULL,
    provider_variant_id BIGINT,
    subscription_status TEXT,
    valid_until TIMESTAMPTZ,
    projected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((tier='free' AND billing_interval IS NULL) OR (tier<>'free' AND billing_interval IS NOT NULL))
);

CREATE TABLE IF NOT EXISTS plan_resource_suspensions (
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    resource_kind TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    reason TEXT NOT NULL CHECK (reason='plan_limit'),
    suspended_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    restored_at TIMESTAMPTZ,
    PRIMARY KEY (tenant_id,resource_kind,resource_id)
);
