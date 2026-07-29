import { notFound, redirect } from "next/navigation"

import { SetupForm } from "@/components/setup-form"
import { isLocale } from "@/lib/locales"
import { getAuthCapabilities } from "@/lib/server-auth"

export default async function SetupPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  const capabilities = await getAuthCapabilities()
  if (capabilities && !capabilities.bootstrap_required)
    redirect(`/${locale}/login`)
  return <SetupForm locale={locale} />
}
