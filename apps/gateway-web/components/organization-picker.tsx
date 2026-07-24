"use client"

import Link from "next/link"
import * as React from "react"
import { useRouter } from "next/navigation"
import { MagnifyingGlassIcon, PlusIcon } from "@phosphor-icons/react"

import type { Locale } from "@/lib/locales"
import type { Membership } from "@/lib/server-auth"
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Button } from "@/components/ui/button"
import { gatewayFetch } from "@/lib/gateway-api"

export function OrganizationPicker({
  locale,
  memberships,
}: {
  locale: Locale
  memberships: Membership[]
}) {
  const [query, setQuery] = React.useState("")
  const [pending, setPending] = React.useState(false)
  const router = useRouter()
  const filtered = memberships.filter((membership) => membership.tenant_name.toLowerCase().includes(query.toLowerCase()))
  async function createOrganization() {
    const name = window.prompt("Organization name")?.trim()
    if (!name || !memberships[0]) return
    setPending(true)
    try {
      const result = await gatewayFetch<{ membership: { tenant_id: string } }>("/auth/tenants", memberships[0].tenant_id, { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ name }) })
      router.push(`/${locale}/${result.membership.tenant_id}/projects`)
      router.refresh()
    } catch { setPending(false) }
  }
  return (
    <main className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-8 p-6 sm:p-10">
      <div className="flex flex-col gap-5 sm:flex-row sm:items-end sm:justify-between">
        <div><p className="text-sm text-muted-foreground">Tuenel Gateway</p><h1 className="font-heading text-3xl font-semibold tracking-tight">Your Organizations</h1><p className="mt-2 text-muted-foreground">Choose a workspace to manage your gateway.</p></div>
        <Button disabled={pending} onClick={createOrganization}><PlusIcon data-icon="inline-start" />New organization</Button>
      </div>
      <div className="relative max-w-md"><MagnifyingGlassIcon className="absolute top-2.5 left-3 size-4 text-muted-foreground" /><Input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search organizations" className="pl-9" /></div>
      {filtered.length === 0 ? <Card><CardContent className="py-12 text-center text-sm text-muted-foreground">No organizations match your search.</CardContent></Card> : <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {filtered.map((membership) => (
          <Link key={membership.tenant_id} href={`/${locale}/${membership.tenant_id}/projects`}>
            <Card className="h-full transition-colors hover:border-primary">
              <CardHeader><CardTitle>{membership.tenant_name}</CardTitle><CardDescription>{membership.role}</CardDescription></CardHeader>
              <CardContent className="text-sm text-muted-foreground">Plan details unavailable · Open workspace</CardContent>
            </Card>
          </Link>
        ))}
      </div>}
    </main>
  )
}
