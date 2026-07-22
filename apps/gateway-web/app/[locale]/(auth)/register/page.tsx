import { notFound } from "next/navigation"

import { AuthForm } from "@/components/auth-form"
import { isLocale } from "@/lib/locales"

export default async function RegisterPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  return <AuthForm mode="signup" locale={locale} />
}
