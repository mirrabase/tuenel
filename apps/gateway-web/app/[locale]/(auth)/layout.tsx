import Image from "next/image"

import { Brand } from "@/components/brand"

export default function AuthLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <main className="grid min-h-screen lg:grid-cols-2">
      <section className="relative hidden lg:block">
        <Image
          src="/auth-tunnel.jpg"
          alt="Illuminated architectural tunnel"
          fill
          priority
          className="object-cover"
        />
        <div className="absolute inset-x-0 bottom-0 bg-background/80 p-8 backdrop-blur">
          <Brand className="text-2xl" size={40} />
          <p className="text-muted-foreground">
            One secure path to every model and tool.
          </p>
        </div>
      </section>
      <section className="flex items-center justify-center bg-background p-6 sm:p-10">
        {children}
      </section>
    </main>
  )
}
