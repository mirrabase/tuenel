import type { Metadata } from "next"

import "./globals.css"
import { ConsoleShell } from "@/components/console-shell"
import { MockProvider } from "@/components/mock-provider"
import { ThemeProvider } from "@/components/theme-provider"
import { Toaster } from "@/components/ui/sonner"

export const metadata: Metadata = {
  title: { default: "Tuenel Gateway", template: "%s · Tuenel Gateway" },
  description: "Provider-neutral AI gateway control plane.",
}

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" suppressHydrationWarning className="antialiased">
      <body>
        <ThemeProvider>
          <MockProvider>
            <ConsoleShell>{children}</ConsoleShell>
          </MockProvider>
          <Toaster />
        </ThemeProvider>
      </body>
    </html>
  )
}
