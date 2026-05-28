import { useState, useEffect } from 'react'
import { apiFetch } from '@/stores/data'

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

export default function NewsFeed({ variant = 'default' }: NewsFeedProps) {
  const [source, setSource] = useState('all')
  const [loading, setLoading] = useState(false)
  const [newsItems, setNewsItems] = useState<NewsItem[]>([])

  useEffect(() => {
    async function fetchNews() {
      setLoading(true)
      try {
        const resp = await apiFetch('/api/telegraph')
        if (resp.ok) {
          const data: unknown[] = await resp.json()
          setNewsItems(
            data.map((item: any, idx: number) => ({
              id: item.id || idx,
              severity: item.level === 'R' || item.level === 'B' ? 'major' : ('normal' as const),
              time: formatTime(item.ctime || item.modified_time),
              title: item.brief || item.title || (item.content || '').substring(0, 60),
              aiNote: item.title || '',
              source: 'cls',
            })),
          )
        }
      } catch {
        // API not available
      } finally {
        setLoading(false)
      }
    }
    fetchNews()
  }, [])

  const filteredNews = source === 'all' ? newsItems : newsItems.filter((n) => n.source === source)
  const majorCount = newsItems.filter((n) => n.severity === 'major').length

  if (variant === 'tab') {
    return (
      <div>
        <div className="flex items-center gap-3 mb-6 text-caption">
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
            className="bg-bg border border-hairline rounded-input px-2 py-0.5 text-caption text-ink-muted outline-none cursor-pointer font-mono"
          >
            <option value="all">全部来源</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>

        {loading ? (
          <div className="text-center py-12 text-caption text-ink-muted">加载中...</div>
        ) : filteredNews.length > 0 ? (
          <div className="space-y-2">
            {filteredNews.map((item) => (
              <div
                key={item.id}
                className="bg-surface-dark rounded-card border border-hairline-dark px-4 py-3 flex items-start gap-3 hover:bg-bg-hover transition-colors cursor-pointer group"
              >
                <span className="text-caption shrink-0 mt-0.5">{item.severity === 'major' ? '🔴' : '⚪'}</span>
                <div className="flex-1 min-w-0">
                  <p className="text-caption text-white/80 leading-relaxed group-hover:text-white transition-colors">
                    <span className="text-ink-muted font-mono mr-2 text-xs">{item.time}</span>
                    {item.title}
                  </p>
                  {item.aiNote && (
                    <p className="text-caption text-ai/70 mt-1 leading-relaxed text-xs">→ {item.aiNote}</p>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-center py-12 text-caption text-ink-muted">暂无资讯</div>
        )}
      </div>
    )
  }

  return (
    <div className="bg-bg-surface rounded-card border border-hairline overflow-hidden">
      <div className="flex items-center justify-between px-4 py-3 border-b border-hairline">
        <div className="flex items-center gap-2">
          <span className="text-caption-strong text-ink-muted font-mono uppercase tracking-wider">📡 实时资讯</span>
          <select
            value={source}
            onChange={(e) => setSource(e.target.value)}
            className="bg-bg border border-hairline rounded-input px-2 py-0.5 text-caption text-ink-muted outline-none cursor-pointer font-mono"
          >
            <option value="all">全部</option>
            <option value="cls">财联社</option>
            <option value="jiuyan">韭研公社</option>
          </select>
        </div>
        <span className="text-caption text-ink-muted font-mono">{filteredNews.length} 条</span>
      </div>

      <div className="max-h-[360px] overflow-y-auto">
        {loading ? (
          <div className="text-center py-8 text-caption text-ink-muted">加载中...</div>
        ) : (
          filteredNews.map((item, idx) => (
            <div
              key={item.id}
              className={`px-4 py-3 border-t border-hairline transition-colors hover:bg-bg-hover cursor-pointer ${
                idx === 0 ? 'border-t-0' : ''
              }`}
            >
              <div className="flex items-start gap-2">
                <span className="text-caption shrink-0 mt-0.5">{item.severity === 'major' ? '🔴' : '⚪'}</span>
                <div className="flex-1 min-w-0">
                  <p className="text-caption text-white/80 leading-relaxed">
                    <span className="text-ink-muted font-mono mr-1">{item.time}</span>
                    {item.title}
                  </p>
                  {item.aiNote && (
                    <p className="text-caption text-ai/80 mt-1 leading-relaxed">→ {item.aiNote}</p>
                  )}
                </div>
              </div>
            </div>
          ))
        )}
        {!loading && filteredNews.length === 0 && (
          <div className="text-center py-8 text-caption text-ink-muted">暂无资讯</div>
        )}
      </div>
    </div>
  )
}