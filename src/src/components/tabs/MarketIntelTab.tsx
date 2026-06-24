import { useState } from 'react'
import { useSentiment } from '@/hooks/query/useSentiment'

const subSections = [
  { id: 'topics', label: '题材热度与舆情' },
  { id: 'posts', label: '论坛热帖聚合' },
]

export default function MarketIntelTab() {
  const [activeSection, setActiveSection] = useState('topics')
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

  const trendText = sentimentData?.trend || '暂无数据'
  const trendColor =
    trendText === '偏多' ? 'text-up' : trendText === '中性' ? 'text-ai' : 'text-down'

  return (
    <div>
      <div className="flex items-center gap-1 mb-6">
        {subSections.map((s) => (
          <button
            key={s.id}
            onClick={() => setActiveSection(s.id)}
            className={`px-4 py-1.5 rounded-button text-caption font-mono transition-colors cursor-pointer ${
              activeSection === s.id ? 'bg-ai/20 text-ai' : 'text-ink-muted hover:text-white/80'
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>

      {activeSection === 'topics' && (
        <div>
          <div className="flex items-center gap-3 mb-8 text-caption">
            <span className="text-white/60">
              热度趋势: <b className={trendColor}>{trendText}</b>
            </span>
          </div>

          {!hasData ? (
            <div className="text-center py-12 text-caption text-white/40">
              暂无语情数据 — 需配置 JIUYAN_PHONE
            </div>
          ) : (
            <>
              {/* 热门股票 */}
              <div className="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6">
                <h3 className="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">热门股票</h3>
                {hotStocks.length ? (
                  <div className="space-y-1">
                    {hotStocks.map((s, idx) => (
                      <div
                        key={s[0]}
                        className="flex items-center gap-3 py-1.5 px-2 rounded-card transition-colors hover:bg-white/[0.04]"
                      >
                        <span className="text-caption font-mono w-6 text-white/40">#{idx + 1}</span>
                        <span className="text-caption font-mono text-white">{s[0]}</span>
                        <span className="text-caption text-white/60">{s[1]}</span>
                        <span className="text-caption font-mono ml-auto px-2 py-0.5 rounded-badge bg-ai/15 text-ai">
                          {s[2]}次
                        </span>
                      </div>
                    ))}
                  </div>
                ) : (
                  <div className="text-center py-4 text-caption text-white/40">今日暂无数据</div>
                )}
              </div>

              {/* 热门文章 */}
              <div className="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6">
                <h3 className="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">
                  热门文章 · {hotArticles.length}篇
                </h3>
                {hotArticles.length ? (
                  <div className="space-y-2">
                    {hotArticles.map((a, idx) => (
                      <div
                        key={a.article_id}
                        className="p-card-inner rounded-card border border-hairline-dark bg-white/[0.04] cursor-pointer transition-colors"
                        onClick={() => setExpandedId(expandedId === a.article_id ? null : a.article_id)}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-1.5 mb-1">
                              <span className="text-caption font-mono shrink-0 text-white/40">#{idx + 1}</span>
                              <a
                                href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                                target="_blank"
                                className="text-caption font-medium truncate hover:underline text-white"
                                onClick={(e) => e.stopPropagation()}
                                rel="noreferrer"
                              >
                                {a.title}
                              </a>
                            </div>
                            <div className="flex items-center gap-3 text-caption text-white/40">
                              <span>{a.nickname}</span>
                              <span>👍 {a.like_count}</span>
                              <span>💬 {a.comment_count}</span>
                              <span>🔄 {a.forward_count}</span>
                              <span className="text-caption font-mono px-1 py-0.5 rounded-badge bg-ai/15 text-ai">
                                {a.heat_score}热度
                              </span>
                            </div>
                          </div>
                          <span
                            className="text-caption shrink-0 transition-transform text-white/40"
                            style={{ transform: expandedId === a.article_id ? 'rotate(180deg)' : '' }}
                          >
                            ▼
                          </span>
                        </div>
                        {expandedId === a.article_id && (
                          <div className="mt-3 pt-3 border-t border-hairline-dark">
                            <div className="text-caption leading-relaxed text-white/60" dangerouslySetInnerHTML={{ __html: a.content }} />
                            <a
                              href={`https://www.jiuyangongshe.com/a/${a.article_id}`}
                              target="_blank"
                              className="inline-block mt-2 text-caption hover:underline text-ai"
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
                  <div className="text-center py-4 text-caption text-white/40">今日暂无数据</div>
                )}
              </div>

              {/* 淘股吧推荐 */}
              {taogubaArticles.length > 0 && (
                <div className="bg-surface-dark rounded-card border border-hairline-dark p-card-inner mb-6">
                  <h3 className="text-caption-strong text-white/40 mb-4 tracking-wider uppercase">
                    淘股吧推荐 · {taogubaArticles.length}篇
                  </h3>
                  <div className="space-y-2">
                    {taogubaArticles.map((a, idx) => (
                      <div
                        key={`${a.topic_id}-${idx}`}
                        className="p-card-inner rounded-card border border-hairline-dark bg-white/[0.04] cursor-pointer transition-colors"
                        onClick={() => setExpandedTgbId(expandedTgbId === a.topic_id ? null : a.topic_id)}
                      >
                        <div className="flex items-start justify-between gap-2">
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-1.5 mb-1">
                              <span className="text-caption font-mono shrink-0 text-white/40">#{idx + 1}</span>
                              <span className="text-caption font-medium truncate text-white">{a.subject}</span>
                            </div>
                            <div className="flex items-center gap-3 text-caption text-white/40">
                              <span>{a.user_name}</span>
                              <span>👁️ {a.view_count}</span>
                              <span>💬 {a.reply_count}</span>
                              <span>{a.date_time}</span>
                            </div>
                          </div>
                          <span
                            className="text-caption shrink-0 transition-transform text-white/40"
                            style={{ transform: expandedTgbId === a.topic_id ? 'rotate(180deg)' : '' }}
                          >
                            ▼
                          </span>
                        </div>
                        {expandedTgbId === a.topic_id && (
                          <div className="mt-3 pt-3 border-t border-hairline-dark">
                            <div className="text-caption leading-relaxed text-white/60" dangerouslySetInnerHTML={{ __html: a.content }} />
                            <a
                              href={`https://www.tgb.cn/article/${a.new_topic_id}`}
                              target="_blank"
                              className="inline-block mt-2 text-caption hover:underline text-ai"
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
            </>
          )}
        </div>
      )}

      {activeSection === 'posts' && (
        <div className="text-center py-16">
          <span className="text-caption font-mono text-white/40">淘股吧 — 需配置 TAOGUBA_USER</span>
        </div>
      )}

    </div>
  )
}
