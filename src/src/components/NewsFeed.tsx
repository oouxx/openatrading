import { useEffect, useRef } from "react"
import { useTelegraphInfinite, type TelegraphItem } from "@/hooks/query/useTelegraph"
import { Button } from "@/components/ui/button"

interface NewsFeedProps {
  variant?: "default" | "tab"
}

interface NewsItem {
  id: number
  severity: "major" | "normal"
  time: string
  title: string
  aiNote: string
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
    aiNote: item.title || "",
  }
}

function NewsList({ items, isLoading }: { items: NewsItem[]; isLoading: boolean }) {
  if (items.length === 0 && isLoading) {
    return <div className="text-caption text-secondary py-12 text-center">加载中...</div>
  }
  if (items.length === 0) {
    return <div className="text-caption text-secondary py-12 text-center">暂无资讯</div>
  }
  return (
    <>
      {items.map((item, idx) => (
        <div
          key={`${item.id}-${idx}`}
          className={`border-hairline hover:bg-hover cursor-pointer border-t px-4 py-3 transition-colors ${idx === 0 ? "border-t-0" : ""}`}
        >
          <div className="flex items-start gap-2">
            <span className="text-caption mt-0.5 shrink-0">
              {item.severity === "major" ? "🔴" : "⚪"}
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-caption text-primary/80 leading-relaxed">
                <span className="text-secondary font-number mr-1">{item.time}</span>
                {item.title}
              </p>
              {item.aiNote && (
                <p className="text-brand/80 text-caption mt-1 leading-relaxed">→ {item.aiNote}</p>
              )}
            </div>
          </div>
        </div>
      ))}
    </>
  )
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

export default function NewsFeed({ variant = "default" }: NewsFeedProps) {
  const { data, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage, error, refetch } =
    useTelegraphInfinite()

  // 资讯只有财联社一个源（/api/telegraph 后端固定走 cls），不再做来源筛选。
  const items: NewsItem[] = data ? data.pages.flat().map((item, idx) => toNewsItem(item, idx)) : []
  const majorCount = items.filter((n) => n.severity === "major").length

  if (variant === "tab") {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="text-caption mb-6 flex shrink-0 items-center gap-3">
          <span className="text-primary/60">
            <b className="text-primary">{items.length}</b> 条资讯
            <span className="ml-2">
              · <b className="text-up">{majorCount}</b> 重要
            </span>
          </span>
        </div>

        {error && (
          <div className="shrink-0 py-12 text-center">
            <p className="text-caption text-up mb-3">资讯加载失败</p>
            <Button variant="outline" size="sm" onClick={() => refetch()}>
              重试
            </Button>
          </div>
        )}

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          {error ? null : items.length > 0 ? (
            <>
              {items.map((item) => (
                <div
                  key={item.id}
                  className="bg-surface border-hairline-dark group hover:bg-hover mb-2 flex cursor-pointer items-start gap-3 rounded-lg border px-4 py-3 transition-colors"
                >
                  <span className="text-caption mt-0.5 shrink-0">
                    {item.severity === "major" ? "🔴" : "⚪"}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="text-caption text-primary/80 group-hover:text-primary leading-relaxed transition-colors">
                      <span className="text-secondary font-number text-caption mr-2">
                        {item.time}
                      </span>
                      {item.title}
                    </p>
                    {item.aiNote && (
                      <p className="text-brand/70 text-caption text-caption mt-1">
                        → {item.aiNote}
                      </p>
                    )}
                  </div>
                </div>
              ))}
              <div className="flex-1" />
              {isFetchingNextPage && (
                <div className="text-caption text-secondary py-4 text-center">加载更多...</div>
              )}
              {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
            </>
          ) : isLoading ? (
            <div className="text-caption text-secondary py-12 text-center">加载中...</div>
          ) : (
            <div className="text-caption text-secondary py-12 text-center">暂无资讯</div>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="bg-surface border-hairline flex h-full min-h-0 flex-col overflow-hidden rounded-lg border">
      <div className="border-hairline flex items-center justify-between border-b px-4 py-3">
        <span className="text-caption text-secondary font-number tracking-wider uppercase">
          📡 实时资讯
        </span>
        <span className="text-caption text-secondary font-number">{items.length} 条</span>
      </div>
      <div className="max-h-full min-h-0 flex-1 overflow-y-auto">
        {isLoading && !items.length ? (
          <div className="text-caption text-secondary py-8 text-center">加载中...</div>
        ) : error && !items.length ? (
          <div className="py-8 text-center">
            <p className="text-caption text-up mb-3">资讯加载失败</p>
            <Button variant="outline" size="sm" onClick={() => refetch()}>
              重试
            </Button>
          </div>
        ) : (
          <>
            <NewsList items={items} isLoading={isLoading} />
            {isFetchingNextPage && (
              <div className="text-caption text-secondary py-3 text-center">加载更多...</div>
            )}
            {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
          </>
        )}
      </div>
    </div>
  )
}
