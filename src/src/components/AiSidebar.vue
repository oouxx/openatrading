<template>
  <div class="h-full flex flex-col bg-bg-surface" style="min-height: calc(100vh - 44px)">
    <!-- Header -->
    <div class="flex items-center justify-between px-4 py-3 border-b border-hairline">
      <div class="flex items-center gap-2">
        <span class="w-1.5 h-1.5 rounded-full bg-ai"></span>
        <span class="text-caption-strong text-white font-mono">AI 分析</span>
      </div>
      <button
        @click="$emit('close')"
        class="text-ink-muted hover:text-white text-sm cursor-pointer transition-colors"
      >
        ✕
      </button>
    </div>

    <!-- Mode Tabs -->
    <div class="flex border-b border-hairline">
      <button
        v-for="m in modes"
        :key="m.value"
        @click="mode = m.value"
        class="flex-1 py-2.5 text-caption transition-colors cursor-pointer"
        :class="mode === m.value
          ? 'text-ai border-b-2 border-ai'
          : 'text-ink-muted hover:text-white/80'"
      >
        {{ m.label }}
      </button>
    </div>

    <!-- Content Area -->
    <div class="flex-1 overflow-y-auto p-4 space-y-4">
      <!-- Mode 1: Global Summary -->
      <div v-if="mode === 'global_summary'">
        <GlobalSummary :overview="overview" />
      </div>

      <!-- Mode 2: Tab Context -->
      <div v-else-if="mode === 'tab_context'">
        <TabContext :active-tab="activeTab" />
      </div>

      <!-- Mode 3: Q&A -->
      <div v-else class="flex flex-col" style="height: calc(100vh - 188px)">
        <ChatPanel />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, defineComponent, h, computed } from 'vue'

defineProps<{
  activeTab: string
  overview: any
}>()

defineEmits<{
  close: []
}>()

type AiContextMode = 'global_summary' | 'tab_context' | 'qa'

const mode = ref<AiContextMode>('global_summary')

const modes: { label: string; value: AiContextMode }[] = [
  { label: '全局摘要', value: 'global_summary' },
  { label: 'Tab联动', value: 'tab_context' },
  { label: '问答', value: 'qa' },
]

// ─── Mode 1: Global Summary ──────────────────────────

const GlobalSummary = defineComponent({
  props: { overview: Object },
  setup(props: any) {
    const riskBadgeClass = computed(() => {
      if (!props.overview?.risk_level) return 'bg-down/10 text-down'
      switch (props.overview.risk_level) {
        case '高': return 'bg-up/10 text-up'
        case '中': return 'bg-up/5 text-up'
        default: return 'bg-down/10 text-down'
      }
    })

    return () => h('div', { class: 'space-y-4' }, [
      // Risk Level
      h('div', { class: 'bg-bg rounded-card border border-hairline p-card-inner' }, [
        h('p', { class: 'text-caption text-ink-muted mb-2' }, '风险等级'),
        h('div', { class: 'flex items-center gap-3' }, [
          h('span', {
            class: ['px-3 py-1 rounded-button text-caption-strong font-mono', riskBadgeClass.value],
          }, props.overview?.risk_level || '--'),
          h('span', { class: 'text-caption text-ink-muted' }, props.overview?.decision_summary || ''),
        ]),
      ]),

      // KPI Snapshot
      h('div', { class: 'bg-bg rounded-card border border-hairline p-card-inner' }, [
        h('p', { class: 'text-caption text-ink-muted mb-3' }, '市场快照'),
        h('div', { class: 'grid grid-cols-2 gap-3' }, [
          h('div', null, [
            h('p', { class: 'text-caption text-ink-muted' }, '上涨'),
            h('p', { class: 'font-mono font-semibold text-up', style: { fontSize: '22px' } },
              props.overview?.rise_count?.toLocaleString() || '--'),
          ]),
          h('div', null, [
            h('p', { class: 'text-caption text-ink-muted' }, '下跌'),
            h('p', { class: 'font-mono font-semibold text-down', style: { fontSize: '22px' } },
              props.overview?.fall_count?.toLocaleString() || '--'),
          ]),
          h('div', null, [
            h('p', { class: 'text-caption text-ink-muted' }, '涨停'),
            h('p', { class: 'font-mono font-semibold text-up', style: { fontSize: '22px' } },
              props.overview?.limit_up_count?.toLocaleString() || '--'),
          ]),
          h('div', null, [
            h('p', { class: 'text-caption text-ink-muted' }, '跌停'),
            h('p', { class: 'font-mono font-semibold text-down', style: { fontSize: '22px' } },
              props.overview?.limit_down_count?.toLocaleString() || '--'),
          ]),
        ]),
      ]),
    ])
  },
})

// ─── Mode 2: Tab Context ─────────────────────────────

const tabLabels: Record<string, string> = {
  'strong-stocks': '强势股',
  'promotion': '晋级分析',
  'plate': '板块异动',
  'timeline': '时间线',
  'sentiment-chip': '情绪筹码',
  'market-intel': '市场情报',
  'ai': 'AI 观点',
}

const TabContext = defineComponent({
  props: { activeTab: String },
  setup(props: any) {
    const quickQuestions = computed(() => {
      const map: Record<string, string[]> = {
        'strong-stocks': ['这只龙头明天能打吗？', '今天适合打板吗？', '哪个板块明天可能接力？'],
        'promotion': ['历史上4板胜率怎么样？', '现在应该几成仓？', '晋级率下降说明什么？'],
        'plate': ['哪个板块明天可能接力？', '轮动路径怎么看？', '今天为什么这个板块涨？'],
        'timeline': ['今天资金节奏怎么样？', '午后有没有加速信号？', '明天早盘怎么看？'],
        'sentiment-chip': ['情绪能持续几天？', '筹码结构健康吗？', '现在偏热还是偏冷？'],
        'market-intel': ['论坛情绪是否过热？', '题材还能持续吗？', '散户在讨论什么？'],
        'ai': ['帮我总结今日行情', '明天操作方向', '市场风险在哪？'],
      }
      return map[props.activeTab] || []
    })

    return () => h('div', { class: 'space-y-3' }, [
      h('div', { class: 'bg-bg rounded-card border border-hairline p-card-inner' }, [
        h('p', { class: 'text-caption text-ink-muted mb-1' }, '当前模块'),
        h('p', { class: 'text-body-strong text-white' }, tabLabels[props.activeTab] || props.activeTab),
      ]),
      h('div', { class: 'bg-bg rounded-card border border-hairline p-card-inner' }, [
        h('p', { class: 'text-caption text-ai mb-3 font-mono' }, '// 快捷问题'),
        h('div', { class: 'space-y-2' },
          quickQuestions.value.map((q: string) =>
            h('button', {
              class: 'w-full text-left px-3 py-2 rounded-card bg-bg-hover text-caption text-ink-muted hover:text-white transition-colors cursor-pointer',
              onClick: () => askQuick(q),
            }, q)
          )
        ),
      ]),
    ])
  },
})

// ─── Mode 3: Chat Panel ──────────────────────────────

const chatInput = ref('')
const chatMessages = ref<{ role: 'user' | 'ai'; text: string }[]>([])

function askQuick(question: string) {
  mode.value = 'qa'
  chatMessages.value = [{ role: 'user' as const, text: question }, {
    role: 'ai' as const,
    text: '需配置 OPENROUTER_API_KEY 以启用 AI 问答功能。',
  }]
}

const ChatPanel = defineComponent({
  setup() {
    function send() {
      if (!chatInput.value.trim()) return
      chatMessages.value.push({ role: 'user', text: chatInput.value.trim() })
      chatMessages.value.push({
        role: 'ai',
        text: '需配置 OPENROUTER_API_KEY 以启用 AI 问答功能。',
      })
      chatInput.value = ''
    }

    return () => h('div', { class: 'flex flex-col h-full' }, [
      // Messages
      h('div', { class: 'flex-1 overflow-y-auto space-y-3 mb-3' }, [
        chatMessages.value.length === 0
          ? h('p', { class: 'text-caption text-ink-muted text-center py-8' }, '向 AI 提问市场相关问题')
          : chatMessages.value.map((msg: any) =>
              h('div', {
                class: ['px-3 py-2 rounded-card text-caption', msg.role === 'user'
                  ? 'bg-ai/15 text-white ml-8'
                  : 'bg-bg border border-hairline text-ink-muted mr-4'],
              }, msg.text)
            ),
      ]),
      // Input
      h('div', { class: 'flex gap-2' }, [
        h('input', {
          class: 'flex-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono',
          placeholder: '输入问题...',
          value: chatInput.value,
          onInput: (e: any) => chatInput.value = e.target.value,
          onKeydown: (e: any) => { if (e.key === 'Enter') send() },
        }),
        h('button', {
          class: 'px-4 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer',
          onClick: send,
        }, '发送'),
      ]),
    ])
  },
})
</script>
