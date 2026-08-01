"use client"

import * as React from "react"
import { usePathname } from "next/navigation"

export type GatewaySession = {
  userId: string
  email: string
  tenantId: string
  tenantName: string
  tenantRole: "owner" | "admin" | "engineer" | "viewer"
  gatewayAdmin: boolean
  gatewayEndpoint: string
  projectDomain: string
  edition: "community" | "enterprise" | "managed"
  capabilities: {
    browserSso: boolean
    auditExport: boolean
  }
  memberships: {
    tenant_id: string
    tenant_name: string
    role: "owner" | "admin" | "engineer" | "viewer"
  }[]
  projectId?: string
}

const GatewayContext = React.createContext<GatewaySession | null>(null)

export function GatewayProvider({
  children,
  session,
}: {
  children: React.ReactNode
  session: GatewaySession
}) {
  const pathname = usePathname()
  const projectId = pathname.match(/\/project\/([0-9a-f-]{36})(?:\/|$)/i)?.[1]
  return (
    <GatewayContext.Provider value={{ ...session, projectId }}>
      {children}
    </GatewayContext.Provider>
  )
}

export function useGateway() {
  const value = React.useContext(GatewayContext)
  if (!value) throw new Error("useGateway must be used within GatewayProvider")
  return value
}
