"use client"

import * as React from "react"

import { WarningCircleIcon } from "@phosphor-icons/react"

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
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
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"

export function StatusBadge({ status }: { status: string }) {
  const variant = [
    "blocked",
    "critical",
    "error",
    "rejected",
    "Revoked",
  ].includes(status)
    ? "destructive"
    : ["healthy", "succeeded", "approved", "Active", "Available"].includes(
          status
        )
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
        <div className="flex items-center gap-2">
          <h1 className="font-heading text-2xl font-semibold tracking-tight">
            {title}
          </h1>
          <Badge variant="outline">Mock</Badge>
        </div>
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

export function BackendNotice({
  children = "This control is simulated in browser memory; no backend request is made.",
}: {
  children?: React.ReactNode
}) {
  return (
    <Alert>
      <WarningCircleIcon />
      <AlertTitle>Full-mock preview</AlertTitle>
      <AlertDescription>{children}</AlertDescription>
    </Alert>
  )
}

export function StateVariants({ children }: { children: React.ReactNode }) {
  return (
    <Tabs defaultValue="data">
      <TabsList>
        <TabsTrigger value="data">Data</TabsTrigger>
        <TabsTrigger value="loading">Loading</TabsTrigger>
        <TabsTrigger value="empty">Empty</TabsTrigger>
        <TabsTrigger value="error">Error</TabsTrigger>
      </TabsList>
      <TabsContent value="data" className="pt-3">
        {children}
      </TabsContent>
      <TabsContent value="loading" className="flex flex-col gap-3 pt-3">
        <Skeleton className="h-16 w-full" />
        <Skeleton className="h-16 w-full" />
      </TabsContent>
      <TabsContent value="empty" className="pt-3">
        <Empty>
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <WarningCircleIcon />
            </EmptyMedia>
            <EmptyTitle>No mock records</EmptyTitle>
            <EmptyDescription>
              The deterministic empty-state fixture is active.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      </TabsContent>
      <TabsContent value="error" className="pt-3">
        <Alert variant="destructive">
          <WarningCircleIcon />
          <AlertTitle>Simulated backend unavailable</AlertTitle>
          <AlertDescription>
            Retry is disabled because this phase performs no network calls.
          </AlertDescription>
        </Alert>
      </TabsContent>
    </Tabs>
  )
}
