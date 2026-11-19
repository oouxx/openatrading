import { useEffect, useRef } from "react"
import { useTelegraphInfinite, type TelegraphItem } from "@/hooks/query/useTelegraph"
import { DataEmpty, DataError, DataLoading } from "@/components/data-state"

interface NewsItem {
  id: number
  severity: "major" | "normal"
  time: string
  title: string
  detail: string
}

function formatTime(ts?: number | string): string {
  if (!ts) return ""
  const t = typeof ts === "string" ? parseInt(ts) : ts
  if (isNaN(t) || t === 0) return ""
  const d = new Date(t * 1000)
  const h = d.getHours().toString().padStart(2, "0")
  const m = d.getMinutes().toString().padStart(2, "0")
  return `${h}:${m}`
}

function toNewsItem(item: TelegraphItem, idx: number): NewsItem {
  return {
    id: item.id || idx,
    severity: (item.level === "R" || item.level === "B" ? "major" : "normal") as "major" | "normal",
    time: formatTime(item.ctime || item.modified_time),
    title: item.brief || item.title || (item.content || "").substring(0, 60),
    detail: item.title || "",
  }
}

function Sentry({ onVisible }: { onVisible: () => void }) {
  const ref = useRef<HTMLDivElement>(null)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const obs = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) onVisible()
      },
      { rootMargin: "200px" }
    )
    obs.observe(el)
    return () => obs.disconnect()
  }, [onVisible])
  return <div ref={ref} className="h-4" />
}

export default function NewsFeed() {
  const { data, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage, error, refetch } =
    useTelegraphInfinite()

  // 资讯只有财联社一个源（/api/telegraph 后端固定走 cls），不再做来源筛选。
  const items: NewsItem[] = data ? data.pages.flat().map((item, idx) => toNewsItem(item, idx)) : []
  const majorCount = items.filter((n) => n.severity === "major").length

  return (
    <div className="@container p-4 @2xl:p-6">
      <div className="text-caption text-tertiary flex items-center gap-2 pb-3">
        <span>{items.length} 条资讯</span>
        {majorCount > 0 && (
          <span className="flex items-center gap-1.5">
            <span className="bg-brand size-1 rounded-full" aria-hidden />
            {majorCount} 条重要
          </span>
        )}
      </div>

      {error && (
        <DataError
          title="资讯加载失败"
          description={error.message}
          size="compact"
          onRetry={() => void refetch()}
        />
      )}

      {error ? null : items.length > 0 ? (
        <div className="border-hairline bg-surface divide-hairline divide-y overflow-hidden rounded-lg border">
          {items.map((item) => (
            <article key={item.id} className="px-4 py-3.5">
              <div className="flex items-start gap-2.5">
                <span
                  className={`mt-1.5 size-1.5 shrink-0 rounded-full ${
                    item.severity === "major" ? "bg-brand" : "bg-tertiary/40"
                  }`}
                  aria-label={item.severity === "major" ? "重要" : undefined}
                />
                <div className="min-w-0 flex-1">
                  <p className="text-body text-secondary leading-[1.65]">
                    <span className="text-tertiary font-number text-caption mr-2">{item.time}</span>
                    {item.title}
                  </p>
                  {item.detail && item.detail !== item.title && (
                    <p className="text-tertiary text-caption mt-1 leading-relaxed">{item.detail}</p>
                  )}
                </div>
              </div>
            </article>
          ))}
          {isFetchingNextPage && <DataLoading label="加载更多资讯…" size="compact" />}
          {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
        </div>
      ) : isLoading ? (
        <DataLoading label="加载资讯…" size="table" rows={5} />
      ) : (
        <DataEmpty title="暂无资讯" size="table" />
      )}
    </div>
  )
}
