import { motion } from "framer-motion"

interface KpiCardsProps {
  overview: {
    rise_count: number
    fall_count: number
    limit_up_count: number
    limit_down_count: number
    bomb_rate: number
    market_temperature: number
    yesterday_limit_up_avg_pcp: number
  }
  riskLevel: string
  decisionSummary: string
}

export default function KpiCards({ overview, riskLevel, decisionSummary }: KpiCardsProps) {
  const bombPct = overview.bomb_rate * 100

  const sentimentTemp = (() => {
    const t = overview.market_temperature
    if (t > 0) return Math.round(t)
    const total = overview.rise_count + overview.fall_count
    if (total === 0) return 50
    const riseRatio = overview.rise_count / total
    const bombFactor = 1 - bombPct / 100
    const limitRatio =
      Math.min(overview.limit_up_count / Math.max(overview.limit_down_count, 1), 10) / 10
    return Math.round(riseRatio * 60 + bombFactor * 20 + limitRatio * 20)
  })()

  const ratio =
    overview.fall_count === 0 ? "∞" : (overview.rise_count / overview.fall_count).toFixed(2)

  const ratioColorClass = (() => {
    const r = overview.rise_count / Math.max(overview.fall_count, 1)
    if (r >= 2) return "text-up"
    if (r >= 1) return "text-brand"
    return "text-down"
  })()

  const bombColorClass = bombPct > 30 ? "text-up" : bombPct > 20 ? "text-brand" : "text-down"
  const bombLabel = bombPct > 30 ? "偏高 注意" : bombPct > 20 ? "正常偏高" : "偏低 惜售"

  const prevLimitUpClass = (() => {
    const v = overview.yesterday_limit_up_avg_pcp
    if (v == null || v === 0) return "text-secondary"
    return v > 0 ? "text-up" : "text-down"
  })()

  const prevLimitUpLabel = (() => {
    const v = overview.yesterday_limit_up_avg_pcp
    if (v == null) return "暂无数据"
    if (v > 0.03) return "赚钱效应强"
    if (v > 0) return "微利"
    if (v > -0.03) return "微亏"
    return "亏钱效应"
  })()

  const riskBadgeClass =
    riskLevel === "高"
      ? "bg-up/10 text-up"
      : riskLevel === "中"
        ? "bg-up/5 text-up"
        : "bg-down/10 text-down"

  const sentimentLabel = sentimentTemp >= 75 ? "偏热" : sentimentTemp >= 50 ? "正常" : "偏冷"
  const sentimentColorClass =
    sentimentTemp >= 75 ? "text-up" : sentimentTemp >= 50 ? "text-brand" : "text-down"
  const sentimentBorderClass =
    sentimentTemp >= 75 ? "border-l-up" : sentimentTemp >= 50 ? "border-l-ai" : "border-l-down"

  const cards = [
    {
      label: "情绪温度",
      value: sentimentTemp.toFixed(0),
      sub: `/100 ${sentimentLabel}`,
      valueClass: sentimentColorClass,
      borderClass: `border-l-2 ${sentimentBorderClass}`,
      delay: 0,
    },
    {
      label: "涨停家数",
      value: String(overview.limit_up_count),
      sub: `跌停 ${overview.limit_down_count}`,
      valueClass: "text-up",
      borderClass: "border-l-2 border-l-up",
      delay: 0.06,
    },
    {
      label: "炸板率",
      value: `${bombPct.toFixed(1)}%`,
      sub: bombLabel,
      valueClass: bombColorClass,
      borderClass: "",
      delay: 0.12,
    },
    {
      label: "昨日涨停表现",
      value:
        overview.yesterday_limit_up_avg_pcp != null
          ? `${(overview.yesterday_limit_up_avg_pcp * 100).toFixed(2)}%`
          : "--",
      sub: prevLimitUpLabel,
      valueClass: prevLimitUpClass,
      borderClass: "",
      delay: 0.18,
    },
    {
      label: "涨跌比",
      value: ratio,
      sub: `上${overview.rise_count} 下${overview.fall_count}`,
      valueClass: ratioColorClass,
      borderClass: "border-l-2 border-l-ai",
      delay: 0.24,
    },
  ]

  return (
    <div>
      <div className="mb-6 grid grid-cols-2 gap-3 lg:grid-cols-5 lg:gap-4">
        {cards.map((card) => (
          <motion.div
            key={card.label}
            className={`p-card-inner rounded-lg border-hairline bg-surface border ${card.borderClass}`}
            initial={{ opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.35, delay: card.delay }}
          >
            <p className="text-caption text-secondary mb-2">{card.label}</p>
            <p className={`font-number font-semibold ${card.valueClass}`} className="text-h1">
              {card.value}
            </p>
            <p className="text-caption text-secondary mt-1">{card.sub}</p>
          </motion.div>
        ))}
      </div>

      {decisionSummary && (
        <div className="bg-surface border-hairline rounded-lg flex items-center gap-4 border px-5 py-4">
          <span
            className={`text-caption rounded-full px-3 py-1 font-number ${riskBadgeClass}`}
          >
            {riskLevel}
          </span>
          <span className="text-body text-secondary">{decisionSummary}</span>
        </div>
      )}
    </div>
  )
}
