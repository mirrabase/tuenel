import { notFound, redirect } from "next/navigation"

import { ConsoleShell } from "@/components/console-shell"
import { GatewayProvider } from "@/components/gateway-provider"
import { isLocale } from "@/lib/locales"
import { getSession } from "@/lib/server-auth"

export default async function TenantLayout({
  children,
  params,
}: {
  children: React.ReactNode
  params: Promise<{ locale: string; tenantId: string }>
}) {
  const { locale, tenantId } = await params
  if (!isLocale(locale) || !/^[0-9a-f]{8}-[0-9a-f-]{27}$/i.test(tenantId))
    notFound()
  const session = await getSession()
  if (!session) redirect(`/${locale}/login`)
  const membership = session.memberships.find(
    (item) => item.tenant_id === tenantId
  )
  if (!membership) notFound()
  return (
    <GatewayProvider
      session={{
        email: session.email,
        tenantId,
        tenantName: membership.tenant_name,
        tenantRole: membership.role,
        gatewayAdmin: session.gateway_admin,
      }}
    >
      <ConsoleShell scoped>{children}</ConsoleShell>
    </GatewayProvider>
  )
}
