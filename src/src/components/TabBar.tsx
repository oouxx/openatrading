import { motion } from 'framer-motion'

interface TabBarProps {
  activeTab: string
  onTabChange: (tab: string) => void
}

const tabs = [
  { id: 'strong-stocks', label: '强势股' },
  { id: 'promotion', label: '晋级分析' },
  { id: 'plate', label: '板块异动' },
  { id: 'timeline', label: '时间线' },
  { id: 'sentiment-chip', label: '情绪筹码' },
  { id: 'market-intel', label: '市场情报' },
  { id: 'news', label: '资讯' },
]

export default function TabBar({ activeTab, onTabChange }: TabBarProps) {
  return (
    <div className="flex overflow-x-auto gap-1" style={{ padding: '12px 0' }}>
      {tabs.map((tab) => (
        <motion.button
          key={tab.id}
          onClick={() => onTabChange(tab.id)}
          className={`flex-shrink-0 px-4 transition-colors duration-200 border-b-2 cursor-pointer ${
            activeTab === tab.id
              ? 'text-white border-ai'
              : 'text-ink-muted border-transparent hover:text-white/80'
          }`}
          style={{ paddingTop: 10, paddingBottom: 10 }}
          whileHover={{ y: -1 }}
          whileTap={{ scale: 0.97 }}
          transition={{ duration: 0.15 }}
        >
          <span className="text-caption">{tab.label}</span>
        </motion.button>
      ))}
    </div>
  )
}