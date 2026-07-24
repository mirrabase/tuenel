"use client"

import * as React from "react"
import { WarningCircleIcon } from "@phosphor-icons/react"

import { useGateway } from "@/components/gateway-provider"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
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
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty"
import { Skeleton } from "@/components/ui/skeleton"
import { GatewayApiError, gatewayFetch } from "@/lib/gateway-api"

export type GatewayDataError = { message: string; status?: number }
export type TimeRange = "24h" | "7d" | "30d"

export function StatusBadge({ status }: { status: string }) {
  const variant = [
    "blocked",
    "critical",
    "error",
    "rejected",
    "revoked",
  ].includes(status.toLowerCase())
    ? "destructive"
    : [
          "healthy",
          "ready",
          "succeeded",
          "approved",
          "active",
          "available",
        ].includes(status.toLowerCase())
      ? "default"
      : "secondary"
  return <Badge variant={variant}>{status}</Badge>
}

export function PageHeader({
  title,
  action,
}: {
  title: React.ReactNode
  action?: React.ReactNode
}) {
  return (
    <div className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex flex-col gap-1">
        <h1 className="font-heading text-2xl font-semibold tracking-tight">
          {title}
        </h1>
      </div>
      {action}
    </div>
  )
}

export function TimeRangeSelector({
  value,
  onChange,
}: {
  value: TimeRange
  onChange: (range: TimeRange) => void
}) {
  return (
    <div className="flex rounded-md border p-1">
      {(["24h", "7d", "30d"] as const).map((range) => (
        <Button
          key={range}
          size="sm"
          variant={value === range ? "secondary" : "ghost"}
          onClick={() => onChange(range)}
        >
          {range}
        </Button>
      ))}
    </div>
  )
}

export function Metric({
  label,
  value,
  detail,
}: {
  label: string
  value: string
  detail: string
}) {
  return (
    <Card size="sm">
      <CardHeader>
        <CardDescription>{label}</CardDescription>
        <CardTitle>{value}</CardTitle>
      </CardHeader>
      <CardContent className="text-muted-foreground">{detail}</CardContent>
    </Card>
  )
}

export function DataState({
  loading,
  error,
  empty,
  onRetry,
  emptyTitle = "No records",
  emptyDescription = "The gateway returned an empty result.",
  children,
}: {
  loading: boolean
  error?: string | GatewayDataError
  empty?: boolean
  onRetry?: () => void
  emptyTitle?: string
  emptyDescription?: string
  children: React.ReactNode
}) {
  if (loading)
    return (
      <div className="flex flex-col gap-3">
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-16 w-full" />
      </div>
    )
  if (error) {
    const detail = typeof error === "string" ? error : error.message
    const denied = typeof error !== "string" && error.status === 403
    return (
      <Alert variant="destructive">
        <WarningCircleIcon />
        <AlertTitle>
          {denied ? "Permission denied" : "Request failed"}
        </AlertTitle>
        <AlertDescription>
          {denied ? "Your organization role cannot view this data." : detail}
          {onRetry && (
            <Button className="ml-2" variant="link" size="sm" onClick={onRetry}>
              Retry
            </Button>
          )}
        </AlertDescription>
      </Alert>
    )
  }
  if (empty)
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <WarningCircleIcon />
          </EmptyMedia>
          <EmptyTitle>{emptyTitle}</EmptyTitle>
          <EmptyDescription>{emptyDescription}</EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  return children
}

export function useGatewayData<T>(path: string, projectId?: string) {
  const { tenantId } = useGateway()
  const [data, setData] = React.useState<T>()
  const [error, setError] = React.useState<GatewayDataError>()
  const [loading, setLoading] = React.useState(true)
  const load = React.useCallback(() => {
    gatewayFetch<T>(path, tenantId, undefined, projectId)
      .then(setData)
      .catch((reason) =>
        setError({
          message: reason instanceof Error ? reason.message : "Request failed",
          status: reason instanceof GatewayApiError ? reason.status : undefined,
        })
      )
      .finally(() => setLoading(false))
  }, [path, projectId, tenantId])
  const reload = React.useCallback(() => {
    setLoading(true)
    setError(undefined)
    load()
  }, [load])
  React.useEffect(() => {
    load()
  }, [load])
  return { data, error, loading, reload }
}

export function useGatewayEndpoint() {
  return React.useSyncExternalStore(
    () => () => {},
    () => `${window.location.origin}/v1`,
    () => "/v1"
  )
}
