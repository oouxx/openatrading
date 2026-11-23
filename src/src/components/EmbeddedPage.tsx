import { Suspense, type ReactNode } from "react"
import ErrorBoundary from "@/components/ErrorBoundary"
import { Skeleton } from "@/components/ui/skeleton"

/**
 * 把整页视图（自选 / 模拟盘）塞进网格格子时的外壳：它们自带页头，
 * 这里只补懒加载骨架与错误边界，避免一格里抛错带走整个工作区。
 */
export default function EmbeddedPage({ name, children }: { name: string; children: ReactNode }) {
  return (
    <div className="bg-canvas flex h-full min-w-0 flex-col overflow-hidden">
      <ErrorBoundary name={name}>
        <Suspense
          fallback={
            <div className="p-4">
              <Skeleton className="h-24 w-full" />
            </div>
          }
        >
          {children}
        </Suspense>
      </ErrorBoundary>
    </div>
  )
}
