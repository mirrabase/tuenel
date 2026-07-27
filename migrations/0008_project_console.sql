-- Additive project-console telemetry and API-key constraints.
ALTER TABLE virtual_keys
    ADD COLUMN allowed_models TEXT[] NOT NULL DEFAULT '{}',
    ADD COLUMN daily_request_limit BIGINT CHECK (daily_request_limit IS NULL OR daily_request_limit > 0),
    ADD COLUMN monthly_budget NUMERIC(18,8) CHECK (monthly_budget IS NULL OR monthly_budget > 0);

ALTER TABLE usage_events
    ADD COLUMN latency_ms BIGINT CHECK (latency_ms IS NULL OR latency_ms >= 0);

CREATE INDEX usage_events_project_status_time_idx
    ON usage_events (project_id, status, occurred_at DESC)
    WHERE project_id IS NOT NULL;
