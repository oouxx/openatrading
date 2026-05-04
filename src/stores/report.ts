import { defineStore } from 'pinia'
import { ref, Ref } from 'vue'

interface MarketOverview {
  rise_count: number
  fall_count: number
  bomb_rate: number
  sentiment: string
}

interface Stock {
  symbol: string
  stock_name: string
  change_percent: number
  turnover_ratio: number
  limit_up_days: number
}

interface PlateAbnormalEvent {
  time: string
  plate_name: string
  description: string
  related_stocks: string[]
}

interface SentimentResult {
  hot_stocks: [string, string, number][]
  trend: string
  summary: string
  articles: any[]
}

interface RoleAnalysis {
  role: string
  content: string
  elapsed_ms: number
}

interface TgbUserSummary {
  user_id: string
  post_count: number
  latest_update: string
}

interface Report {
  date: string
  overview: MarketOverview
  limit_up: Stock[]
  plate_abnormal: PlateAbnormalEvent[]
  sentiment: SentimentResult | null
  analyses: RoleAnalysis[]
  taoguba_users: TgbUserSummary[]
}

export const useReportStore = defineStore('report', () => {
  const report: Ref<Report | null> = ref(null)
  const loading = ref(false)

  const mockReport: Report = {
    date: '2026-05-04',
    overview: {
      rise_count: 120,
      fall_count: 30,
      bomb_rate: 0.25,
      sentiment: '中'
    },
    limit_up: [
      { symbol: '600519', stock_name: '贵州茅台', change_percent: 10.0, turnover_ratio: 2.5, limit_up_days: 3 },
      { symbol: '000001', stock_name: '平安银行', change_percent: 9.8, turnover_ratio: 3.1, limit_up_days: 2 },
      { symbol: '600036', stock_name: '招商银行', change_percent: 9.5, turnover_ratio: 2.8, limit_up_days: 2 },
    ],
    plate_abnormal: [
      { time: '09:30', plate_name: '新能源', description: '板块异动拉升', related_stocks: ['600519', '000001'] },
      { time: '10:15', plate_name: '半导体', description: '资金流入明显', related_stocks: ['600036'] },
    ],
    sentiment: {
      hot_stocks: [['600519', '贵州茅台', 15], ['000001', '平安银行', 10]],
      trend: '震荡上行',
      summary: '市场情绪偏乐观，资金关注蓝筹股',
      articles: []
    },
    analyses: [
      { role: '游资操盘手', content: '蓝筹股表现强势，建议持有优质标的', elapsed_ms: 1200 },
      { role: '量化分析师', content: '技术指标显示短期超买，注意回调风险', elapsed_ms: 980 },
      { role: '风控官', content: '整体风险可控，炸板率处于合理区间', elapsed_ms: 1100 },
    ],
    taoguba_users: [
      { user_id: '2577911', post_count: 30, latest_update: '2026-05-04' }
    ]
  }

  async function fetchReport(format: string = 'markdown') {
    loading.value = true
    try {
      setTimeout(() => {
        report.value = mockReport
        loading.value = false
      }, 500)
    } catch (error) {
      console.error('Failed to fetch report:', error)
      loading.value = false
    }
  }
  }
  }

  function clearReport() {
    report.value = null
  }

  return {
    report,
    loading,
    fetchReport,
    clearReport
  }
})
