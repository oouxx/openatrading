import { useState } from "react"
import { useDapanAbnormal } from "@/hooks/query/useDapanAbnormal"
import { useTimeline } from "@/hooks/query/useTimeline"

const subViews = [
  { id: "dapan", label: "今日大盘" },
  { id: "timeline", label: "本月事件" },
]

function typeEmoji(t: number): string {
  if (t === 1) return "📈"
  if (t === 2) return "📉"
  return "📊"
}

function formatTs(ts: number): string {
  if (!ts) return ""
  const d = new Date(ts * 1000)
  const h = d.getHours().toString().padStart(2, "0")
  const m = d.getMinutes().toString().padStart(2, "0")
  return `${h}:${m}`
}

export default function TimelineTab() {
  const [activeSubView, setActiveSubView] = useState("dapan")

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
      grade_emoji: item.timeline?.grade === 6 ? "🌟" : item.timeline?.grade === 5 ? "⭐" : "•",
      title: item.title || "",
      tags: (item.timeline?.theme_list || []).map((t) => t.name).join(", "),
      content: (item.content || "").replace(/\n/g, " ").substring(0, 200),
    })),
  }))

  const totalEvents = timelineDays.reduce((sum, d) => sum + d.items.length, 0)

  return (
    <div>
      <div className="mb-6 flex items-center gap-1">
        {subViews.map((v) => (
          <button
            key={v.id}
            onClick={() => setActiveSubView(v.id)}
            className={`text-caption rounded-full cursor-pointer px-4 py-2 font-mono transition-colors ${
              activeSubView === v.id ? "bg-ai/20 text-ai" : "text-ink-muted hover:text-foreground/80"
            }`}
          >
            {v.label}
          </button>
        ))}
      </div>

      {activeSubView === "dapan" && (
        <>
          {dapanLoading ? (
            <div className="flex items-center justify-center gap-3 py-16">
              <span className="bg-ai size-2 animate-pulse rounded-full" />
              <span className="text-caption font-mono text-foreground/40">加载大盘异动...</span>
            </div>
          ) : dapanError ? (
            <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg text-caption border text-center text-foreground/40">
              {dapanError.message}
            </div>
          ) : (
            <>
              <div className="text-caption mb-6 flex items-center gap-3">
                <span className="text-foreground/60">
                  共 <b className="text-foreground">{dapanList.length}</b> 条异动
                </span>
              </div>

              {dapanList.length === 0 ? (
                <div className="text-caption py-12 text-center text-foreground/40">今日暂无大盘异动</div>
              ) : (
                <div className="space-y-2">
                  {dapanList.map((item: any, idx: number) => (
                    <div
                      key={item.id || idx}
                      className="bg-surface-dark border-hairline-dark hover:bg-bg-hover rounded-lg flex items-start gap-3 border px-4 py-3 transition-colors"
                    >
                      <span className="text-caption mt-0.5 shrink-0">
                        {typeEmoji(item.bk_yi_dong_type)}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="flex flex-wrap items-center gap-2">
                          <span className="text-body text-foreground">{item.title}</span>
                          <span className="bg-ai/15 rounded-sm text-ai text-caption px-2 py-0.5 font-mono">
                            {item.bkj_info?.name || "-"}
                          </span>
                          <span className="text-caption font-mono text-foreground/40">
                            {formatTs(item.created_at)}
                          </span>
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

      {activeSubView === "timeline" && (
        <>
          {timelineLoading ? (
            <div className="flex items-center justify-center gap-3 py-16">
              <span className="bg-ai size-2 animate-pulse rounded-full" />
              <span className="text-caption font-mono text-foreground/40">加载时间线数据...</span>
            </div>
          ) : (
            <>
              <div className="text-caption mb-8 flex items-center gap-3">
                <span className="text-foreground/60">
                  <b className="text-foreground">{totalEvents}</b> 条事件 ·{" "}
                  <b className="text-foreground">{timelineDays.length}</b> 天
                </span>
              </div>

              {timelineDays.length === 0 ? (
                <div className="text-caption py-12 text-center text-foreground/40">暂无时间线数据</div>
              ) : (
                <div className="space-y-8">
                  {timelineDays.map((day) => (
                    <div key={day.date}>
                      <h3 className="text-ai text-caption mb-4 px-2 font-mono">
                        {day.date}
                      </h3>
                      <div className="space-y-3">
                        {day.items.map((item, idx) => (
                          <div
                            key={idx}
                            className="bg-surface-dark border-hairline-dark rounded-lg flex items-start gap-3 border px-4 py-3"
                          >
                            <span className="text-caption mt-0.5 shrink-0">{item.grade_emoji}</span>
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="text-body text-foreground">{item.title}</span>
                                {item.tags && (
                                  <span className="rounded-sm text-caption bg-white/10 px-2 py-0.5 font-mono text-foreground/40">
                                    {item.tags}
                                  </span>
                                )}
                              </div>
                              {item.content && (
                                <p className="text-caption mt-1 leading-relaxed text-foreground/60">
                                  {item.content}
                                </p>
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
