import { notFound } from "next/navigation"

import { VerificationForm } from "@/components/verification-form"
import { isLocale } from "@/lib/locales"

export default async function VerifyPage({
  params,
  searchParams,
}: {
  params: Promise<{ locale: string }>
  searchParams: Promise<{ token?: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  const { token } = await searchParams
  return <VerificationForm locale={locale} token={token ?? ""} />
}
