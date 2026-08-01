"use client"

import * as React from "react"
import Link from "next/link"
import { usePathname } from "next/navigation"
import {
  CheckCircleIcon,
  CircleIcon,
  ListChecksIcon,
  LockIcon,
} from "@phosphor-icons/react"

import { useGateway } from "@/components/gateway-provider"
import { useGatewayData } from "@/components/pages/shared"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Progress,
  ProgressLabel,
  ProgressValue,
} from "@/components/ui/progress"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { gatewayFetch } from "@/lib/gateway-api"
import { cn } from "@/lib/utils"

type StepStatus = "complete" | "current" | "pending" | "blocked" | "needs_admin"

type OnboardingProgress = {
  version: number
  auto_open: boolean
  seen: boolean
  display: "expanded" | "collapsed"
  can_configure: boolean
  can_test: boolean
  project_id?: string
  completed_steps: number
  total_steps: number
  complete: boolean
  steps: { id: string; status: StepStatus }[]
}

const content: Record<
  string,
  { title: string; description: string; action: string }
> = {
  create_project: {
    title: "Create a project",
    description: "Projects isolate routes, API keys, usage, and policies.",
    action: "Create project",
  },
  connect_provider: {
    title: "Connect a provider",
    description:
      "Add credentials once. Tuenel checks health and syncs upstream models automatically.",
    action: "Add provider",
  },
  create_route: {
    title: "Create a model route",
    description:
      "Expose a stable alias and select the provider model behind it.",
    action: "Create alias",
  },
  create_api_key: {
    title: "Create an API key",
    description: "Issue a project credential for your application or device.",
    action: "Create key",
  },
  send_first_request: {
    title: "Send your first request",
    description:
      "Run a prompt in the Playground or call the OpenAI-compatible endpoint.",
    action: "Open Playground",
  },
}

function hrefFor(step: string, scope: string, projectId: string | undefined) {
  if (step === "create_project") return `${scope}/projects/new`
  if (step === "connect_provider") return `${scope}/providers`
  if (!projectId) return `${scope}/projects`
  const project = `${scope}/project/${projectId}`
  if (step === "create_route") return `${project}/models`
  if (step === "create_api_key") return `${project}/keys`
  if (step === "send_first_request") return `${project}/playground`
  return `${project}/playground`
}

export function OnboardingGuide({ className }: { className?: string }) {
  const session = useGateway()
  const pathname = usePathname()
  const locale = pathname.split("/").filter(Boolean)[0]
  const scope = `/${locale}/${session.tenantId}`
  const query = session.projectId
    ? `?project_id=${encodeURIComponent(session.projectId)}`
    : ""
  const progress = useGatewayData<OnboardingProgress>(
    `/auth/tenants/${session.tenantId}/onboarding${query}`,
    session.projectId
  )
  const [open, setOpen] = React.useState(false)
  const autoOpened = React.useRef(false)

  const updateDisplay = React.useCallback(
    async (display: "expanded" | "collapsed") => {
      try {
        await gatewayFetch(
          `/auth/tenants/${session.tenantId}/onboarding`,
          session.tenantId,
          {
            method: "PATCH",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ display }),
          }
        )
        progress.reload()
      } catch {
        // The guide remains usable for this visit if preference persistence fails.
      }
    },
    [progress, session.tenantId]
  )

  React.useEffect(() => {
    if (
      autoOpened.current ||
      !progress.data ||
      progress.data.complete ||
      (!progress.data.auto_open && progress.data.display !== "expanded")
    )
      return
    autoOpened.current = true
    setOpen(true)
    if (progress.data.auto_open) void updateDisplay("expanded")
  }, [progress.data, updateDisplay])

  React.useEffect(() => {
    if (!open || progress.data?.complete) return
    const interval = window.setInterval(progress.reload, 3000)
    return () => window.clearInterval(interval)
  }, [open, progress.data?.complete, progress.reload])

  function changeOpen(next: boolean) {
    setOpen(next)
    void updateDisplay(next ? "expanded" : "collapsed")
  }

  const data = progress.data
  const percentage = data
    ? Math.round((data.completed_steps / data.total_steps) * 100)
    : 0

  return (
    <>
      <Button
        variant="ghost"
        size="sm"
        className={cn("relative", className)}
        aria-label="Open setup guide"
        onClick={() => changeOpen(true)}
      >
        <ListChecksIcon data-icon="inline-start" />
        <span className="hidden xl:inline">Setup Guide</span>
        {data && !data.complete && (
          <span className="absolute top-0.5 right-0.5 size-1.5 rounded-full bg-primary xl:hidden" />
        )}
      </Button>
      <Sheet open={open} onOpenChange={changeOpen}>
        <SheetContent className="w-[min(92vw,26rem)] sm:max-w-md">
          <SheetHeader className="border-b">
            <div className="flex items-center gap-2">
              <div className="flex size-9 items-center justify-center rounded-lg bg-primary/10 text-primary">
                <ListChecksIcon className="size-5" weight="duotone" />
              </div>
              <div>
                <SheetTitle>Set up Tuenel</SheetTitle>
                <SheetDescription>
                  Follow these steps in order to make your first request.
                </SheetDescription>
              </div>
            </div>
            {data && (
              <Progress value={percentage} className="mt-4">
                <ProgressLabel>Setup progress</ProgressLabel>
                <ProgressValue>{() => `${percentage}%`}</ProgressValue>
              </Progress>
            )}
          </SheetHeader>
          <div className="flex-1 overflow-y-auto p-4 sm:p-6">
            {progress.loading && !data && (
              <p className="text-muted-foreground">Loading setup progress…</p>
            )}
            {progress.error && !data && (
              <div className="rounded-lg border border-destructive/30 bg-destructive/5 p-3">
                <p className="font-medium">Setup progress is unavailable</p>
                <Button
                  variant="link"
                  size="sm"
                  className="px-0"
                  onClick={progress.reload}
                >
                  Retry
                </Button>
              </div>
            )}
            <ol className="space-y-2">
              {data?.complete && (
                <li className="mb-4 rounded-lg border border-emerald-500/30 bg-emerald-500/5 p-4">
                  <div className="flex items-center gap-2 font-medium text-emerald-700 dark:text-emerald-400">
                    <CheckCircleIcon className="size-5" weight="fill" />
                    Setup complete
                  </div>
                  <p className="mt-1 text-muted-foreground">
                    Your project is ready to receive production traffic.
                  </p>
                </li>
              )}
              {data?.steps.map((step, index) => {
                const copy = content[step.id]
                const actionable = step.status === "current"
                return (
                  <li
                    key={step.id}
                    className={cn(
                      "relative rounded-lg border p-3 pl-11",
                      step.status === "current" &&
                        "border-primary/40 bg-primary/5",
                      step.status === "complete" && "bg-muted/30"
                    )}
                  >
                    <span className="absolute top-3 left-3 flex size-6 items-center justify-center">
                      {step.status === "complete" ? (
                        <CheckCircleIcon
                          className="size-5 text-emerald-500"
                          weight="fill"
                        />
                      ) : step.status === "blocked" ||
                        step.status === "needs_admin" ? (
                        <LockIcon className="size-4 text-muted-foreground" />
                      ) : (
                        <CircleIcon
                          className={cn(
                            "size-5",
                            step.status === "current"
                              ? "text-primary"
                              : "text-muted-foreground"
                          )}
                          weight={
                            step.status === "current" ? "duotone" : "regular"
                          }
                        />
                      )}
                    </span>
                    <div className="flex items-start justify-between gap-2">
                      <div>
                        <p className="font-medium">
                          {index + 1}. {copy.title}
                        </p>
                        <p className="mt-0.5 text-muted-foreground">
                          {copy.description}
                        </p>
                      </div>
                      {step.status === "current" && (
                        <Badge variant="secondary">Current</Badge>
                      )}
                    </div>
                    {step.status === "needs_admin" && (
                      <p className="mt-2 text-amber-600 dark:text-amber-400">
                        A user with the required organization permission must
                        complete this step.
                      </p>
                    )}
                    {actionable && (
                      <Button
                        size="sm"
                        className="mt-3"
                        render={
                          <Link
                            href={hrefFor(step.id, scope, data.project_id)}
                            onClick={() => setOpen(false)}
                          />
                        }
                      >
                        {copy.action}
                      </Button>
                    )}
                  </li>
                )
              })}
            </ol>
          </div>
        </SheetContent>
      </Sheet>
    </>
  )
}
