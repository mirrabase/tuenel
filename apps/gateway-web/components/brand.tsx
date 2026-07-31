import Image from "next/image"

import { cn } from "@/lib/utils"

export function Brand({
  className,
  imageClassName,
  nameClassName,
  showName = true,
  size = 32,
}: {
  className?: string
  imageClassName?: string
  nameClassName?: string
  showName?: boolean
  size?: number
}) {
  return (
    <span className={cn("inline-flex min-w-0 items-center gap-2", className)}>
      <Image
        src="/logo.svg"
        alt="Tuenel logo"
        width={size}
        height={size}
        priority
        className={cn("shrink-0", imageClassName)}
      />
      {showName && (
        <span
          className={cn("truncate font-heading font-semibold", nameClassName)}
        >
          Tuenel
        </span>
      )}
    </span>
  )
}
