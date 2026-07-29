import { notFound } from "next/navigation"

import { VerificationForm } from "@/components/verification-form"
import { isLocale } from "@/lib/locales"

export default async function VerifyPage({
  params,
}: {
  params: Promise<{ locale: string }>
}) {
  const { locale } = await params
  if (!isLocale(locale)) notFound()
  return <VerificationForm locale={locale} />
}
