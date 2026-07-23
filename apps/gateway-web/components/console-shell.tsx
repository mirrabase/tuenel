"use client"

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

import { useGateway } from "@/components/gateway-provider"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
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
      { href: "/mcp", label: "MCP explorer", icon: WrenchIcon },
    ],
  },
  {
    label: "Operations",
    operator: true,
    items: [
      { href: "/operator", label: "Overview", icon: GaugeIcon },
      { href: "/operator/tenants", label: "Tenants", icon: BuildingsIcon },
      {
        href: "/operator/providers",
        label: "Providers",
        icon: PlugsConnectedIcon,
      },
      { href: "/operator/routing", label: "Routing", icon: TreeStructureIcon },
      { href: "/operator/pricing", label: "Pricing", icon: CoinsIcon },
      { href: "/operator/policies", label: "Policies", icon: ShieldCheckIcon },
      { href: "/operator/ledger", label: "Ledger", icon: DatabaseIcon },
      { href: "/operator/system", label: "System", icon: ActivityIcon },
      {
        href: "/operator/integrations",
        label: "Integrations",
        icon: PlugsConnectedIcon,
      },
      {
        href: "/operator/mcp",
        label: "MCP registry",
        icon: PlugsConnectedIcon,
      },
      {
        href: "/operator/mcp/policies",
        label: "MCP policies",
        icon: ShieldCheckIcon,
      },
      { href: "/operator/approvals", label: "Approvals", icon: UserCircleIcon },
      { href: "/operator/security", label: "Security", icon: ActivityIcon },
      {
        href: "/operator/security/policies",
        label: "Security policies",
        icon: LockKeyIcon,
      },
    ],
  },
] as const

export function ConsoleShell({
  children,
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
  const session = useGateway()
  const operator = session.gatewayAdmin || session.tenantRole !== "viewer"

  return (
    <SidebarProvider>
      <Sidebar variant="inset" collapsible="icon">
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip="Tuenel Gateway"
                render={<Link href={scope} />}
              >
                <span className="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground">
                  <TreeStructureIcon weight="bold" />
                </span>
                <span className="flex min-w-0 flex-col">
                  <span className="font-heading text-sm font-semibold">
                    Tuenel
                  </span>
                  <span className="truncate text-muted-foreground">
                    {session.tenantName}
                  </span>
                </span>
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
          {groups
            .filter((group) => !group.operator || operator)
            .map((group) => (
              <SidebarGroup key={group.label}>
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
                {session.email.slice(0, 2).toUpperCase()}
              </AvatarFallback>
            </Avatar>
            <div className="min-w-0 flex-1 group-data-[collapsible=icon]:hidden">
              <p className="truncate text-xs font-medium">{session.email}</p>
              <p className="truncate text-xs text-muted-foreground">
                {session.gatewayAdmin ? "gateway_admin" : session.tenantRole}
              </p>
            </div>
          </div>
        </SidebarFooter>
        <SidebarRail />
      </Sidebar>
      <SidebarInset>
        <header className="sticky top-0 flex h-14 items-center gap-3 border-b bg-background/95 px-4 backdrop-blur sm:px-6">
          <SidebarTrigger />
          <Badge variant="outline">Live</Badge>
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
                router.replace(`/${segments[0]}/login`)
                router.refresh()
              }}
            >
              <SignOutIcon />
            </Button>
          </div>
        </header>
        <div className="flex flex-1 flex-col p-4 sm:p-6 lg:p-8">{children}</div>
      </SidebarInset>
    </SidebarProvider>
  )
}
