"use client"

import Link from "next/link"
import * as React from "react"
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  Position,
  ReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"

type RecordValue = Record<string, unknown>

function value(input: unknown, fallback = "") {
  return input === null || input === undefined || input === ""
    ? fallback
    : String(input)
}

function InfrastructureNode({
  eyebrow,
  title,
  detail,
  status,
}: {
  eyebrow: string
  title: string
  detail?: string
  status?: string
}) {
  return (
    <div className="min-w-0 text-left">
      <div className="mb-1 flex items-center justify-between gap-2">
        <span className="text-[9px] font-semibold tracking-wider text-muted-foreground uppercase">
          {eyebrow}
        </span>
        {status && (
          <span className="rounded bg-muted px-1.5 py-0.5 text-[8px] text-muted-foreground">
            {status}
          </span>
        )}
      </div>
      <p className="truncate font-mono text-[11px] font-semibold">{title}</p>
      {detail && (
        <p className="mt-0.5 truncate text-[9px] text-muted-foreground">
          {detail}
        </p>
      )}
    </div>
  )
}

function topology(routes: RecordValue[], providers: RecordValue[]) {
  const providerNames = new Map(
    providers.map((provider) => [
      value(provider.id),
      value(provider.name, value(provider.id)),
    ])
  )
  const grouped = new Map<string, RecordValue[]>()
  for (const route of routes) {
    const alias = value(route.requested_model)
    if (!alias) continue
    const targets = grouped.get(alias) ?? []
    targets.push(route)
    grouped.set(alias, targets)
  }

  const lanes = [...grouped].map(([alias, targets]) => ({
    alias,
    targets: targets.sort(
      (left, right) => Number(left.priority ?? 0) - Number(right.priority ?? 0)
    ),
  }))
  const laneGap = 52
  const targetGap = 86
  const laneHeights = lanes.map(({ targets }) =>
    Math.max(96, targets.length * targetGap)
  )
  const graphHeight =
    laneHeights.reduce((total, height) => total + height, 0) +
    Math.max(0, lanes.length - 1) * laneGap
  const centerY = Math.max(0, graphHeight / 2 - 34)
  const nodes: Node[] = [
    {
      id: "application",
      position: { x: 0, y: centerY },
      sourcePosition: Position.Right,
      data: {
        label: (
          <InfrastructureNode
            eyebrow="Client"
            title="Application"
            detail="Inference traffic"
          />
        ),
      },
    },
    {
      id: "gateway",
      position: { x: 220, y: centerY },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: {
        label: (
          <InfrastructureNode
            eyebrow="Gateway"
            title="Tuenel Gateway"
            detail="Policy and routing"
          />
        ),
      },
    },
  ]
  const edges: Edge[] = [
    {
      id: "application-gateway",
      source: "application",
      target: "gateway",
      animated: true,
      markerEnd: { type: MarkerType.ArrowClosed },
      style: { stroke: "var(--primary)", strokeWidth: 2 },
    },
  ]

  let laneTop = 0
  lanes.forEach(({ alias, targets }, laneIndex) => {
    const laneHeight = laneHeights[laneIndex]
    const aliasId = `alias-${laneIndex}`
    const aliasY = laneTop + laneHeight / 2 - 34
    const activeIndex = targets.findIndex((route) => route.enabled !== false)
    nodes.push({
      id: aliasId,
      position: { x: 455, y: aliasY },
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: {
        label: (
          <InfrastructureNode
            eyebrow="Alias / Router"
            title={alias}
            detail={`${targets.length} ordered target${targets.length === 1 ? "" : "s"}`}
          />
        ),
      },
    })
    edges.push({
      id: `gateway-${aliasId}`,
      source: "gateway",
      target: aliasId,
      markerEnd: { type: MarkerType.ArrowClosed },
      style: {
        stroke: activeIndex >= 0 ? "var(--primary)" : "var(--muted-foreground)",
        strokeWidth: activeIndex >= 0 ? 1.8 : 1.2,
      },
    })

    targets.forEach((route, targetIndex) => {
      const providerId = value(route.provider ?? route.provider_id)
      const nodeId = `${aliasId}-target-${targetIndex}`
      const active = targetIndex === activeIndex
      const enabled = route.enabled !== false
      nodes.push({
        id: nodeId,
        position: { x: 715, y: laneTop + targetIndex * targetGap },
        targetPosition: Position.Left,
        data: {
          label: (
            <InfrastructureNode
              eyebrow={
                targetIndex === 0 ? "Primary" : `Fallback ${targetIndex}`
              }
              title={providerNames.get(providerId) ?? providerId}
              detail={value(route.upstream_model)}
              status={enabled ? (active ? "Active" : undefined) : "Disabled"}
            />
          ),
        },
        style: {
          opacity: enabled ? 1 : 0.55,
          borderColor: active ? "var(--primary)" : "var(--border)",
          boxShadow: active
            ? "0 0 0 2px color-mix(in oklab, var(--primary) 20%, transparent)"
            : undefined,
        },
      })
      edges.push({
        id: `${aliasId}-${nodeId}`,
        source: aliasId,
        target: nodeId,
        animated: active,
        markerEnd: { type: MarkerType.ArrowClosed },
        style: {
          stroke: active ? "var(--primary)" : "var(--muted-foreground)",
          strokeWidth: active ? 2 : 1.2,
          strokeDasharray: targetIndex > 0 ? "5 4" : undefined,
          opacity: enabled ? 1 : 0.45,
        },
      })
    })
    laneTop += laneHeight + laneGap
  })

  return {
    nodes: nodes.map((node) => ({
      ...node,
      style: {
        width: 174,
        borderRadius: 8,
        borderColor: "var(--border)",
        background: "var(--card)",
        color: "var(--card-foreground)",
        padding: "10px 12px",
        boxShadow: "0 1px 2px rgb(0 0 0 / 0.06)",
        ...node.style,
      },
    })),
    edges,
  }
}

export function RoutingTopology({
  routes,
  providers,
  modelsHref,
  className,
}: {
  routes: RecordValue[]
  providers: RecordValue[]
  modelsHref: string
  className?: string
}) {
  const graph = React.useMemo(
    () => topology(routes, providers),
    [providers, routes]
  )

  if (!routes.length) {
    return (
      <div
        className={cn(
          "flex h-full min-h-[380px] items-center justify-center p-6 text-center",
          className
        )}
      >
        <div className="max-w-sm">
          <p className="font-medium">No routing topology yet</p>
          <p className="mt-1 text-xs text-muted-foreground">
            Create a model alias and at least one provider target to expose an
            inference route.
          </p>
          <Button
            className="mt-4"
            size="sm"
            render={<Link href={modelsHref} />}
          >
            Configure models
          </Button>
        </div>
      </div>
    )
  }

  return (
    <div
      className={cn(
        "h-full min-h-[380px] w-full overflow-hidden bg-muted/10",
        className
      )}
    >
      <ReactFlow
        key={routes
          .map((route) =>
            [
              route.id,
              route.version,
              route.priority,
              route.enabled,
              route.upstream_model,
            ].join(":")
          )
          .join("|")}
        defaultNodes={graph.nodes}
        defaultEdges={graph.edges}
        fitView
        fitViewOptions={{ padding: 0.18, maxZoom: 1 }}
        minZoom={0.25}
        maxZoom={1.6}
        nodesConnectable={false}
        deleteKeyCode={null}
        proOptions={{ hideAttribution: true }}
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={18}
          size={1}
          color="var(--border)"
        />
        <Controls
          showInteractive={false}
          position="bottom-left"
          orientation="horizontal"
        />
      </ReactFlow>
    </div>
  )
}
