import { redirect } from "next/navigation"

import { OrganizationPicker } from "@/components/organization-picker"
import { getSession } from "@/lib/server-auth"
import { isLocale } from "@/lib/locales"

export default async function OrganizationPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) redirect("/en/login")
  const session = await getSession()
  if (!session) redirect(`/${locale}/login`)
  if (session.memberships.length === 1)
    redirect(`/${locale}/${session.memberships[0].tenant_id}/projects`)
  return (
    <OrganizationPicker locale={locale} memberships={session.memberships} />
  )
}
