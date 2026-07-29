import { notFound, redirect } from "next/navigation"

import { AuthForm } from "@/components/auth-form"
import { isLocale } from "@/lib/locales"
import { getAuthCapabilities } from "@/lib/server-auth"

export default async function LoginPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  const capabilities = await getAuthCapabilities()
  if (capabilities?.bootstrap_required) redirect(`/${locale}/setup`)
  return (
    <AuthForm
      mode="login"
      locale={locale}
      allowSignup={capabilities?.registration_mode === "public"}
      allowSso={
        capabilities?.edition === "managed" ||
        capabilities?.instance_capabilities.browser_sso.enabled === true
      }
    />
  )
}
