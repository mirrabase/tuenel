-- Additive provider/model reliability and project endpoint metadata.
ALTER TABLE usage_events
    ADD COLUMN pricing_status TEXT NOT NULL DEFAULT 'legacy_estimate'
    CHECK (pricing_status IN ('priced','unpriced','legacy_estimate'));

CREATE INDEX usage_events_pricing_status_time_idx
    ON usage_events (tenant_id, pricing_status, occurred_at DESC);

UPDATE admin_resources
SET body=jsonb_set(
    body,
    '{endpoint_id}',
    to_jsonb('p' || replace(gen_random_uuid()::text,'-','')),
    true
)
WHERE kind='projects' AND NOT body ? 'endpoint_id';

CREATE UNIQUE INDEX admin_projects_endpoint_id_idx
    ON admin_resources ((body->>'endpoint_id'))
    WHERE kind='projects' AND retired_at IS NULL;

-- Existing ties are made deterministic before the API starts maintaining the
-- invariant transactionally. The most recently edited target wins the earlier
-- position when two legacy rows requested the same priority.
WITH ranked AS (
    SELECT id,
           row_number() OVER (
               PARTITION BY tenant_id,
                            COALESCE(body->>'project_id',''),
                            body->>'requested_model'
               ORDER BY COALESCE((body->>'priority')::int, 2147483647),
                        updated_at DESC,
                        id
           ) AS position
    FROM admin_resources
    WHERE kind='model_routes' AND retired_at IS NULL
)
UPDATE admin_resources AS route
SET body = jsonb_set(route.body, '{priority}', to_jsonb(ranked.position::int), true),
    version = route.version + 1,
    updated_at = now()
FROM ranked
WHERE route.kind='model_routes'
  AND route.id=ranked.id
  AND COALESCE((route.body->>'priority')::int, 2147483647)<>ranked.position;
