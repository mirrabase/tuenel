# Commercial extension contract

Tuenel Community is complete and usable without registration, a license, or a
vendor network connection. Optional commercial binaries compose with the
public router and inject `EntitlementProvider` plus provider-neutral storage
implemented by `store-postgres`; they do not add edition checks to
`gateway-core` or inference handlers.

The public contract exposes `browser_sso` and `audit_export` decisions through:

- anonymous `GET /auth/capabilities` for safe instance-level presentation;
- authenticated `GET /auth/tenants/{tenant_id}/capabilities` for tenant UI.

`CommunityEntitlements` deterministically denies both premium capabilities
without I/O. `EntitlementService::require` is the application-service gate for
every premium operation; UI visibility is presentation only. A commercial
transport maps `EntitlementError::FeatureNotEntitled` to `403` with code
`feature_not_entitled`.

The public PostgreSQL schema contains provider-neutral state for grants,
encrypted instance-license state, encrypted OIDC configuration and login
attempts, external identities, and an encrypted idempotent commerce webhook
inbox. Lemon Squeezy DTOs, license validation, checkout/webhook processing,
browser OIDC flow, and audit-export implementations belong in the private
commercial repository.

Commercial OIDC adapters may receive a `TrustedSessionIssuer` implemented by
`WebAuthService` after they validate issuer, audience, redirect URI, PKCE,
state, nonce, verified email, membership/invitation, and domain policy. No HTTP
session-minting endpoint exists.

Commercial releases must pin the public dependency to an immutable tag and
must not ship with a local path override:

```toml
gateway-entitlements = { git = "https://github.com/mirrabase/tuenel", tag = "v0.4.0" }
gateway-auth = { git = "https://github.com/mirrabase/tuenel", tag = "v0.4.0" }
gateway-server = { git = "https://github.com/mirrabase/tuenel", tag = "v0.4.0" }
store-postgres = { git = "https://github.com/mirrabase/tuenel", tag = "v0.4.0" }
```
