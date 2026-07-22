"use client"

import * as React from "react"

import {
  createSeedState,
  mockReducer,
  type MockAction,
  type MockState,
} from "@/lib/mock-store"

const MockContext = React.createContext<{
  state: MockState
  dispatch: React.Dispatch<MockAction>
} | null>(null)

type AuthenticatedSession = {
  email: string
  tenantId: string
  tenantName: string
  role: "owner" | "admin" | "engineer" | "viewer"
}

export function MockProvider({
  children,
  session,
}: {
  children: React.ReactNode
  session?: AuthenticatedSession
}) {
  const [state, dispatch] = React.useReducer(mockReducer, undefined, () => {
    const state = createSeedState()
    if (!session) return state
    const projectId = `${session.tenantId}:default`
    state.tenants = {
      [session.tenantId]: { id: session.tenantId, name: session.tenantName },
    }
    state.projects = {
      [projectId]: {
        id: projectId,
        tenantId: session.tenantId,
        name: "Default",
      },
    }
    state.principal = {
      id: session.email,
      name: session.email.split("@")[0],
      email: session.email,
      role: session.role === "viewer" ? "tenant_user" : "gateway_admin",
      tenantRole: session.role,
      tenantId: session.tenantId,
      projectId,
      authMode: "oidc",
    }
    return state
  })
  return (
    <MockContext.Provider value={{ state, dispatch }}>
      {children}
    </MockContext.Provider>
  )
}

export function useMockGateway() {
  const value = React.useContext(MockContext)
  if (!value) throw new Error("useMockGateway must be used within MockProvider")
  return value
}
