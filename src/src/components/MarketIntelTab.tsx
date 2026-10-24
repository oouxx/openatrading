import { useState } from "react"
import { Button } from "@/components/ui/button"
import { SafeHtml } from "@/components/SafeHtml"
import { cn } from "@/lib/utils"
import { StatusBanner } from "@/components/status-banner"
import { useSentiment } from "@/hooks/query/useSentiment"

const subSections = [
  { id: "topics", label: "题材热度与舆情" },
  { id: "posts", label: "论坛热帖聚合" },
]

export default function MarketIntelTab() {
  const [activeSection, setActiveSection] = useState("topics")
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [expandedTgbId, setExpandedTgbId] = useState<number | null>(null)

  const { data: sentimentData } = useSentiment()
  const hasData = !!(
    sentimentData &&
    (sentimentData.hot_stocks?.length ||
      sentimentData.hot_articles?.length ||
      sentimentData.taoguba_articles?.length)
  )

  const hotStocks = sentimentData?.hot_stocks ?? []
  const hotArticles = sentimentData?.hot_articles ?? []
  const taogubaArticles = sentimentData?.taoguba_articles ?? []

  const trendText = sentimentData?.trend || "暂无数据"
  const trendColor =
    trendText === "偏多" ? "text-up" : trendText === "中性" ? "text-brand" : "text-down"

  return (
    <div>
      <div className="mb-6 flex items-center gap-1">
        {subSections.map((s) => (
          <Button
            key={s.id}
            variant="ghost"
            size="sm"
            onClick={() => setActiveSection(s.id)}
            className={cn(
              "text-caption font-number rounded-full px-4 transition-colors",
              activeSection === s.id
                ? "bg-brand/20 text-brand hover:bg-brand/20 hover:text-brand"
                : "text-secondary hover:text-secondary"
            )}
          >
            {s.label}
          </Button>
        ))}
      </div>

      {activeSection === "topics" && (
        <div>
          <div className="text-caption mb-8 flex items-center gap-3">
            <span className="text-secondary">
              热度趋势: <b className={trendColor}>{trendText}</b>
            </span>
          </div>

          {!hasData ? (
            <StatusBanner tone="info">
              暂无语情数据：需要先在 ~/.atrading/.env 配置 JIUYAN_PHONE 才抓取韭研公社
            </StatusBanner>
          ) : (
            <>
              {/* 热门股票 */}
              <div className="bg-surface border-hairline p-card-inner mb-6 rounded-lg border">
                <h3 className="text-caption text-tertiary mb-4">热门股票</h3>
                {hotStocks.length ? (
                  <div className="space-y-2">
                    {hotStocks.map((s, idx) => (
                      <div
                        key={s[0]}
                        className="hover:bg-hover flex items-center gap-3 rounded-lg px-3 py-2.5 transition-colors"
                      >
                        <span className="text-caption font-number text-tertiary w-6">
                          #{idx + 1}
                        </span>
                        <span className="text-caption font-number text-primary">{s[0]}</span>
                        <span className="text-caption text-secondary">{s[1]}</span>
                        <span className="bg-brand/15 text-brand text-caption font-number ml-auto rounded-sm px-2 py-0.5">
                          {s[2]}次
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-caption text-tertiary py-4 text-center">今日暂无数据</div>
                )}
              </div>

              {/* 热门文章 */}
              <div className="bg-surface border-hairline p-card-inner mb-6 rounded-lg border">
                <h3 className="text-caption text-tertiary mb-4">
                  热门文章 · {hotArticles.length}篇
                </h3>
                {hotArticles.length ? (
                  <div className="space-y-2">
                    {hotArticles.map((a, idx) => (
                      <div
                        key={a.article_id}
                        role="button"
                        tabIndex={0}
                        aria-expanded={expandedId === a.article_id}
                        className="border-hairline p-card-inner bg-canvas cursor-pointer rounded-lg border transition-colors"
                        onClick={() =>
                          setExpandedId(expandedId === a.article_id ? null : a.article_id)
                        }
                        onKeyDown={(e) => {
                          if (e.target !== e.currentTarget) return
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault()
                            setExpandedId(expandedId === a.article_id ? null : a.article_id)
                          }
                        }}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0 flex-1">
                            <div className="mb-1 flex items-center gap-2">
                              <span className="text-caption font-number text-tertiary shrink-0">
                                #{idx + 1}
                              </span>
                              <a
                                href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                                target="_blank"
                                className="text-body text-primary truncate font-medium transition-colors hover:underline"
                                onClick={(e) => e.stopPropagation()}
                                rel="noreferrer"
                              >
                                {a.title}
                              </a>
                            </div>
                            <div className="text-caption text-tertiary flex items-center gap-3">
                              <span>{a.nickname}</span>
                              <span>👍 {a.like_count}</span>
                              <span>💬 {a.comment_count}</span>
                              <span>🔄 {a.forward_count}</span>
                              <span className="bg-brand/15 text-brand text-caption font-number rounded-sm px-1 py-0.5">
                                {a.heat_score}热度
                              </span>
                            </div>
                          </div>
                          <span
                            className="text-caption text-tertiary shrink-0 transition-transform"
                            style={{
                              transform: expandedId === a.article_id ? "rotate(180deg)" : "",
                            }}
                          >
                            ▼
                          </span>
                        </div>
                        {expandedId === a.article_id && (
                          <div className="border-hairline mt-3 border-t pt-3">
                            <SafeHtml
                              html={a.content}
                              className="text-body text-secondary leading-relaxed"
                            />
                            <a
                              href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                              target="_blank"
                              className="text-brand text-caption mt-2 inline-block transition-colors hover:underline"
                              rel="noreferrer"
                            >
                              📎 查看原文
                            </a>
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-caption text-tertiary py-4 text-center">今日暂无数据</div>
                )}
              </div>
            </>
          )}
        </div>
      )}

      {activeSection === "posts" && (
        <div className="py-1 text-center">
          {/* 淘股吧推荐 */}
          {taogubaArticles.length > 0 && (
            <div className="bg-surface border-hairline p-card-inner mb-6 rounded-lg border">
              <h3 className="text-caption text-tertiary mb-4">
                淘股吧推荐 · {taogubaArticles.length}篇
              </h3>
              <div className="space-y-2">
                {taogubaArticles.map((a, idx) => (
                  <div
                    key={`${a.topic_id}-${idx}`}
                    role="button"
                    tabIndex={0}
                    aria-expanded={expandedTgbId === a.topic_id}
                    className="border-hairline p-card-inner bg-canvas cursor-pointer rounded-lg border transition-colors"
                    onClick={() =>
                      setExpandedTgbId(expandedTgbId === a.topic_id ? null : a.topic_id)
                    }
                    onKeyDown={(e) => {
                      if (e.target !== e.currentTarget) return
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault()
                        setExpandedTgbId(expandedTgbId === a.topic_id ? null : a.topic_id)
                      }
                    }}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="mb-1 flex items-center gap-2">
                          <span className="text-caption font-number text-tertiary shrink-0">
                            #{idx + 1}
                          </span>
                          <span className="text-caption text-primary truncate font-medium">
                            {a.subject}
                          </span>
                        </div>
                        <div className="text-caption text-tertiary flex items-center gap-3">
                          <span>{a.user_name}</span>
                          <span>👁️ {a.view_count}</span>
                          <span>💬 {a.reply_count}</span>
                          <span>{a.date_time}</span>
                        </div>
                      </div>
                      <span
                        className="text-caption text-tertiary shrink-0 transition-transform"
                        style={{
                          transform: expandedTgbId === a.topic_id ? "rotate(180deg)" : "",
                        }}
                      >
                        ▼
                      </span>
                    </div>
                    {expandedTgbId === a.topic_id && (
                      <div className="border-hairline mt-3 border-t pt-3">
                        <SafeHtml
                          html={a.content}
                          className="text-body text-secondary leading-relaxed"
                        />
                        <a
                          href={`https://www.tgb.cn/article/${a.new_topic_id}`}
                          target="_blank"
                          className="text-brand text-caption mt-2 inline-block transition-colors hover:underline"
                          rel="noreferrer"
                        >
                          📎 查看原文
                        </a>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
