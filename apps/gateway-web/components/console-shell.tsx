"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname, useRouter } from "next/navigation"
import { useTheme } from "next-themes"
import {
  ActivityIcon,
  BookOpenIcon,
  BuildingsIcon,
  ChatCircleDotsIcon,
  ChartLineUpIcon,
  CirclesThreePlusIcon,
  CoinsIcon,
  DatabaseIcon,
  GaugeIcon,
  KeyIcon,
  LockKeyIcon,
  MoonIcon,
  PlugsConnectedIcon,
  ShieldCheckIcon,
  SignOutIcon,
  SunIcon,
  TreeStructureIcon,
  UserCircleIcon,
  UsersThreeIcon,
  WrenchIcon,
} from "@phosphor-icons/react"

import { useMockGateway } from "@/components/mock-provider"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card"
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field"
import { Input } from "@/components/ui/input"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarRail,
  SidebarTrigger,
} from "@/components/ui/sidebar"
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

const groups = [
  {
    label: "Workspace",
    operator: false,
    items: [
      { href: "/", label: "Overview", icon: GaugeIcon },
      { href: "/playground", label: "Playground", icon: ChatCircleDotsIcon },
      { href: "/models", label: "Models", icon: CirclesThreePlusIcon },
      { href: "/keys", label: "Virtual Keys", icon: KeyIcon },
      { href: "/usage", label: "Usage & cost", icon: ChartLineUpIcon },
      { href: "/members", label: "Members", icon: UsersThreeIcon },
      { href: "/docs", label: "API docs", icon: BookOpenIcon },
    ],
  },
  {
    label: "MCP",
    operator: false,
    items: [{ href: "/mcp", label: "Tool explorer", icon: WrenchIcon }],
  },
  {
    label: "MCP",
    operator: true,
    items: [
      { href: "/operator/mcp", label: "Registry", icon: PlugsConnectedIcon },
      {
        href: "/operator/mcp/policies",
        label: "Policies",
        icon: ShieldCheckIcon,
      },
      { href: "/operator/approvals", label: "Approvals", icon: UserCircleIcon },
    ],
  },
  {
    label: "Security",
    operator: true,
    items: [
      { href: "/operator/security", label: "Operations", icon: ActivityIcon },
      {
        href: "/operator/security/policies",
        label: "Policies",
        icon: LockKeyIcon,
      },
    ],
  },
  {
    label: "Platform Operations",
    operator: true,
    items: [
      { href: "/operator", label: "Fleet overview", icon: GaugeIcon },
      { href: "/operator/tenants", label: "Tenants", icon: BuildingsIcon },
      {
        href: "/operator/providers",
        label: "Providers",
        icon: PlugsConnectedIcon,
      },
      { href: "/operator/routing", label: "Routing", icon: TreeStructureIcon },
      { href: "/operator/pricing", label: "Pricing", icon: CoinsIcon },
      {
        href: "/operator/policies",
        label: "General policies",
        icon: ShieldCheckIcon,
      },
      { href: "/operator/ledger", label: "Usage ledger", icon: DatabaseIcon },
      { href: "/operator/system", label: "System", icon: ActivityIcon },
      {
        href: "/operator/integrations",
        label: "Integrations",
        icon: PlugsConnectedIcon,
      },
    ],
  },
]

function Login() {
  const { dispatch } = useMockGateway()
  const [token, setToken] = React.useState("demo_admin")
  const valid = token.startsWith("demo_")
  return (
    <main className="flex min-h-screen items-center justify-center bg-muted/30 p-4">
      <Card className="w-full max-w-lg">
        <CardHeader>
          <CardTitle>Gateway mock login</CardTitle>
          <CardDescription>
            Choose a simulated OIDC identity or use a demo-only development
            token.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <Tabs defaultValue="oidc">
            <TabsList>
              <TabsTrigger value="oidc">Simulated OIDC</TabsTrigger>
              <TabsTrigger value="token">Development token</TabsTrigger>
            </TabsList>
            <TabsContent value="oidc" className="pt-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <Button
                  variant="outline"
                  onClick={() =>
                    dispatch({
                      type: "login",
                      mode: "oidc",
                      role: "tenant_user",
                    })
                  }
                >
                  <UserCircleIcon data-icon="inline-start" />
                  Avery · tenant user
                </Button>
                <Button
                  onClick={() =>
                    dispatch({
                      type: "login",
                      mode: "oidc",
                      role: "gateway_admin",
                    })
                  }
                >
                  <ShieldCheckIcon data-icon="inline-start" />
                  Alan · gateway admin
                </Button>
              </div>
            </TabsContent>
            <TabsContent value="token" className="pt-4">
              <FieldGroup>
                <Field data-invalid={!valid}>
                  <FieldLabel htmlFor="dev-token">Demo token</FieldLabel>
                  <Input
                    id="dev-token"
                    value={token}
                    aria-invalid={!valid}
                    onChange={(event) => setToken(event.target.value)}
                    autoComplete="off"
                  />
                  <FieldDescription>
                    Only values prefixed with demo_ are accepted. They are never
                    persisted.
                  </FieldDescription>
                </Field>
                <Button
                  disabled={!valid}
                  onClick={() =>
                    dispatch({
                      type: "login",
                      mode: "development-token",
                      role: token.includes("admin")
                        ? "gateway_admin"
                        : "tenant_user",
                    })
                  }
                >
                  Start simulated session
                </Button>
              </FieldGroup>
            </TabsContent>
          </Tabs>
        </CardContent>
      </Card>
    </main>
  )
}

export function ConsoleShell({
  children,
  scoped = false,
}: {
  children: React.ReactNode
  scoped?: boolean
}) {
  const pathname = usePathname()
  const segments = pathname.split("/").filter(Boolean)
  const scope = `/${segments[0]}/${segments[1]}`
  const scopedHref = (href: string) => `${scope}${href === "/" ? "" : href}`
  const router = useRouter()
  const { resolvedTheme, setTheme } = useTheme()
  const { state, dispatch } = useMockGateway()
  if (!scoped && /^\/(en|id)(\/|$)/.test(pathname)) return children
  if (!state.principal) return <Login />
  const isAdmin = state.principal.role === "gateway_admin"
  const blocked = pathname.startsWith("/operator") && !isAdmin
  const projects = Object.values(state.projects).filter(
    (project) => project.tenantId === state.principal?.tenantId
  )
  return (
    <SidebarProvider>
      <Sidebar variant="inset" collapsible="icon">
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip="Tuenel Gateway"
                render={<Link href="/" />}
              >
                <span className="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
                  <TreeStructureIcon weight="bold" />
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className="font-heading text-sm font-semibold">
                    Tuenel
                  </span>
                  <span className="text-muted-foreground">
                    Gateway v0.3 mock
                  </span>
                </span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
          {isAdmin && (
            <FieldGroup>
              <Field>
                <FieldLabel className="sr-only">Tenant</FieldLabel>
                <Select
                  items={Object.values(state.tenants).map((tenant) => ({
                    label: tenant.name,
                    value: tenant.id,
                  }))}
                  value={state.principal.tenantId}
                  onValueChange={(tenantId) => {
                    if (!tenantId) return
                    const project = Object.values(state.projects).find(
                      (item) => item.tenantId === tenantId
                    )
                    if (project)
                      dispatch({
                        type: "switch-context",
                        tenantId,
                        projectId: project.id,
                      })
                  }}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {Object.values(state.tenants).map((tenant) => (
                        <SelectItem key={tenant.id} value={tenant.id}>
                          {tenant.name}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
              <Field>
                <FieldLabel className="sr-only">Project</FieldLabel>
                <Select
                  items={projects.map((project) => ({
                    label: project.name,
                    value: project.id,
                  }))}
                  value={state.principal.projectId}
                  onValueChange={(projectId) =>
                    projectId &&
                    dispatch({
                      type: "switch-context",
                      tenantId: state.principal!.tenantId,
                      projectId,
                    })
                  }
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectGroup>
                      {projects.map((project) => (
                        <SelectItem key={project.id} value={project.id}>
                          {project.name}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
              </Field>
            </FieldGroup>
          )}
        </SidebarHeader>
        <SidebarContent>
          {groups
            .filter((group) => !group.operator || isAdmin)
            .map((group) => (
              <SidebarGroup key={`${group.label}-${group.operator}`}>
                <SidebarGroupLabel>{group.label}</SidebarGroupLabel>
                <SidebarGroupContent>
                  <SidebarMenu>
                    {group.items.map((item) => (
                      <SidebarMenuItem key={item.href}>
                        <SidebarMenuButton
                          isActive={pathname === scopedHref(item.href)}
                          tooltip={item.label}
                          render={<Link href={scopedHref(item.href)} />}
                        >
                          <item.icon />
                          <span>{item.label}</span>
                        </SidebarMenuButton>
                      </SidebarMenuItem>
                    ))}
                  </SidebarMenu>
                </SidebarGroupContent>
              </SidebarGroup>
            ))}
        </SidebarContent>
        <SidebarFooter>
          <div className="flex items-center gap-2 rounded-md p-2 group-data-[collapsible=icon]:p-0">
            <Avatar className="size-8">
              <AvatarFallback>
                {state.principal.name
                  .split(" ")
                  .map((part) => part[0])
                  .join("")}
              </AvatarFallback>
            </Avatar>
            <div className="min-w-0 flex-1 group-data-[collapsible=icon]:hidden">
              <p className="truncate text-xs font-medium">
                {state.principal.name}
              </p>
              <p className="truncate text-xs text-muted-foreground">
                {state.principal.role} · {state.principal.authMode}
              </p>
            </div>
          </div>
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
      <SidebarInset>
        <header className="sticky top-0 flex h-14 items-center gap-3 border-b bg-background/95 px-4 backdrop-blur sm:px-6">
          <SidebarTrigger />
          <Badge variant="outline">Connected</Badge>
          <div className="ml-auto flex items-center gap-1">
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Toggle color theme"
              onClick={() =>
                setTheme(resolvedTheme === "dark" ? "light" : "dark")
              }
            >
              {resolvedTheme === "dark" ? <SunIcon /> : <MoonIcon />}
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Sign out"
              onClick={async () => {
                await fetch("/api/auth/logout", { method: "POST" })
                dispatch({ type: "logout" })
                router.replace(`/${segments[0]}/login`)
              }}
            >
              <SignOutIcon />
            </Button>
          </div>
        </header>
        <div className="flex flex-1 flex-col p-4 sm:p-6 lg:p-8">
          {blocked ? (
            <Alert variant="destructive">
              <LockKeyIcon />
              <AlertTitle>Operator role required</AlertTitle>
              <AlertDescription className="flex flex-col items-start gap-3">
                Tenant users cannot open operator routes.
                <Button variant="outline" onClick={() => router.replace(scope)}>
                  Return to workspace
                </Button>
              </AlertDescription>
            </Alert>
          ) : (
            children
          )}
        </div>
      </SidebarInset>
    </SidebarProvider>
  )
}
