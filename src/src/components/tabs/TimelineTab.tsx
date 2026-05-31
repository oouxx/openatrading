import { useState } from 'react'
import { useDapanAbnormal } from '@/hooks/query/useDapanAbnormal'
import { useTimeline } from '@/hooks/query/useTimeline'

const subViews = [
  { id: 'dapan', label: '今日大盘' },
  { id: 'timeline', label: '本月事件' },
]

function typeEmoji(t: number): string {
  if (t === 1) return '📈'
  if (t === 2) return '📉'
  return '📊'
}

function formatTs(ts: number): string {
  if (!ts) return ''
  const d = new Date(ts * 1000)
  const h = d.getHours().toString().padStart(2, '0')
  const m = d.getMinutes().toString().padStart(2, '0')
  return `${h}:${m}`
}

export default function TimelineTab() {
  const [activeSubView, setActiveSubView] = useState('dapan')

  const { data: dapanList = [], isLoading: dapanLoading, error: dapanError } = useDapanAbnormal()
  const { data: timelineRaw = [], isLoading: timelineLoading } = useTimeline()
  const timelineList = timelineRaw as Array<{
    date: string
    list: Array<{
      title: string
      content: string
      timeline?: { grade?: number; theme_list?: Array<{ name: string }> }
    }>
  }>

  // Reverse to show newest first
  const timelineDays = [...timelineList].reverse().map((day) => ({
    date: day.date,
    items: (day.list || []).map((item) => ({
      grade_emoji: item.timeline?.grade === 6 ? '🌟' : item.timeline?.grade === 5 ? '⭐' : '•',
      title: item.title || '',
      tags: (item.timeline?.theme_list || []).map((t) => t.name).join(', '),
      content: (item.content || '').replace(/\n/g, ' ').substring(0, 200),
    })),
  }))

  const totalEvents = timelineDays.reduce((sum, d) => sum + d.items.length, 0)

  return (
    <div>
      <div className="flex items-center gap-1 mb-6">
        {subViews.map((v) => (
          <button
            key={v.id}
            onClick={() => setActiveSubView(v.id)}
            className={`px-4 py-1.5 rounded-button text-caption font-mono transition-colors cursor-pointer ${
              activeSubView === v.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'
            }`}
          >
            {v.label}
          </button>
        ))}
      </div>

      {activeSubView === 'dapan' && (
        <>
          {dapanLoading ? (
            <div className="flex items-center justify-center py-16 gap-3">
              <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />
              <span className="text-caption font-mono text-white/40">加载大盘异动...</span>
            </div>
          ) : dapanError ? (
            <div className="bg-surface-dark rounded-card border border-hairline-dark p-card-inner text-center text-caption text-white/40">
              {dapanError.message}
            </div>
          ) : (
            <>
              <div className="flex items-center gap-3 mb-6 text-caption">
                <span className="text-white/60">
                  共 <b className="text-white">{dapanList.length}</b> 条异动
                </span>
              </div>

              {dapanList.length === 0 ? (
                <div className="text-center py-12 text-caption text-white/40">今日暂无大盘异动</div>
              ) : (
                <div className="space-y-2">
                  {dapanList.map((item: any, idx: number) => (
                    <div
                      key={item.id || idx}
                      className="bg-surface-dark rounded-card border border-hairline-dark px-4 py-3 flex items-start gap-3 hover:bg-bg-hover transition-colors"
                    >
                      <span className="text-caption flex-shrink-0 mt-0.5">{typeEmoji(item.bk_yi_dong_type)}</span>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="text-body-strong text-white">{item.title}</span>
                          <span className="text-caption px-2 py-0.5 rounded-badge font-mono bg-ai/15 text-ai">
                            {item.bkj_info?.name || '-'}
                          </span>
                          <span className="text-caption font-mono text-white/40">{formatTs(item.created_at)}</span>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </>
      )}

      {activeSubView === 'timeline' && (
        <>
          {timelineLoading ? (
            <div className="flex items-center justify-center py-16 gap-3">
              <span className="w-2 h-2 rounded-full bg-ai animate-pulse" />
              <span className="text-caption font-mono text-white/40">加载时间线数据...</span>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-3 mb-8 text-caption">
                <span className="text-white/60">
                  <b className="text-white">{totalEvents}</b> 条事件 · <b className="text-white">{timelineDays.length}</b> 天
                </span>
              </div>

              {timelineDays.length === 0 ? (
                <div className="text-center py-12 text-caption text-white/40">暂无时间线数据</div>
              ) : (
                <div className="space-y-8">
                  {timelineDays.map((day) => (
                    <div key={day.date}>
                      <h3 className="text-caption-strong font-mono mb-4 px-2 text-ai">{day.date}</h3>
                      <div className="space-y-3">
                        {day.items.map((item, idx) => (
                          <div
                            key={idx}
                            className="bg-surface-dark rounded-card border border-hairline-dark pl-4 py-3 pr-4 flex gap-3 items-start"
                          >
                            <span className="text-caption flex-shrink-0 mt-0.5">{item.grade_emoji}</span>
                            <div className="flex-1 min-w-0">
                              <div className="flex items-center gap-2 flex-wrap">
                                <span className="text-body-strong text-white">{item.title}</span>
                                {item.tags && (
                                  <span className="text-caption px-2 py-0.5 rounded-badge font-mono bg-white/10 text-white/40">
                                    {item.tags}
                                  </span>
                                )}
                              </div>
                              {item.content && (
                                <p className="text-caption mt-1 leading-relaxed text-white/60">{item.content}</p>
                              )}
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </>
      )}
    </div>
  )
}
