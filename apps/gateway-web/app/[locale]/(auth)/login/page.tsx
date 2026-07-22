import { notFound } from "next/navigation"

import { AuthForm } from "@/components/auth-form"
import { isLocale } from "@/lib/locales"

export default async function LoginPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  return <AuthForm mode="login" locale={locale} />
}
