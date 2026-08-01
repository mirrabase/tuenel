-- Give existing projects a readable, stable DNS label. The four-character
-- suffix keeps duplicate project names distinct within the DNS label limit.
WITH normalized AS (
    SELECT kind,
           id,
           left(
               trim(both '-' from regexp_replace(
                   lower(COALESCE(NULLIF(body->>'name', ''), 'project')),
                   '[^a-z0-9]+', '-', 'g'
               )),
               58
           ) AS candidate,
           substring(md5(id) from 1 for 4) AS suffix
    FROM admin_resources
    WHERE kind='projects' AND retired_at IS NULL
), endpoints AS (
    SELECT kind,
           id,
           COALESCE(NULLIF(trim(both '-' from candidate), ''), 'project') || '-' || suffix AS endpoint_id
    FROM normalized
)
UPDATE admin_resources AS resource
SET body=jsonb_set(resource.body, '{endpoint_id}', to_jsonb(endpoints.endpoint_id), true),
    version=resource.version + 1,
    updated_at=now()
FROM endpoints
WHERE resource.kind=endpoints.kind AND resource.id=endpoints.id;
