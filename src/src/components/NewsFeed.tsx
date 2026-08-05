import { useEffect, useRef, useState } from "react"
import { useTelegraphInfinite } from "@/hooks/query/useTelegraph"

interface NewsFeedProps {
  variant?: "default" | "tab"
}

interface NewsItem {
  id: number
  severity: "major" | "normal"
  time: string
  title: string
  aiNote: string
  source: string
}

function formatTime(ts: number | string): string {
  if (!ts) return ""
  const t = typeof ts === "string" ? parseInt(ts) : ts
  if (isNaN(t) || t === 0) return ""
  const d = new Date(t * 1000)
  const h = d.getHours().toString().padStart(2, "0")
  const m = d.getMinutes().toString().padStart(2, "0")
  return `${h}:${m}`
}

function toNewsItem(item: any, idx: number): NewsItem {
  return {
    id: item.id || idx,
    severity: (item.level === "R" || item.level === "B" ? "major" : "normal") as "major" | "normal",
    time: formatTime(item.ctime || item.modified_time),
    title: item.brief || item.title || (item.content || "").substring(0, 60),
    aiNote: item.title || "",
    source: "cls",
  }
}

function NewsList({ items, isLoading }: { items: NewsItem[]; isLoading: boolean }) {
  if (items.length === 0 && isLoading) {
    return <div className="text-caption text-ink-muted py-12 text-center">加载中...</div>
  }
  if (items.length === 0) {
    return <div className="text-caption text-ink-muted py-12 text-center">暂无资讯</div>
  }
  return (
    <>
      {items.map((item, idx) => (
        <div
          key={`${item.id}-${idx}`}
          className={`border-hairline hover:bg-bg-hover cursor-pointer border-t px-4 py-3 transition-colors ${idx === 0 ? "border-t-0" : ""}`}
        >
          <div className="flex items-start gap-2">
            <span className="text-caption mt-0.5 shrink-0">
              {item.severity === "major" ? "🔴" : "⚪"}
            </span>
            <div className="min-w-0 flex-1">
              <p className="text-caption leading-relaxed text-foreground/80">
                <span className="text-ink-muted mr-1 font-mono">{item.time}</span>
                {item.title}
              </p>
              {item.aiNote && (
                <p className="text-ai/80 text-caption mt-1 leading-relaxed">→ {item.aiNote}</p>
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
  const [source, setSource] = useState("all")
  const { data, isLoading, isFetchingNextPage, hasNextPage, fetchNextPage, error, refetch } =
    useTelegraphInfinite()

  const allItems: NewsItem[] = data
    ? data.pages.flat().map((item: any, idx: number) => toNewsItem(item, idx))
    : []

  const filteredNews = source === "all" ? allItems : allItems.filter((n) => n.source === source)
  const majorCount = allItems.filter((n) => n.severity === "major").length

  if (variant === "tab") {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="text-caption mb-6 flex shrink-0 items-center gap-3">
          <span className="text-foreground/60">
            <b className="text-foreground">{filteredNews.length}</b> 条资讯
            {source === "all" && (
              <span className="ml-2">
                · <b className="text-up">{majorCount}</b> 重要
              </span>
            )}
          </span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="bg-bg border-hairline rounded-lg text-caption text-ink-muted cursor-pointer border px-2 py-0.5 font-mono outline-none"
          >
            <option value="all">全部来源</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>

        {error && (
          <div className="shrink-0 py-12 text-center">
            <p className="text-caption text-up mb-3">资讯加载失败</p>
            <button
              onClick={() => refetch()}
              className="border-hairline text-ink-muted cursor-pointer rounded border px-4 py-2 text-xs transition-colors hover:text-foreground"
            >
              重试
            </button>
          </div>
        )}

        <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
          {error ? null : filteredNews.length > 0 ? (
            <>
              {filteredNews.map((item) => (
                <div
                  key={item.id}
                  className="bg-surface-dark border-hairline-dark group hover:bg-bg-hover rounded-lg mb-2 flex cursor-pointer items-start gap-3 border px-4 py-3 transition-colors"
                >
                  <span className="text-caption mt-0.5 shrink-0">
                    {item.severity === "major" ? "🔴" : "⚪"}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="text-caption leading-relaxed text-foreground/80 transition-colors group-hover:text-foreground">
                      <span className="text-ink-muted mr-2 font-mono text-xs">{item.time}</span>
                      {item.title}
                    </p>
                    {item.aiNote && (
                      <p className="text-ai/70 text-caption mt-1 text-xs/relaxed">
                        → {item.aiNote}
                      </p>
                    )}
                  </div>
                </div>
              ))}
              <div className="flex-1" />
              {isFetchingNextPage && (
                <div className="text-caption text-ink-muted py-4 text-center">加载更多...</div>
              )}
              {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
            </>
          ) : isLoading ? (
            <div className="text-caption text-ink-muted py-12 text-center">加载中...</div>
          ) : (
            <div className="text-caption text-ink-muted py-12 text-center">暂无资讯</div>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="bg-bg-surface border-hairline rounded-lg flex h-full min-h-0 flex-col overflow-hidden border">
      <div className="border-hairline flex items-center justify-between border-b px-4 py-3">
        <div className="flex items-center gap-2">
          <span className="text-caption text-ink-muted font-mono tracking-wider uppercase">
            📡 实时资讯
          </span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="bg-bg border-hairline rounded-lg text-caption text-ink-muted cursor-pointer border px-2 py-0.5 font-mono outline-none"
          >
            <option value="all">全部</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>
        <span className="text-caption text-ink-muted font-mono">{filteredNews.length} 条</span>
      </div>
      <div className="max-h-full min-h-0 flex-1 overflow-y-auto">
        {isLoading && !allItems.length ? (
          <div className="text-caption text-ink-muted py-8 text-center">加载中...</div>
        ) : error && !allItems.length ? (
          <div className="py-8 text-center">
            <p className="text-caption text-up mb-3">资讯加载失败</p>
            <button
              onClick={() => refetch()}
              className="border-hairline text-ink-muted cursor-pointer rounded border px-4 py-2 text-xs transition-colors hover:text-foreground"
            >
              重试
            </button>
          </div>
        ) : (
          <>
            <NewsList items={filteredNews} isLoading={isLoading} />
            {isFetchingNextPage && (
              <div className="text-caption text-ink-muted py-3 text-center">加载更多...</div>
            )}
            {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
          </>
        )}
      </div>
    </div>
  )
}
