-- Organization workspace settings and provider-neutral billing.
ALTER TABLE tenants ADD COLUMN slug TEXT;
UPDATE tenants SET slug = id WHERE slug IS NULL;
ALTER TABLE tenants ALTER COLUMN slug SET NOT NULL;
ALTER TABLE tenants ADD CONSTRAINT tenants_slug_key UNIQUE (slug);
ALTER TABLE tenants ADD CONSTRAINT tenants_slug_format
    CHECK (slug ~ '^[a-z0-9]+(-[a-z0-9]+)*$' AND length(slug) BETWEEN 2 AND 63);
ALTER TABLE tenants ADD COLUMN default_environment TEXT NOT NULL DEFAULT 'production'
    CHECK (default_environment IN ('production','staging','development'));
ALTER TABLE tenants ADD COLUMN region TEXT NOT NULL DEFAULT 'global'
    CHECK (region IN ('global','us','eu','apac'));
ALTER TABLE tenants ADD COLUMN default_member_role TEXT NOT NULL DEFAULT 'engineer'
    CHECK (default_member_role IN ('engineer','viewer'));
ALTER TABLE tenants ADD COLUMN default_provider_id TEXT;
ALTER TABLE tenants ADD COLUMN version BIGINT NOT NULL DEFAULT 1 CHECK (version > 0);

CREATE TABLE organization_billing (
    tenant_id TEXT PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
    configured BOOLEAN NOT NULL DEFAULT false,
    plan_name TEXT,
    billing_cycle TEXT CHECK (billing_cycle IS NULL OR billing_cycle IN ('monthly','annual')),
    request_allowance BIGINT CHECK (request_allowance IS NULL OR request_allowance >= 0),
    token_allowance BIGINT CHECK (token_allowance IS NULL OR token_allowance >= 0),
    payment_status TEXT,
    upgrade_url TEXT,
    manage_url TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE organization_invoices (
    id UUID PRIMARY KEY,
    tenant_id TEXT NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    number TEXT NOT NULL,
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    amount NUMERIC(18,2) NOT NULL CHECK (amount >= 0),
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL,
    url TEXT,
    UNIQUE (tenant_id, number),
    CHECK (period_end >= period_start)
);
CREATE INDEX organization_invoices_tenant_issued_idx
    ON organization_invoices (tenant_id, issued_at DESC);

-- Ledger rows remain immutable during normal operation. The only permitted
-- deletion is the nested FK cascade triggered by confirmed tenant deletion.
CREATE OR REPLACE FUNCTION reject_usage_event_mutation() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' AND pg_trigger_depth() > 1 THEN
        RETURN OLD;
    END IF;
    RAISE EXCEPTION '% is append-only', TG_TABLE_NAME;
END;
$$;

-- Organization deletion is explicit and owner-confirmed. Let PostgreSQL remove
-- every tenant-owned row atomically instead of maintaining a manual delete list.
DO $$
DECLARE
    item RECORD;
BEGIN
    FOR item IN
        SELECT conrelid::regclass AS table_name, conname,
               pg_get_constraintdef(oid) AS definition
        FROM pg_constraint
        WHERE contype='f' AND confrelid='tenants'::regclass AND confdeltype<>'c'
    LOOP
        EXECUTE format(
            'ALTER TABLE %s DROP CONSTRAINT %I, ADD CONSTRAINT %I %s ON DELETE CASCADE',
            item.table_name, item.conname, item.conname, item.definition
        );
    END LOOP;
END $$;
