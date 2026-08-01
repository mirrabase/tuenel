"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname, useRouter } from "next/navigation"
import { useTheme } from "next-themes"
import type { Icon } from "@phosphor-icons/react"
import {
  BellIcon,
  BuildingsIcon,
  CaretDownIcon,
  ChartLineUpIcon,
  ClipboardTextIcon,
  CloudIcon,
  CodeIcon,
  CreditCardIcon,
  CubeIcon,
  GearIcon,
  GitBranchIcon,
  HeartbeatIcon,
  HouseLineIcon,
  KeyIcon,
  LifebuoyIcon,
  MagnifyingGlassIcon,
  MoonIcon,
  PlugsConnectedIcon,
  PulseIcon,
  ShieldChevronIcon,
  SignOutIcon,
  SlidersHorizontalIcon,
  SunIcon,
  TerminalWindowIcon,
  UsersThreeIcon,
} from "@phosphor-icons/react"

import { Brand } from "@/components/brand"
import { OnboardingGuide } from "@/components/onboarding-guide"
import { useGateway } from "@/components/gateway-provider"
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandShortcut,
} from "@/components/ui/command"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import {
  Sidebar,
  SidebarContent,
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
  useSidebar,
} from "@/components/ui/sidebar"
import { useGatewayData } from "@/components/pages/shared"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import type { Page } from "@/lib/gateway-api"
import { cn } from "@/lib/utils"
import { toast } from "sonner"

type NavItem = {
  path: string
  label: string
  icon: Icon
  exact?: boolean
}

type NavGroup = { label: string; items: readonly NavItem[] }

const organizationNavigation: readonly NavGroup[] = [
  {
    label: "Organization",
    items: [
      { path: "/projects", label: "Projects", icon: BuildingsIcon },
      { path: "/team", label: "Team", icon: UsersThreeIcon },
      { path: "/providers", label: "Providers", icon: CloudIcon },
      { path: "/usage", label: "Organization Usage", icon: ChartLineUpIcon },
      { path: "/billing", label: "Billing", icon: CreditCardIcon },
      { path: "/settings", label: "Organization Settings", icon: GearIcon },
    ],
  },
]

const projectNavigation: readonly NavGroup[] = [
  {
    label: "Project",
    items: [
      { path: "", label: "Overview", icon: HouseLineIcon, exact: true },
      { path: "/playground", label: "Playground", icon: TerminalWindowIcon },
    ],
  },
  {
    label: "Gateway",
    items: [
      { path: "/providers", label: "Providers", icon: CloudIcon },
      { path: "/models", label: "Models", icon: CubeIcon },
      { path: "/routing", label: "Routing", icon: GitBranchIcon },
      { path: "/keys", label: "API Keys", icon: KeyIcon },
    ],
  },
  {
    label: "Observability",
    items: [
      { path: "/logs", label: "Requests", icon: PulseIcon },
      { path: "/usage", label: "Usage & Cost", icon: ChartLineUpIcon },
      { path: "/health", label: "Provider Health", icon: HeartbeatIcon },
    ],
  },
  {
    label: "Governance",
    items: [
      { path: "/policies", label: "Policies", icon: ShieldChevronIcon },
      { path: "/audit", label: "Audit Logs", icon: ClipboardTextIcon },
    ],
  },
  {
    label: "Developer",
    items: [
      {
        path: "/docs",
        label: "API Reference",
        icon: CodeIcon,
      },
      {
        path: "/integrations",
        label: "Integrations",
        icon: PlugsConnectedIcon,
      },
      {
        path: "/settings",
        label: "Project Settings",
        icon: SlidersHorizontalIcon,
      },
    ],
  },
]

const menuItemClass =
  "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs hover:bg-accent focus-visible:bg-accent focus-visible:outline-none disabled:pointer-events-none disabled:opacity-50"

function HeaderMenu({
  label,
  children,
  className,
}: {
  label: React.ReactNode
  children: React.ReactNode
  className?: string
}) {
  return (
    <details className={cn("group relative", className)}>
      <summary className="flex h-8 cursor-pointer list-none items-center gap-1.5 rounded-md px-2 text-xs font-medium hover:bg-muted focus-visible:ring-2 focus-visible:ring-ring/30 focus-visible:outline-none [&::-webkit-details-marker]:hidden">
        {label}
        <CaretDownIcon className="size-3 text-muted-foreground transition-transform group-open:rotate-180" />
      </summary>
      <div className="absolute top-full left-0 z-50 mt-1 min-w-56 rounded-lg bg-popover p-1 text-popover-foreground shadow-md ring-1 ring-foreground/10">
        {children}
      </div>
    </details>
  )
}

type Project = {
  id: string
  name?: string
  environment?: string
  retired_at?: string
}

function target(base: string, path: string) {
  return `${base}${path}`
}

function NavigationGroups({
  base,
  groups,
  pathname,
}: {
  base: string
  groups: readonly NavGroup[]
  pathname: string
}) {
  const { setOpenMobile } = useSidebar()
  return groups.map((group) => (
    <SidebarGroup key={group.label}>
      <SidebarGroupLabel className="h-7 text-[0.6875rem] tracking-wider uppercase">
        {group.label}
      </SidebarGroupLabel>
      <SidebarGroupContent>
        <SidebarMenu>
          {group.items.map((item) => {
            const href = target(base, item.path)
            const active =
              pathname === href ||
              (!item.exact && pathname.startsWith(`${href}/`))
            return (
              <SidebarMenuItem key={item.label}>
                <SidebarMenuButton
                  isActive={active}
                  tooltip={item.label}
                  render={
                    <Link href={href} onClick={() => setOpenMobile(false)} />
                  }
                >
                  <item.icon weight={active ? "fill" : "regular"} />
                  <span>{item.label}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            )
          })}
        </SidebarMenu>
      </SidebarGroupContent>
    </SidebarGroup>
  ))
}

function ManagedPlanBadge({ tenantId }: { tenantId: string }) {
  const plan = useGatewayData<{
    tier: "free" | "core" | "pro"
    usage: { routed_tokens_this_month: number }
    limits: { routed_tokens_per_month: number | null }
  }>(`/commercial/tenants/${tenantId}/billing/status`)
  if (!plan.data) return null
  const usage = plan.data.usage.routed_tokens_this_month
  const limit = plan.data.limits.routed_tokens_per_month
  const title =
    limit === null
      ? `${usage.toLocaleString()} routed tokens this month · Unlimited plan`
      : `${usage.toLocaleString()} / ${limit.toLocaleString()} routed tokens this month`
  return (
    <Badge variant="secondary" className="capitalize" title={title}>
      {plan.data.tier}
    </Badge>
  )
}

export function ConsoleShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname()
  const router = useRouter()
  const { resolvedTheme, setTheme } = useTheme()
  const session = useGateway()
  const [searchOpen, setSearchOpen] = React.useState(false)
  const [helpOpen, setHelpOpen] = React.useState(false)
  const segments = pathname.split("/").filter(Boolean)
  const locale = segments[0]
  const scope = `/${locale}/${session.tenantId}`
  const projectScope = session.projectId
    ? `${scope}/project/${session.projectId}`
    : ""
  const inProject = Boolean(session.projectId)
  const groups = inProject
    ? projectNavigation
    : organizationNavigation.map((group) => ({
        ...group,
        items: group.items.filter(
          (item) => item.path !== "/billing" || session.edition === "managed"
        ),
      }))
  const base = inProject ? projectScope : scope
  const projects = useGatewayData<Page<Project>>(
    `/admin/projects?tenant_id=${encodeURIComponent(session.tenantId)}`
  )
  const project = projects.data?.data.find(
    (item) => item.id === session.projectId
  )
  const projectName =
    project?.name ?? `Project ${session.projectId?.slice(0, 8)}`
  const environment = project?.environment ?? "Environment"
  const playground = pathname === `${projectScope}/playground`

  React.useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === "k" && (event.metaKey || event.ctrlKey)) {
        event.preventDefault()
        setSearchOpen((open) => !open)
      }
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [])

  const navigate = (href: string) => {
    setSearchOpen(false)
    router.push(href)
  }

  return (
    <SidebarProvider
      className={playground ? "h-svh overflow-hidden" : undefined}
    >
      <Sidebar collapsible="icon">
        <SidebarHeader>
          <SidebarMenu>
            <SidebarMenuItem>
              <SidebarMenuButton
                size="lg"
                tooltip="Tuenel"
                render={<Link href={`${scope}/projects`} />}
              >
                <Brand
                  size={32}
                  className="text-sm group-data-[collapsible=icon]:gap-0"
                  imageClassName="rounded-md"
                  nameClassName="group-data-[collapsible=icon]:hidden"
                />
              </SidebarMenuButton>
            </SidebarMenuItem>
          </SidebarMenu>
        </SidebarHeader>
        <SidebarContent>
          <NavigationGroups base={base} groups={groups} pathname={pathname} />
        </SidebarContent>
        <SidebarRail />
      </Sidebar>
      <SidebarInset
        className={playground ? "min-h-0 overflow-hidden" : undefined}
      >
        <header className="sticky top-0 z-30 flex h-12 shrink-0 items-center gap-2 border-b bg-background/95 px-3 backdrop-blur supports-[backdrop-filter]:bg-background/80 sm:px-4">
          <SidebarTrigger aria-label="Toggle project navigation" />
          <nav
            aria-label="Workspace hierarchy"
            className="flex min-w-0 items-center gap-1"
          >
            <HeaderMenu
              className="max-w-36"
              label={<span className="truncate">{session.tenantName}</span>}
            >
              <p className="px-2 py-1.5 text-xs text-muted-foreground">
                Organization
              </p>
              {session.memberships.map((membership) => (
                <button
                  type="button"
                  key={membership.tenant_id}
                  className={menuItemClass}
                  onClick={() =>
                    router.push(`/${locale}/${membership.tenant_id}/projects`)
                  }
                >
                  <BuildingsIcon />
                  {membership.tenant_name}
                </button>
              ))}
            </HeaderMenu>
            {session.edition === "managed" && (
              <ManagedPlanBadge tenantId={session.tenantId} />
            )}
            {inProject && (
              <>
                <span className="text-xs text-muted-foreground">/</span>
                <HeaderMenu
                  className="max-w-40"
                  label={<span className="truncate">{projectName}</span>}
                >
                  <p className="px-2 py-1.5 text-xs text-muted-foreground">
                    Project
                  </p>
                  {projects.data?.data
                    .filter((item) => !item.retired_at)
                    .map((item) => (
                      <button
                        type="button"
                        key={item.id}
                        className={menuItemClass}
                        onClick={() =>
                          router.push(`${scope}/project/${item.id}`)
                        }
                      >
                        <CubeIcon />
                        {item.name ?? `Project ${item.id.slice(0, 8)}`}
                      </button>
                    ))}
                </HeaderMenu>
                <span className="hidden text-xs text-muted-foreground sm:inline">
                  /
                </span>
                <HeaderMenu
                  className="hidden max-w-32 sm:block"
                  label={<span className="truncate">{environment}</span>}
                >
                  <p className="px-2 py-1.5 text-xs text-muted-foreground">
                    Environment
                  </p>
                  <button
                    type="button"
                    className={menuItemClass}
                    onClick={() => router.push(`${projectScope}/settings`)}
                  >
                    <GitBranchIcon />
                    <span className="flex flex-col">
                      <span>{environment}</span>
                      <span className="text-muted-foreground">
                        Configure environment
                      </span>
                    </span>
                  </button>
                </HeaderMenu>
              </>
            )}
          </nav>
          <div className="ml-auto flex shrink-0 items-center gap-1">
            <Button
              variant="outline"
              size="sm"
              className="hidden w-48 justify-start text-muted-foreground lg:flex"
              onClick={() => setSearchOpen(true)}
            >
              <MagnifyingGlassIcon data-icon="inline-start" />
              Search
              <kbd className="ml-auto text-[0.625rem]">⌘K</kbd>
            </Button>
            <OnboardingGuide />
            <Button
              variant="ghost"
              size="icon-sm"
              className="lg:hidden"
              aria-label="Search"
              onClick={() => setSearchOpen(true)}
            >
              <MagnifyingGlassIcon />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Help & Feedback"
              onClick={() => setHelpOpen(true)}
            >
              <LifebuoyIcon />
            </Button>
            <HeaderMenu
              className="[&>div]:right-0 [&>div]:left-auto"
              label={
                <>
                  <BellIcon />
                  <span className="sr-only">Notifications</span>
                </>
              }
            >
              <p className="px-2 py-1.5 text-xs text-muted-foreground">
                Notifications
              </p>
              <p className="px-2 py-3 text-xs text-muted-foreground">
                No new notifications
              </p>
            </HeaderMenu>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Toggle color theme"
              onClick={() =>
                setTheme(resolvedTheme === "dark" ? "light" : "dark")
              }
            >
              <SunIcon className="hidden dark:block" />
              <MoonIcon className="dark:hidden" />
            </Button>
            <HeaderMenu
              className="[&>div]:right-0 [&>div]:left-auto"
              label={
                <div className="flex items-center gap-2">
                  <div className="flex size-6 shrink-0 items-center justify-center overflow-hidden rounded-full bg-primary/10 text-[11px] font-semibold text-primary">
                    {(session as Record<string, unknown>).avatarUrl ||
                    (session as Record<string, unknown>).avatar ||
                    (session as Record<string, unknown>).image ? (
                      // Arbitrary identity-provider avatar URLs cannot be
                      // safely allowlisted for the Next.js image proxy.
                      // eslint-disable-next-line @next/next/no-img-element
                      <img
                        src={String(
                          (session as Record<string, unknown>).avatarUrl ||
                            (session as Record<string, unknown>).avatar ||
                            (session as Record<string, unknown>).image
                        )}
                        alt="Profile photo"
                        className="size-full object-cover"
                      />
                    ) : (
                      (session.email?.[0] || "U").toUpperCase()
                    )}
                  </div>
                  <span className="hidden max-w-[120px] truncate text-xs font-medium sm:inline-block">
                    {session.email?.split("@")[0] || "Account"}
                  </span>
                </div>
              }
            >
              <div className="px-2 py-1.5 text-xs">
                <span className="block truncate font-medium">
                  {session.email}
                </span>
                <span className="text-muted-foreground">
                  {session.gatewayAdmin ? "gateway_admin" : session.tenantRole}
                </span>
              </div>
              <div className="my-1 h-px bg-border/50" />
              <button
                type="button"
                className={menuItemClass}
                onClick={async () => {
                  await fetch("/api/auth/logout", { method: "POST" })
                  router.replace(`/${locale}/login`)
                  router.refresh()
                }}
              >
                <SignOutIcon />
                Sign out
              </button>
            </HeaderMenu>
          </div>
        </header>
        <div
          className={cn(
            "flex max-w-full min-w-0 flex-1 flex-col",
            playground
              ? "min-h-0 overflow-hidden p-4 sm:p-5"
              : "p-4 sm:p-6 lg:p-8"
          )}
        >
          {children}
        </div>
      </SidebarInset>
      {searchOpen && (
        <div
          role="dialog"
          aria-modal="true"
          aria-label="Search Tuenel"
          className="fixed inset-0 z-[100] flex items-start justify-center p-4 pt-[15vh]"
          onKeyDown={(event) => {
            if (event.key === "Escape") setSearchOpen(false)
          }}
        >
          <button
            type="button"
            aria-label="Close search"
            className="absolute inset-0 bg-black/70"
            onClick={() => setSearchOpen(false)}
          />
          <Command className="relative z-10 h-auto max-h-[70vh] w-full max-w-xl shadow-2xl ring-1 ring-foreground/10">
            <CommandInput placeholder="Search pages and projects…" autoFocus />
            <CommandList>
              <CommandEmpty>No matching pages or projects.</CommandEmpty>
              {groups.map((group) => (
                <CommandGroup key={group.label} heading={group.label}>
                  {group.items.map((item) => (
                    <CommandItem
                      key={item.label}
                      value={item.label}
                      onSelect={() => navigate(target(base, item.path))}
                    >
                      <item.icon />
                      {item.label}
                    </CommandItem>
                  ))}
                </CommandGroup>
              ))}
              {inProject && (
                <CommandGroup heading="Projects">
                  {projects.data?.data
                    .filter((item) => !item.retired_at)
                    .map((item) => (
                      <CommandItem
                        key={item.id}
                        value={`${item.name ?? "Project"} ${item.id}`}
                        onSelect={() => navigate(`${scope}/project/${item.id}`)}
                      >
                        <CubeIcon />
                        {item.name ?? `Project ${item.id.slice(0, 8)}`}
                      </CommandItem>
                    ))}
                </CommandGroup>
              )}
            </CommandList>
            <div className="border-t px-3 py-2 text-xs text-muted-foreground">
              <CommandShortcut>Enter to open · Esc to close</CommandShortcut>
            </div>
          </Command>
        </div>
      )}
      {helpOpen && <HelpModal open={helpOpen} onOpenChange={setHelpOpen} />}
    </SidebarProvider>
  )
}

function HelpModal({
  open,
  onOpenChange,
}: {
  open: boolean
  onOpenChange: (open: boolean) => void
}) {
  const [topic, setTopic] = React.useState("feedback")
  const [message, setMessage] = React.useState("")
  const [sending, setSending] = React.useState(false)

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault()
    if (!message.trim()) return
    setSending(true)
    setTimeout(() => {
      setSending(false)
      toast.success("Feedback sent! Thank you for helping us improve Tuenel.")
      setMessage("")
      onOpenChange(false)
    }, 500)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <LifebuoyIcon className="size-5 text-primary" />
            Help & Feedback
          </DialogTitle>
          <DialogDescription>
            Found a bug, have a feature request, or need help? Send feedback
            directly to the team.
          </DialogDescription>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4 py-1">
          <div className="space-y-1.5">
            <label className="text-xs font-medium text-foreground">Topic</label>
            <select
              value={topic}
              onChange={(e) => setTopic(e.target.value)}
              className="w-full rounded-md border bg-background px-3 py-2 text-xs text-foreground focus:ring-2 focus:ring-ring/30 focus:outline-none"
            >
              <option value="feedback">General Feedback</option>
              <option value="bug">Report an Issue / Bug</option>
              <option value="feature">Feature Request</option>
              <option value="question">Question / Help</option>
            </select>
          </div>

          <div className="space-y-1.5">
            <label className="text-xs font-medium text-foreground">
              Message
            </label>
            <textarea
              required
              rows={4}
              value={message}
              onChange={(e) => setMessage(e.target.value)}
              placeholder="Tell us what's on your mind or describe the issue..."
              className="w-full rounded-md border bg-background p-3 text-xs text-foreground focus:ring-2 focus:ring-ring/30 focus:outline-none"
            />
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => onOpenChange(false)}
            >
              Cancel
            </Button>
            <Button
              type="submit"
              size="sm"
              disabled={sending || !message.trim()}
            >
              {sending ? "Sending..." : "Submit Feedback"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
