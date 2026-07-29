import { notFound, redirect } from "next/navigation"

import { AuthForm } from "@/components/auth-form"
import { isLocale } from "@/lib/locales"
import { getAuthCapabilities } from "@/lib/server-auth"

export default async function RegisterPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  const capabilities = await getAuthCapabilities()
  if (capabilities?.bootstrap_required) redirect(`/${locale}/setup`)
  if (capabilities?.registration_mode !== "public") redirect(`/${locale}/login`)
  return <AuthForm mode="signup" locale={locale} allowSignup />
}
