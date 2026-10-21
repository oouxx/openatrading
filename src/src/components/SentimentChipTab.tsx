import { useMemo } from "react"
import { motion } from "motion/react"
import { motionTransition } from "@/lib/motion"
import { type SentimentHistoryDay } from "@/stores/data"
import { useSentimentHistory } from "@/hooks/query/useSentimentHistory"
import { useOverview } from "@/hooks/query/useOverview"
import SentimentHistory from "./SentimentHistory"

function getFeature(row: SentimentHistoryDay): string {
  const s = row.sentiment
  const b = row.bomb_rate
  if (s >= 75 && b < 20) return "偏热，情绪好"
  if (s >= 75) return "偏热，炸板偏高"
  if (s >= 60) return "正常偏暖"
  if (s >= 50) return "正常"
  if (s >= 40) return "偏冷"
  return "冰点"
}

export default function SentimentChipTab() {
  const { data: overview } = useOverview()
  const { data: historyRaw = [] } = useSentimentHistory()

  const history = historyRaw as SentimentHistoryDay[]

  const currentSentiment = history.length ? history[history.length - 1].sentiment : 50
  const currentSentimentPct = Math.min(Math.max(currentSentiment, 0), 100)

  // Count consecutive hot/cold days
  const { hotDays, coldDays } = useMemo(() => {
    let hot = 0
    let cold = 0
    for (let i = history.length - 1; i >= 0; i--) {
      if (history[i].sentiment >= 75) hot++
      else break
    }
    for (let i = history.length - 1; i >= 0; i--) {
      if (history[i].sentiment < 50) cold++
      else break
    }
    return { hotDays: hot, coldDays: cold }
  }, [history])

  const currentPhaseLabel = useMemo(() => {
    const s = currentSentiment
    if (s >= 75) return hotDays > 1 ? `偏热区间 第${hotDays}日` : "偏热区间"
    if (s >= 50) return "正常区间"
    return coldDays > 1 ? `偏冷区间 第${coldDays}日` : "偏冷区间"
  }, [currentSentiment, hotDays, coldDays])

  const currentPhaseClass = useMemo(() => {
    const s = currentSentiment
    if (s >= 75) return "bg-up/15 text-up"
    if (s >= 50) return "bg-brand/15 text-brand"
    return "bg-down/15 text-down"
  }, [currentSentiment])

  const phaseAdvice = useMemo(() => {
    const s = currentSentiment
    if (s >= 75 && hotDays >= 3)
      return "历史规律：偏热第3日后，次日大概率出现情绪降温或震荡整理。建议：今日减少新建仓，持仓者注意设好止盈位。"
    if (s >= 75) return "情绪偏热，短线赚钱效应好，但需警惕连续升温后的回调风险。"
    if (s >= 50) return "情绪正常偏暖，短线操作环境尚可，可适度参与。"
    if (coldDays >= 3) return "连续偏冷，历史规律显示情绪冰点后可能迎来反弹，关注企稳信号。"
    return "情绪偏冷，建议观望为主，等待回暖信号再参与。"
  }, [currentSentiment, hotDays, coldDays])

  const bombRate = useMemo(() => {
    const b = overview?.bomb_rate ?? 0
    return b > 1 ? b : b * 100
  }, [overview])

  const bombRateClass = bombRate > 30 ? "text-up" : bombRate > 20 ? "text-brand" : "text-down"

  const riseRatio = useMemo(() => {
    const r = overview?.rise_count ?? 0
    const f = Math.max(overview?.fall_count ?? 1, 1)
    const ratio = r / f
    return {
      display: ratio.toFixed(2),
      class: ratio >= 2 ? "text-up" : ratio >= 1 ? "text-brand" : "text-down",
    }
  }, [overview])

  const sentimentEval = useMemo(() => {
    const s = currentSentiment
    if (s >= 75) return "过热"
    if (s >= 60) return "偏暖"
    if (s >= 50) return "正常"
    if (s >= 40) return "偏冷"
    return "冰点"
  }, [currentSentiment])

  const sentimentEvalClass = useMemo(() => {
    const s = currentSentiment
    if (s >= 75) return "text-up"
    if (s >= 50) return "text-brand"
    return "text-down"
  }, [currentSentiment])

  const riskHint = useMemo(() => {
    const s = currentSentiment
    if (s >= 75 && bombRate > 25) return "情绪过热+炸板率高，注意风险"
    if (s >= 75) return "情绪过热，注意回调"
    if (bombRate > 30) return "炸板率偏高，追涨风险大"
    if (s < 50) return "情绪偏冷，观望为主"
    return "暂无明显风险信号"
  }, [currentSentiment, bombRate])

  return (
    <div>
      <div className="mb-8 grid grid-cols-1 gap-6 lg:grid-cols-2">
        {/* 市场情绪周期判断 */}
        <motion.div
          className="bg-surface border-hairline p-card-inner rounded-lg border"
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={motionTransition.slow}
        >
          <h3 className="text-caption text-secondary mb-4">市场情绪周期判断</h3>
          {!history.length ? (
            <div className="text-caption text-secondary py-8 text-center">加载中...</div>
          ) : (
            <div className="space-y-3">
              <div className="flex items-center gap-3">
                <span className="text-caption text-secondary">当前：</span>
                <span
                  className={`text-caption font-number rounded-full px-3 py-1 ${currentPhaseClass}`}
                >
                  {currentPhaseLabel}
                </span>
              </div>
              <div className="bg-canvas h-2 overflow-hidden rounded-full">
                <div
                  className="from-down to-up via-ai h-full rounded-full bg-linear-to-r"
                  style={{ width: currentSentimentPct + "%" }}
                />
              </div>
              <div className="text-caption text-secondary flex justify-between">
                <span>冷淡</span>
                <span>正常</span>
                <span>偏热</span>
              </div>
              <p className="text-body text-secondary mt-3">{phaseAdvice}</p>
            </div>
          )}
        </motion.div>

        {/* 市场筹码结构分析 */}
        <motion.div
          className="bg-surface border-hairline p-card-inner rounded-lg border"
          initial={{ opacity: 0, x: 20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={motionTransition.slow}
        >
          <h3 className="text-caption text-secondary mb-4">市场筹码结构分析</h3>
          {!overview ? (
            <div className="text-caption text-secondary py-8 text-center">加载中...</div>
          ) : (
            <div className="space-y-3">
              <div className="text-caption flex justify-between">
                <span className="text-secondary">涨停家数</span>
                <span className="text-up font-number">{overview.limit_up_count}</span>
              </div>
              <div className="text-caption flex justify-between">
                <span className="text-secondary">跌停家数</span>
                <span className="text-down font-number">{overview.limit_down_count}</span>
              </div>
              <div className="text-caption flex justify-between">
                <span className="text-secondary">炸板率</span>
                <span className={`font-number ${bombRateClass}`}>{bombRate.toFixed(1)}%</span>
              </div>
              <div className="text-caption flex justify-between">
                <span className="text-secondary">涨跌比</span>
                <span className={`font-number ${riseRatio.class}`}>{riseRatio.display}</span>
              </div>
              <div className="border-hairline mt-3 border-t pt-3">
                <div className="text-caption flex justify-between">
                  <span className="text-secondary">情绪评价</span>
                  <span className={`font-number ${sentimentEvalClass}`}>{sentimentEval}</span>
                </div>
                <div className="text-caption mt-1 flex justify-between">
                  <span className="text-secondary">风险提示</span>
                  <span className="text-up font-number">{riskHint}</span>
                </div>
              </div>
            </div>
          )}
        </motion.div>
      </div>
      <SentimentHistory />

      {/* 情绪历史表格 */}
      <motion.div
        className="bg-surface border-hairline overflow-hidden rounded-lg border"
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ ...motionTransition.slow, delay: 0.15 }}
      >
        <div className="border-hairline p-card-inner border-b">
          <h3 className="text-caption text-secondary">近{history.length}日情绪数据</h3>
        </div>
        {!history.length ? (
          <div className="text-caption text-secondary py-8 text-center">暂无数据</div>
        ) : (
          <table className="data-table">
            <thead>
              <tr>
                <th>日期</th>
                <th className="text-right">情绪值</th>
                <th className="text-right">涨停数</th>
                <th className="text-right">炸板率</th>
                <th className="text-right">涨跌比</th>
                <th>市场特征</th>
              </tr>
            </thead>
            <tbody>
              {history.map((row) => (
                <tr key={row.date}>
                  <td className="font-number">{row.date}</td>
                  <td
                    className={`font-number text-right ${row.sentiment >= 75 ? "text-up" : row.sentiment >= 60 ? "text-brand" : "text-down"}`}
                  >
                    {row.sentiment.toFixed(1)}
                  </td>
                  <td className="font-number text-right">{row.limit_up_count}</td>
                  <td className="font-number text-right">{row.bomb_rate.toFixed(1)}%</td>
                  <td className="font-number text-right">{row.rise_ratio.toFixed(2)}</td>
                  <td>{getFeature(row)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </motion.div>
    </div>
  )
}
