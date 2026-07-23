# Gateway Web v0.3 — Backend Gaps

Snapshot: 22 Juli 2026. `apps/gateway-web` pada fase ini adalah **full mock**: tidak melakukan `fetch`, tidak meneruskan bearer token, dan tidak menyimpan state ke backend maupun `localStorage`. Label “Mock” dan “Simulated” pada UI bukan bukti implementasi server.

## API tersedia, sengaja belum diintegrasikan

| UI surface | Kondisi repo saat ini | Dampak fase mock | Prioritas | Kontrak integrasi minimum |
|---|---|---|---|---|
| Playground dan Models | `GET /v1/models`, `POST /v1/chat/completions`, `POST /v1/responses`, dan `POST /v1/embeddings` tersedia | Payload, streaming, usage, dan error hanya fixture deterministik | P0 | BFF/auth produksi, abortable SSE, schema request/response lengkap, korelasi request ID |
| Virtual Keys | `POST /admin/virtual-keys` dan `DELETE /admin/virtual-keys/{id}` tersedia | Issue/revoke hanya mengubah reducer; plaintext demo ditampilkan sekali | P0 | Admin client terautentikasi, response shown-once, revoke idempotent, error envelope |
| MCP Registry | CRUD server, health, refresh, inventory, dan tool listing tersedia di `/admin/mcp/*` | Create/edit/enable/delete/health/discovery tidak durable | P0 | Typed server/tool DTO, write-only secret fields, optimistic concurrency, pagination |
| MCP Explorer | `GET /v1/mcp/servers`, `GET /v1/mcp/tools`, dan `POST /v1/mcp/tools/call` tersedia | Hasil, block/redact/warn, dan idempotensi hanya simulasi | P0 | Typed invocation, idempotency key, sanitized result/error, approval reference |
| MCP Policies | CRUD `/admin/mcp/policies` tersedia | Scope dan limit editor tidak memengaruhi enforcement | P0 | Policy DTO/version, validation errors, precedence preview, concurrency-safe update |
| Approval Inbox | List/detail/approve/reject admin dan polling caller tersedia | Keputusan serta polling hanya state browser | P0 | Tenant-scoped filters, expiry, decision reason, idempotent decision, audit reference |
| Security Policies | CRUD `/admin/security/policies` tersedia | Toggle/matrix tidak memengaruhi inspection backend | P0 | Complete policy schema, validation, version, fail-open acknowledgement |
| Security Operations | List/detail/update incident serta list findings/events tersedia | Status dan sanitized note tidak durable | P0 | Tenant filters, pagination, sanitized evidence, allowed status transition, request link |
| Docs dan System | `/health`, `/ready`, `/metrics`, dan `/openapi.json` tersedia | Loading/error/health values adalah fixture | P1 | Same-origin/BFF policy, safe metrics projection, cache and retry semantics |

## Capability UI tanpa HTTP API backend memadai

Setiap baris berikut adalah gap; capability UI tetap berlabel mock sampai kontrak minimum tersedia.

| Capability | UI surface | Kondisi repo saat ini | Dampak | Prioritas | Kontrak backend minimum |
|---|---|---|---|---|---|
| Virtual-key listing | Virtual Keys | Hanya create dan revoke | Daftar, filter, serta status tidak dapat direkonsiliasi | P0 | `GET /admin/virtual-keys?tenant_id&project_id&cursor&query`, metadata non-secret, status, `next_cursor` |
| Administrasi tenant/provider/routing/pricing/model-policy/quota | Platform Operations | Belum ada HTTP CRUD lengkap | Semua mutation dan ringkasan hanya fixture | P0 | CRUD per resource, tenant/project scope, validation, version/ETag, audit ID |
| Query usage, cost, MCP invocation, reservation | Usage, Ledger, MCP Explorer | Store internal ada tetapi HTTP query operasional tidak lengkap | Tidak ada rekonsiliasi atau drill-down | P0 | Filter waktu/tenant/project/request, cursor, totals, estimated flag, sanitized invocation |
| Administrasi provider health | Providers, System | Health provider internal tersedia tanpa admin contract lengkap | Health-check dan override hanya simulasi | P1 | List/check endpoint, timeout, normalized status/reason, checked-at timestamp |
| Billing delivery/outbox management | Integrations | Delivery internal/non-blocking, tanpa UI API | Retry dan dead-letter tidak dapat dioperasikan | P1 | Outbox list/detail, idempotent retry, delivery status, sanitized failure |
| Query audit event umum | Seluruh mutation dan operator pages | Belum ada endpoint audit lintas domain | Operator tidak dapat membuktikan siapa mengubah apa | P0 | Immutable audit query, actor, scope, action, resource, request ID, cursor; tanpa secret |
| CRUD custom security pattern | Security Policies | Policy API belum menyediakan lifecycle pattern lengkap | Custom detector tidak dapat dikelola | P1 | Pattern metadata CRUD, safe validation/test endpoint, version, severity/category |
| CRUD anotasi MCP tool langsung | MCP Registry tool detail | Discovery menampilkan anotasi; mutation langsung belum tersedia | Risk annotation UI tidak durable | P1 | Patch annotation per tool/version, risk enum, reviewer, provenance, audit ID |
| Retrieval incident timeline | Security Operations detail | Incident detail belum menjamin timeline terstruktur | Investigasi kehilangan kronologi | P1 | `GET /admin/security/incidents/{id}/timeline`, ordered sanitized events, actor, timestamp |
| History/list approval untuk caller | MCP Explorer | Caller hanya dapat polling satu approval ID | Pengguna tidak dapat menemukan keputusan lama | P1 | Tenant-scoped caller list, status/time filters, cursor, sanitized summaries |
| Ringkasan operasional terstruktur | Overview, System | Angka lintas domain dihitung sebagai fixture UI | Dashboard tidak dapat menyajikan snapshot konsisten | P1 | Versioned summary DTO, timestamp/window, partial-data flags, per-metric source |
| Cursor pagination dan search | Semua tabel | List API belum konsisten mendukung cursor/search | Dataset besar tidak operasional | P0 | `limit`, opaque `cursor`, `query`, stable sort, `next_cursor`, documented maxima |
| Schema OpenAPI v0.3 lengkap | API docs | `/openapi.json` tersedia tetapi schema request/response belum lengkap untuk seluruh v0.3 | Client generation dan validasi UI tidak andal | P0 | Semua path, auth, DTO, enums, examples, error envelope, SSE dan approval variants |
| Web auth/BFF dan deployment produksi | Login dan seluruh console | OIDC serta development token hanya disimulasikan | Browser belum aman untuk memanggil admin API | P0 | OIDC callback/session, CSRF, secure cookie, RBAC, tenant binding, BFF routes, runtime config, CSP/CORS, deployment health |

## Endpoint machine-to-machine tanpa UI mentah

| Surface | Kondisi repo saat ini | Dampak/keputusan UI | Prioritas | Kontrak minimum yang tetap diperlukan |
|---|---|---|---|---|
| Native `/mcp` (`GET`, `POST`, `DELETE`) | Transport MCP tersedia | Hanya didokumentasikan; tidak dibuat console raw-protocol | P0 | Protokol MCP, session/auth, content types, capability negotiation, error semantics |
| Eksekusi provider | Adapter/provider dipanggil oleh gateway core | Tidak ada tombol untuk melewati routing/policy | P0 | Tetap internal; tracing, timeout, normalized error, credential isolation |
| Webhook dan billing workers | Delivery harus idempotent dan tidak memblokir inference | UI kelak hanya mengelola status/outbox | P1 | Signed delivery, idempotency key, retry policy, dead-letter observability |
| Enforcement quota/security internal | Berjalan sebelum/sesudah inference dan MCP sesuai pipeline | UI hanya mengedit policy dan membaca hasil | P0 | Atomic reservation/finalization, idempotent events, fail-open/closed semantics, sanitized audit |

PostgreSQL tetap sumber durable truth. Redis hanya boleh menyimpan counter, reservation, dan cache; kontrak UI kelak tidak boleh menganggap cache sebagai state administratif authoritative.
