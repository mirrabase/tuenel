"use client"

import { Button } from "@/components/ui/button"
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty"

export default function ErrorPage({ reset }: { reset: () => void }) {
  return (
    <Empty className="min-h-[60vh]">
      <EmptyHeader>
        <EmptyTitle>Something went wrong</EmptyTitle>
        <EmptyDescription>
          The gateway console could not render this page.
        </EmptyDescription>
      </EmptyHeader>
      <EmptyContent>
        <Button onClick={reset}>Try again</Button>
      </EmptyContent>
    </Empty>
  )
}
