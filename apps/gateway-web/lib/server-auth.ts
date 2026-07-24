import { cookies } from "next/headers"

export const SESSION_COOKIE = "tuenel_session"
export const SESSION_SECONDS = 12 * 60 * 60

const gatewayUrl = () => process.env.GATEWAY_API_URL ?? "http://localhost:8080"

async function key() {
  const secret = process.env.WEB_SESSION_SECRET
  if (!secret || secret.length < 32)
    throw new Error("WEB_SESSION_SECRET must be at least 32 characters")
  return crypto.subtle.importKey(
    "raw",
    await crypto.subtle.digest("SHA-256", new TextEncoder().encode(secret)),
    "AES-GCM",
    false,
    ["encrypt", "decrypt"]
  )
}

export async function seal(value: string) {
  const iv = crypto.getRandomValues(new Uint8Array(12))
  const encrypted = new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv },
      await key(),
      new TextEncoder().encode(value)
    )
  )
  return Buffer.concat([Buffer.from(iv), Buffer.from(encrypted)]).toString(
    "base64url"
  )
}

export async function unseal(value: string) {
  try {
    const payload = Buffer.from(value, "base64url")
    const decrypted = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: payload.subarray(0, 12) },
      await key(),
      payload.subarray(12)
    )
    return new TextDecoder().decode(decrypted)
  } catch {
    return null
  }
}

export async function sessionCredential() {
  const value = (await cookies()).get(SESSION_COOKIE)?.value
  return value ? unseal(value) : null
}

export type Membership = {
  tenant_id: string
  tenant_name: string
  role: "owner" | "admin" | "engineer" | "viewer"
}

export type Session = {
  user_id: string
  email: string
  gateway_admin: boolean
  expires_at: string
  memberships: Membership[]
}

export async function getSession(): Promise<Session | null> {
  const credential = await sessionCredential()
  if (!credential) return null
  const response = await fetch(`${gatewayUrl()}/auth/session`, {
    headers: { authorization: `Bearer ${credential}` },
    cache: "no-store",
  })
  return response.ok ? response.json() : null
}

export function gatewayApiUrl(path: string) {
  return `${gatewayUrl()}${path}`
}

export async function projectBelongsToTenant(
  tenantId: string,
  projectId: string
) {
  const credential = await sessionCredential()
  if (!credential) return false
  const response = await fetch(
    `${gatewayApiUrl("/admin/projects")}?tenant_id=${encodeURIComponent(tenantId)}&project_id=${encodeURIComponent(projectId)}`,
    {
      headers: { authorization: `Bearer ${credential}.${tenantId}` },
      cache: "no-store",
    }
  )
  if (!response.ok) return false
  const body = (await response.json()) as { data?: { id?: string }[] }
  return body.data?.some((project) => project.id === projectId) ?? false
}
