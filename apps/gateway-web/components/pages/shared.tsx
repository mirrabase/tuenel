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
import { gatewayFetch } from "@/lib/gateway-api"

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
  description,
  action,
}: {
  title: string
  description: string
  action?: React.ReactNode
}) {
  return (
    <div className="mb-6 flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
      <div className="flex flex-col gap-1">
        <h1 className="font-heading text-2xl font-semibold tracking-tight">
          {title}
        </h1>
        <p className="max-w-3xl text-sm text-muted-foreground">{description}</p>
      </div>
      {action}
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
  children,
}: {
  loading: boolean
  error?: string
  empty?: boolean
  onRetry?: () => void
  children: React.ReactNode
}) {
  if (loading)
    return (
      <div className="flex flex-col gap-3">
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-16 w-full" />
      </div>
    )
  if (error)
    return (
      <Alert variant="destructive">
        <WarningCircleIcon />
        <AlertTitle>Request failed</AlertTitle>
        <AlertDescription>
          {error}
          {onRetry && (
            <Button className="ml-2" variant="link" size="sm" onClick={onRetry}>
              Retry
            </Button>
          )}
        </AlertDescription>
      </Alert>
    )
  if (empty)
    return (
      <Empty>
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <WarningCircleIcon />
          </EmptyMedia>
          <EmptyTitle>No records</EmptyTitle>
          <EmptyDescription>
            The gateway returned an empty result.
          </EmptyDescription>
        </EmptyHeader>
      </Empty>
    )
  return children
}

export function useGatewayData<T>(path: string) {
  const { tenantId } = useGateway()
  const [data, setData] = React.useState<T>()
  const [error, setError] = React.useState<string>()
  const [loading, setLoading] = React.useState(true)
  const load = React.useCallback(() => {
    gatewayFetch<T>(path, tenantId)
      .then(setData)
      .catch((reason) =>
        setError(reason instanceof Error ? reason.message : "Request failed")
      )
      .finally(() => setLoading(false))
  }, [path, tenantId])
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
