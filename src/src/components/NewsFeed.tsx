import { useEffect, useRef, useState } from 'react'
import { useTelegraphInfinite } from '@/hooks/query/useTelegraph'

interface NewsFeedProps {
  variant?: 'default' | 'tab'
}

interface NewsItem {
  id: number
  severity: 'major' | 'normal'
  time: string
  title: string
  aiNote: string
  source: string
}

function formatTime(ts: number | string): string {
  if (!ts) return ''
  const t = typeof ts === 'string' ? parseInt(ts) : ts
  if (isNaN(t) || t === 0) return ''
  const d = new Date(t * 1000)
  const h = d.getHours().toString().padStart(2, '0')
  const m = d.getMinutes().toString().padStart(2, '0')
  return `${h}:${m}`
}

function toNewsItem(item: any, idx: number): NewsItem {
  return {
    id: item.id || idx,
    severity: (item.level === 'R' || item.level === 'B' ? 'major' : 'normal') as 'major' | 'normal',
    time: formatTime(item.ctime || item.modified_time),
    title: item.brief || item.title || (item.content || '').substring(0, 60),
    aiNote: item.title || '',
    source: 'cls',
  }
}

function NewsList({
  items,
  isLoading,
}: {
  items: NewsItem[]
  isLoading: boolean
}) {
  if (items.length === 0 && isLoading) {
    return <div className="text-caption py-12 text-center text-ink-muted">加载中...</div>
  }
  if (items.length === 0) {
    return <div className="text-caption py-12 text-center text-ink-muted">暂无资讯</div>
  }
  return (
    <>
      {items.map((item, idx) => (
        <div
          key={`${item.id}-${idx}`}
          className={`cursor-pointer border-t border-hairline px-4 py-3 transition-colors hover:bg-bg-hover ${idx === 0 ? 'border-t-0' : ''}`}
        >
          <div className="flex items-start gap-2">
            <span className="text-caption mt-0.5 shrink-0">{item.severity === 'major' ? '🔴' : '⚪'}</span>
            <div className="min-w-0 flex-1">
              <p className="text-caption leading-relaxed text-white/80">
                <span className="mr-1 font-mono text-ink-muted">{item.time}</span>
                {item.title}
              </p>
              {item.aiNote && (
                <p className="text-caption mt-1 leading-relaxed text-ai/80">→ {item.aiNote}</p>
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
      ([entry]) => { if (entry.isIntersecting) onVisible() },
      { rootMargin: '200px' },
    )
    obs.observe(el)
    return () => obs.disconnect()
  }, [onVisible])
  return <div ref={ref} className="h-4" />
}

export default function NewsFeed({ variant = 'default' }: NewsFeedProps) {
  const [source, setSource] = useState('all')
  const {
    data,
    isLoading,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
    error,
    refetch,
  } = useTelegraphInfinite()

  const allItems: NewsItem[] = data
    ? data.pages.flat().map((item: any, idx: number) => toNewsItem(item, idx))
    : []

  const filteredNews =
    source === 'all' ? allItems : allItems.filter((n) => n.source === source)
  const majorCount = allItems.filter((n) => n.severity === 'major').length

  if (variant === 'tab') {
    return (
      <div className="flex h-full min-h-0 flex-col">
        <div className="text-caption mb-6 flex flex-shrink-0 items-center gap-3">
          <span className="text-white/60">
            <b className="text-white">{filteredNews.length}</b> 条资讯
            {source === 'all' && (
              <span className="ml-2">
                · <b className="text-up">{majorCount}</b> 重要
              </span>
            )}
          </span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="text-caption cursor-pointer rounded-input border border-hairline bg-bg px-2 py-0.5 font-mono text-ink-muted outline-none"
          >
            <option value="all">全部来源</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>

        {error && (
          <div className="flex-shrink-0 py-12 text-center">
            <p className="text-caption mb-3 text-up">资讯加载失败</p>
            <button
              onClick={() => refetch()}
              className="cursor-pointer rounded border border-hairline px-4 py-1.5 text-xs text-ink-muted transition-colors hover:text-white"
            >
              重试
            </button>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto">
          {!error && filteredNews.map((item) => (
            <div
              key={item.id}
              className="group mb-2 flex cursor-pointer items-start gap-3 rounded-card border border-hairline-dark bg-surface-dark px-4 py-3 transition-colors hover:bg-bg-hover"
            >
              <span className="text-caption mt-0.5 shrink-0">{item.severity === 'major' ? '🔴' : '⚪'}</span>
              <div className="min-w-0 flex-1">
                <p className="text-caption leading-relaxed text-white/80 transition-colors group-hover:text-white">
                  <span className="mr-2 font-mono text-xs text-ink-muted">{item.time}</span>
                  {item.title}
                </p>
                {item.aiNote && (
                  <p className="text-caption mt-1 text-xs leading-relaxed text-ai/70">→ {item.aiNote}</p>
                )}
              </div>
            </div>
          ))}

          {isFetchingNextPage && (
            <div className="text-caption py-4 text-center text-ink-muted">加载更多...</div>
          )}
          {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}

          {!isLoading && !error && filteredNews.length === 0 && (
            <div className="text-caption py-12 text-center text-ink-muted">暂无资讯</div>
          )}
        </div>
      </div>
    )
  }

  return (
    <div className="overflow-hidden rounded-card border border-hairline bg-bg-surface">
      <div className="flex items-center justify-between border-b border-hairline px-4 py-3">
        <div className="flex items-center gap-2">
          <span className="text-caption-strong font-mono tracking-wider text-ink-muted uppercase">📡 实时资讯</span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="text-caption cursor-pointer rounded-input border border-hairline bg-bg px-2 py-0.5 font-mono text-ink-muted outline-none"
          >
            <option value="all">全部</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>
        <span className="text-caption font-mono text-ink-muted">{filteredNews.length} 条</span>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto" style={{ maxHeight: 'calc(100vh - 300px)' }}>
        {isLoading && !allItems.length ? (
          <div className="text-caption py-8 text-center text-ink-muted">加载中...</div>
        ) : error && !allItems.length ? (
          <div className="py-8 text-center">
            <p className="text-caption mb-3 text-up">资讯加载失败</p>
            <button
              onClick={() => refetch()}
              className="cursor-pointer rounded border border-hairline px-4 py-1.5 text-xs text-ink-muted transition-colors hover:text-white"
            >
              重试
            </button>
          </div>
        ) : (
          <>
            <NewsList items={filteredNews} isLoading={isLoading} />
            {isFetchingNextPage && (
              <div className="text-caption py-3 text-center text-ink-muted">加载更多...</div>
            )}
            {hasNextPage && <Sentry onVisible={() => fetchNextPage()} />}
          </>
        )}
      </div>
    </div>
  )
}
