import { useState } from "react"
import { useSentiment } from "@/hooks/query/useSentiment"

const subSections = [
  { id: "topics", label: "题材热度与舆情" },
  { id: "posts", label: "论坛热帖聚合" },
]

export default function MarketIntelTab() {
  const [activeSection, setActiveSection] = useState("topics")
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [expandedTgbId, setExpandedTgbId] = useState<number | null>(null)

  const { data: sentimentRaw } = useSentiment()
  const sentimentData = sentimentRaw as any
  const hasData = !!(
    sentimentData &&
    (sentimentData.hot_stocks?.length ||
      sentimentData.hot_articles?.length ||
      sentimentData.taoguba_articles?.length)
  )

  const hotStocks: Array<[string, string, number]> = sentimentData?.hot_stocks || []
  const hotArticles: Array<{
    article_id: string
    title: string
    nickname: string
    like_count: number
    comment_count: number
    forward_count: number
    heat_score: number
    content: string
  }> = sentimentData?.hot_articles || []
  const taogubaArticles: Array<{
    topic_id: number
    new_topic_id: number
    subject: string
    user_name: string
    view_count: number
    reply_count: number
    date_time: string
    content: string
  }> = sentimentData?.taoguba_articles || []

  const trendText = sentimentData?.trend || "暂无数据"
  const trendColor =
    trendText === "偏多" ? "text-up" : trendText === "中性" ? "text-brand" : "text-down"

  return (
    <div>
      <div className="mb-6 flex items-center gap-1">
        {subSections.map((s) => (
          <button
            key={s.id}
            onClick={() => setActiveSection(s.id)}
            className={`text-caption rounded-full cursor-pointer px-4 py-2 font-mono transition-colors ${
              activeSection === s.id ? "bg-brand/20 text-brand" : "text-secondary hover:text-primary/80"
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>

      {activeSection === "topics" && (
        <div>
          <div className="text-caption mb-8 flex items-center gap-3">
            <span className="text-primary/60">
              热度趋势: <b className={trendColor}>{trendText}</b>
            </span>
          </div>

          {!hasData ? (
            <div className="text-caption py-12 text-center text-primary/40">
              暂无语情数据 — 需配置 JIUYAN_PHONE
            </div>
          ) : (
            <>
              {/* 热门股票 */}
              <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg mb-6 border">
                <h3 className="text-caption mb-4 tracking-wider text-primary/40 uppercase">
                  热门股票
                </h3>
                {hotStocks.length ? (
                  <div className="space-y-1">
                    {hotStocks.map((s, idx) => (
                      <div
                        key={s[0]}
                        className="rounded-lg flex items-center gap-3 px-2 py-2 transition-colors hover:bg-white/4"
                      >
                        <span className="text-caption w-6 font-mono text-primary/40">#{idx + 1}</span>
                        <span className="text-caption font-mono text-primary">{s[0]}</span>
                        <span className="text-caption text-primary/60">{s[1]}</span>
                        <span className="bg-brand/15 rounded-sm text-brand text-caption ml-auto px-2 py-0.5 font-mono">
                          {s[2]}次
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-caption py-4 text-center text-primary/40">今日暂无数据</div>
                )}
              </div>

              {/* 热门文章 */}
              <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg mb-6 border">
                <h3 className="text-caption mb-4 tracking-wider text-primary/40 uppercase">
                  热门文章 · {hotArticles.length}篇
                </h3>
                {hotArticles.length ? (
                  <div className="space-y-2">
                    {hotArticles.map((a, idx) => (
                      <div
                        key={a.article_id}
                        className="border-hairline-dark p-card-inner rounded-lg cursor-pointer border bg-white/4 transition-colors"
                        onClick={() =>
                          setExpandedId(expandedId === a.article_id ? null : a.article_id)
                        }
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0 flex-1">
                            <div className="mb-1 flex items-center gap-2">
                              <span className="text-caption shrink-0 font-mono text-primary/40">
                                #{idx + 1}
                              </span>
                              <a
                                href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                                target="_blank"
                                className="text-caption truncate font-medium text-primary hover:underline"
                                onClick={(e) => e.stopPropagation()}
                                rel="noreferrer"
                              >
                                {a.title}
                              </a>
                            </div>
                            <div className="text-caption flex items-center gap-3 text-primary/40">
                              <span>{a.nickname}</span>
                              <span>👍 {a.like_count}</span>
                              <span>💬 {a.comment_count}</span>
                              <span>🔄 {a.forward_count}</span>
                              <span className="bg-brand/15 rounded-sm text-brand text-caption px-1 py-0.5 font-mono">
                                {a.heat_score}热度
                              </span>
                            </div>
                          </div>
                          <span
                            className="text-caption shrink-0 text-primary/40 transition-transform"
                            style={{
                              transform: expandedId === a.article_id ? "rotate(180deg)" : "",
                            }}
                          >
                            ▼
                          </span>
                        </div>
                        {expandedId === a.article_id && (
                          <div className="border-hairline-dark mt-3 border-t pt-3">
                            <div
                              className="text-caption leading-relaxed text-primary/60"
                              dangerouslySetInnerHTML={{ __html: a.content }}
                            />
                            <a
                              href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                              target="_blank"
                              className="text-brand text-caption mt-2 inline-block hover:underline"
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
                  <div className="text-caption py-4 text-center text-primary/40">今日暂无数据</div>
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
            <div className="bg-surface-dark border-hairline-dark p-card-inner rounded-lg mb-6 border">
              <h3 className="text-caption mb-4 tracking-wider text-primary/40 uppercase">
                淘股吧推荐 · {taogubaArticles.length}篇
              </h3>
              <div className="space-y-2">
                {taogubaArticles.map((a, idx) => (
                  <div
                    key={`${a.topic_id}-${idx}`}
                    className="border-hairline-dark p-card-inner rounded-lg cursor-pointer border bg-white/4 transition-colors"
                    onClick={() =>
                      setExpandedTgbId(expandedTgbId === a.topic_id ? null : a.topic_id)
                    }
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0 flex-1">
                        <div className="mb-1 flex items-center gap-2">
                          <span className="text-caption shrink-0 font-mono text-primary/40">
                            #{idx + 1}
                          </span>
                          <span className="text-caption truncate font-medium text-primary">
                            {a.subject}
                          </span>
                        </div>
                        <div className="text-caption flex items-center gap-3 text-primary/40">
                          <span>{a.user_name}</span>
                          <span>👁️ {a.view_count}</span>
                          <span>💬 {a.reply_count}</span>
                          <span>{a.date_time}</span>
                        </div>
                      </div>
                      <span
                        className="text-caption shrink-0 text-primary/40 transition-transform"
                        style={{
                          transform: expandedTgbId === a.topic_id ? "rotate(180deg)" : "",
                        }}
                      >
                        ▼
                      </span>
                    </div>
                    {expandedTgbId === a.topic_id && (
                      <div className="border-hairline-dark mt-3 border-t pt-3">
                        <div
                          className="text-caption leading-relaxed text-primary/60"
                          dangerouslySetInnerHTML={{ __html: a.content }}
                        />
                        <a
                          href={`https://www.tgb.cn/article/${a.new_topic_id}`}
                          target="_blank"
                          className="text-brand text-caption mt-2 inline-block hover:underline"
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
