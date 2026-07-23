"use client"

import * as React from "react"

export type GatewaySession = {
  email: string
  tenantId: string
  tenantName: string
  tenantRole: "owner" | "admin" | "engineer" | "viewer"
  gatewayAdmin: boolean
}

const GatewayContext = React.createContext<GatewaySession | null>(null)

export function GatewayProvider({
  children,
  session,
}: {
  children: React.ReactNode
  session: GatewaySession
}) {
  return (
    <GatewayContext.Provider value={session}>
      {children}
    </GatewayContext.Provider>
  )
}

export function useGateway() {
  const value = React.useContext(GatewayContext)
  if (!value) throw new Error("useGateway must be used within GatewayProvider")
  return value
}
