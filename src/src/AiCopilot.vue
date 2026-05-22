<template>
  <div class="copilot-root">
    <!-- Ambient background layers -->
    <div class="bg-ambient" />
    <div class="bg-grid" />
    <div class="bg-scanline" />

    <!-- Page content -->
    <div class="page-content">
      <!-- Header -->
      <header class="copilot-header">
        <div class="header-left">
          <router-link to="/trading" class="back-link">
            <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m15 18-6-6 6-6"/></svg>
          </router-link>
          <div class="header-title-group">
            <h1 class="header-title">Neural<span class="text-ai">Command</span></h1>
            <div class="ai-status">
              <span class="status-dot" />
              <span class="status-text">AI Online</span>
            </div>
          </div>
        </div>
        <div class="header-right">
          <!-- Role selector -->
          <div class="role-selector">
            <button
              @click="showRoleMenu = !showRoleMenu"
              class="role-trigger"
            >
              <span class="role-icon">
                <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a5 5 0 1 0 0 10 5 5 0 0 0 0-10z"/><path d="M20 21a8 8 0 0 0-16 0"/></svg>
              </span>
              <span class="role-name">{{ currentRoleDisplay }}</span>
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 chevron" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
            </button>
            <div v-if="showRoleMenu" class="role-menu">
              <button
                v-for="r in aiRoles"
                :key="r.name"
                @click="selectRole(r.name)"
                class="role-option"
                :class="{ active: r.name === currentRole }"
              >
                <span>{{ r.display_name }}</span>
                <svg v-if="r.name === currentRole" xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M20 6 9 17l-5-5"/></svg>
              </button>
            </div>
          </div>
        </div>
      </header>

      <!-- Main content -->
      <main class="copilot-main">
        <!-- Left: Context Panel -->
        <aside class="context-panel">
          <div class="context-section">
            <div class="section-label">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>
              市场快照
            </div>
            <div v-if="overview.data" class="snapshot-grid">
              <div class="snap-item">
                <span class="snap-label">涨停 / 跌停</span>
                <span class="snap-value">
                  <em class="text-up">{{ overview.data.limit_up_count }}</em>
                  <span class="text-ink-muted"> / </span>
                  <em class="text-down">{{ overview.data.limit_down_count }}</em>
                </span>
              </div>
              <div class="snap-item">
                <span class="snap-label">上涨 / 下跌</span>
                <span class="snap-value">
                  <em class="text-up">{{ overview.data.rise_count }}</em>
                  <span class="text-ink-muted"> / </span>
                  <em class="text-down">{{ overview.data.fall_count }}</em>
                </span>
              </div>
              <div class="snap-item">
                <span class="snap-label">炸板率</span>
                <span class="snap-value text-ink">{{ (overview.data.bomb_rate * 100).toFixed(2) }}%</span>
              </div>
              <div class="snap-item">
                <span class="snap-label">市场温度</span>
                <span class="snap-value" :class="tempColor">{{ overview.data.market_temperature.toFixed(0) }}°</span>
              </div>
            </div>
            <div v-else class="context-loading">
              <span class="loading-pulse" />
              <span class="text-caption text-ink-muted">等待数据...</span>
            </div>
          </div>

          <div class="context-section">
            <div class="section-label">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20V10"/><path d="M18 20V4"/><path d="M6 20v-4"/></svg>
              风险 & 情绪
            </div>
            <div v-if="overview.data" class="risk-panel">
              <div class="risk-level-row">
                <span class="snap-label">风险等级</span>
                <span class="risk-badge" :class="riskClass">{{ overview.data.risk_level }}</span>
              </div>
              <p class="decision-text">{{ overview.data.decision_summary || '暂无决策摘要' }}</p>
            </div>
            <div v-else class="context-loading">
              <span class="loading-pulse" />
            </div>
          </div>

          <div class="context-section">
            <div class="section-label">
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>
              指数行情
            </div>
            <div v-if="indexQuotes.data.length" class="index-list">
              <div v-for="idx in indexQuotes.data" :key="idx.symbol" class="index-row">
                <span class="index-name">{{ idx.name }}</span>
                <span class="index-price">{{ idx.last_px.toFixed(2) }}</span>
                <span class="index-change" :class="idx.px_change_rate >= 0 ? 'text-up' : 'text-down'">
                  {{ idx.px_change_rate >= 0 ? '+' : '' }}{{ idx.px_change_rate.toFixed(2) }}%
                </span>
              </div>
            </div>
            <div v-else class="context-loading">
              <span class="loading-pulse" />
            </div>
          </div>
        </aside>

        <!-- Right: Chat Panel -->
        <section class="chat-panel">
          <!-- Messages -->
          <div ref="msgContainer" class="messages-area">
            <div class="messages-column">
            <!-- Empty state -->
            <div v-if="messages.length === 0" class="empty-state">
              <motion.div
                class="empty-greeting"
                :initial="{ opacity: 0, y: 20 }"
                :animate="{ opacity: 1, y: 0 }"
                :transition="{ duration: 0.5, ease: 'easeOut' }"
              >
                <div class="greeting-ring">
                  <div class="ring-outer" />
                  <div class="ring-inner" />
                  <span class="greeting-emoji">◆</span>
                </div>
                <h2 class="greeting-title">有什么可以帮你的？</h2>
                <p class="greeting-sub">AI 交易副驾驶已接入实时行情数据，随时回答短线市场问题</p>
              </motion.div>

              <div class="suggestions">
                <motion.button
                  v-for="(q, qi) in suggestedQuestions"
                  :key="q"
                  @click="send(q)"
                  class="suggestion-chip"
                  :initial="{ opacity: 0, y: 12 }"
                  :animate="{ opacity: 1, y: 0 }"
                  :transition="{ duration: 0.3, delay: 0.15 + qi * 0.06 }"
                  :whileHover="{ y: -2, borderColor: 'rgba(127,119,221,0.4)' }"
                >
                  {{ q }}
                </motion.button>
              </div>
            </div>

            <!-- Messages -->
            <motion.div
              v-for="(msg, idx) in messages"
              :key="idx"
              class="message-row"
              :class="msg.role"
              :initial="{ opacity: 0, y: 16, scale: 0.98 }"
              :animate="{ opacity: 1, y: 0, scale: 1 }"
              :transition="{ duration: 0.25, delay: 0.04 }"
            >
              <div class="message-avatar">
                <template v-if="msg.role === 'ai'">
                  <div class="avatar-ai">
                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2a4 4 0 0 1 4 4v2a4 4 0 0 1-8 0V6a4 4 0 0 1 4-4z"/><path d="M12 14a8 8 0 0 0-8 8h16a8 8 0 0 0-8-8z"/></svg>
                  </div>
                </template>
                <template v-else>
                  <div class="avatar-user">
                    <svg xmlns="http://www.w3.org/2000/svg" class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="8" r="4"/><path d="M20 21a8 8 0 0 0-16 0"/></svg>
                  </div>
                </template>
              </div>
              <div class="message-body">
                <div v-if="msg.role === 'ai'" class="ai-msg">
                  <MarkdownRender
                    :content="msg.text"
                    :is-dark="true"
                    :max-live-nodes="0"
                    :typewriter="false"
                    :fade="false"
                    :final="!streaming || idx !== messages.length - 1"
                  />
                </div>
                <div v-else class="user-msg-text">{{ msg.text }}</div>
              </div>
            </motion.div>

            <!-- Streaming cursor -->
            <div v-if="streaming && messages.length > 0 && messages[messages.length - 1]?.role === 'ai'" class="stream-cursor">
              <span class="cursor-blink">▌</span>
            </div>
            </div><!-- /messages-column -->
          </div>

          <!-- Input -->
          <div class="input-area">
            <div class="input-column">
            <div class="input-wrapper">
              <input
                v-model="input"
                class="chat-input"
                placeholder="问任何短线问题... Enter 发送"
                @keydown="onKeydown"
                :disabled="streaming"
                ref="inputEl"
              />
              <div class="input-actions">
                <button
                  v-if="messages.length > 0"
                  @click="clearChat"
                  class="input-btn"
                  title="清空对话"
                >
                  <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
                </button>
                <button
                  @click="send()"
                  :disabled="streaming || !input.trim()"
                  class="send-btn"
                >
                  <svg v-if="!streaming" xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="22" y1="2" x2="11" y2="13"/><polygon points="22 2 15 22 11 13 2 9 22 2"/></svg>
                  <span v-else class="send-spinner" />
                </button>
              </div>
            </div>
            </div><!-- /input-column -->
          </div>
        </section>
      </main>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted, computed } from 'vue'
import MarkdownRender from 'markstream-vue'
import 'markstream-vue/index.css'
import { motion } from 'motion-v'
import { getApiBaseUrl, overview, fetchOverview, indexQuotes, fetchIndexQuotes } from '@/stores/data'

interface DisplayMessage {
  role: 'user' | 'ai'
  text: string
}

interface HistoryMessage {
  role: string
  content: string
}

interface AiRole {
  name: string
  display_name: string
}

const input = ref('')
const messages = ref<DisplayMessage[]>([])
const streaming = ref(false)
const msgContainer = ref<HTMLElement>()
const inputEl = ref<HTMLInputElement>()
const aiRoles = ref<AiRole[]>([])
const currentRole = ref('zarili')
const showRoleMenu = ref(false)

const currentRoleDisplay = computed(() => {
  const r = aiRoles.value.find(r => r.name === currentRole.value)
  return r ? r.display_name : currentRole.value
})

const suggestedQuestions = [
  '今天市场情绪怎么样？',
  '当前龙头是谁？',
  '今天哪些板块值得关注？',
  '今天适合短线操作吗？',
  '昨日涨停股今天表现如何？',
]

const tempColor = computed(() => {
  const t = overview.data?.market_temperature ?? 50
  if (t >= 70) return 'text-up'
  if (t <= 30) return 'text-down'
  return 'text-ink'
})

const riskClass = computed(() => {
  const r = overview.data?.risk_level ?? ''
  if (r.includes('高') || r.includes('High')) return 'risk-high'
  if (r.includes('低') || r.includes('Low')) return 'risk-low'
  return 'risk-mid'
})

onMounted(async () => {
  fetchOverview()
  fetchIndexQuotes()
  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/ai/roles`)
    if (resp.ok) aiRoles.value = await resp.json()
  } catch {}
})

function selectRole(name: string) {
  currentRole.value = name
  showRoleMenu.value = false
  if (messages.value.length > 0) clearChat()
}

function buildHistory(): HistoryMessage[] {
  return messages.value.map(m => ({
    role: m.role === 'ai' ? 'assistant' : 'user',
    content: m.text,
  }))
}

function clearChat() {
  messages.value = []
}

async function send(question?: string) {
  const text = (question || input.value).trim()
  if (!text || streaming.value) return

  input.value = ''
  const history = buildHistory()
  messages.value.push({ role: 'user', text })
  messages.value.push({ role: 'ai', text: '' })
  streaming.value = true
  scrollDown()

  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/ai/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: text, role: currentRole.value, history }),
    })

    if (!resp.ok) {
      updateLastMessage('[错误] AI 服务不可用，请检查 LLM_API_KEY 配置')
      streaming.value = false
      return
    }

    const reader = resp.body?.getReader()
    if (!reader) {
      updateLastMessage('[错误] 无法读取响应流')
      streaming.value = false
      return
    }

    const decoder = new TextDecoder()
    let buffer = ''

    while (true) {
      const { done, value } = await reader.read()
      if (done) break

      buffer += decoder.decode(value, { stream: true })
      const lines = buffer.split('\n')
      buffer = lines.pop() || ''

      for (const line of lines) {
        if (line.startsWith('data: ')) {
          const data = line.slice(6)
          if (data === '[DONE]') continue
          if (data.startsWith('[ERROR]')) {
            updateLastMessage(data)
          } else {
            appendToLastMessage(data.replace(/\\n/g, '\n'))
          }
        }
      }
    }
  } catch (e: any) {
    updateLastMessage(`[错误] ${e.message || '网络请求失败'}`)
  } finally {
    streaming.value = false
  }
}

function appendToLastMessage(text: string) {
  const last = messages.value[messages.value.length - 1]
  if (last && last.role === 'ai') {
    last.text += text
    scrollDown()
  }
}

function updateLastMessage(text: string) {
  const last = messages.value[messages.value.length - 1]
  if (last && last.role === 'ai') {
    last.text = text
  }
}

function scrollDown() {
  nextTick(() => {
    if (msgContainer.value) {
      msgContainer.value.scrollTop = msgContainer.value.scrollHeight
    }
  })
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter' && !e.shiftKey) {
    e.preventDefault()
    send()
  }
}
</script>

<style scoped>
/* ── Root & Background ─────────────────────────────── */
.copilot-root {
  position: fixed;
  inset: 0;
  top: 44px; /* below App header */
  background: #0d0d1a;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.bg-ambient {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(ellipse 80% 60% at 30% 50%, rgba(127,119,221,0.06) 0%, transparent 60%),
    radial-gradient(ellipse 60% 80% at 70% 40%, rgba(127,119,221,0.04) 0%, transparent 60%),
    radial-gradient(ellipse 50% 50% at 50% 80%, rgba(29,158,117,0.03) 0%, transparent 50%);
  animation: bg-breathe 12s ease-in-out infinite alternate;
  pointer-events: none;
}

@keyframes bg-breathe {
  0% { opacity: 0.7; }
  100% { opacity: 1; }
}

.bg-grid {
  position: absolute;
  inset: 0;
  background-image:
    radial-gradient(circle, rgba(255,255,255,0.03) 1px, transparent 1px);
  background-size: 32px 32px;
  pointer-events: none;
  mask-image: radial-gradient(ellipse 80% 60% at 30% 50%, black 30%, transparent 70%);
}

.bg-scanline {
  position: absolute;
  inset: 0;
  background: repeating-linear-gradient(
    0deg,
    transparent,
    transparent 2px,
    rgba(0,0,0,0.03) 2px,
    rgba(0,0,0,0.03) 4px
  );
  pointer-events: none;
  opacity: 0.4;
}

.page-content {
  position: relative;
  z-index: 1;
  display: flex;
  flex-direction: column;
  height: 100%;
}

/* ── Header ────────────────────────────────────────── */
.copilot-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 24px;
  height: 48px;
  border-bottom: 1px solid rgba(255,255,255,0.05);
  flex-shrink: 0;
  backdrop-filter: blur(8px);
}

.header-left {
  display: flex;
  align-items: center;
  gap: 12px;
}

.back-link {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: 8px;
  color: #888780;
  transition: all 0.2s;
}
.back-link:hover {
  color: #e8e8ed;
  background: rgba(255,255,255,0.06);
}

.header-title-group {
  display: flex;
  align-items: baseline;
  gap: 12px;
}

.header-title {
  font-family: 'JetBrains Mono', monospace;
  font-size: 13px;
  font-weight: 600;
  color: #e8e8ed;
  letter-spacing: 0.5px;
}

.ai-status {
  display: flex;
  align-items: center;
  gap: 6px;
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: #1D9E75;
  box-shadow: 0 0 6px rgba(29,158,117,0.5);
  animation: status-pulse 2s ease-in-out infinite;
}

@keyframes status-pulse {
  0%, 100% { box-shadow: 0 0 6px rgba(29,158,117,0.5); }
  50% { box-shadow: 0 0 12px rgba(29,158,117,0.8); }
}

.status-text {
  font-size: 11px;
  font-family: 'JetBrains Mono', monospace;
  color: #1D9E75;
  letter-spacing: 0.3px;
}

/* Role Selector */
.role-selector {
  position: relative;
}

.role-trigger {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 12px;
  border-radius: 8px;
  background: rgba(127,119,221,0.1);
  border: 1px solid rgba(127,119,221,0.15);
  color: #9990f0;
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
  cursor: pointer;
  transition: all 0.2s;
}
.role-trigger:hover {
  background: rgba(127,119,221,0.18);
  border-color: rgba(127,119,221,0.3);
}

.role-icon {
  display: flex;
  opacity: 0.7;
}

.chevron {
  opacity: 0.5;
  transition: transform 0.2s;
}

.role-menu {
  position: absolute;
  top: calc(100% + 6px);
  right: 0;
  min-width: 160px;
  background: #111124;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 12px;
  padding: 4px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.4);
  z-index: 50;
}

.role-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  width: 100%;
  padding: 8px 12px;
  border-radius: 8px;
  font-size: 13px;
  color: #ccc;
  cursor: pointer;
  transition: all 0.15s;
  border: none;
  background: none;
  font-family: inherit;
}
.role-option:hover {
  background: rgba(255,255,255,0.04);
  color: #fff;
}
.role-option.active {
  color: #9990f0;
  background: rgba(127,119,221,0.08);
}

/* ── Main Layout ───────────────────────────────────── */
.copilot-main {
  flex: 1;
  display: flex;
  overflow: hidden;
  min-height: 0;
}

/* ── Context Panel ─────────────────────────────────── */
.context-panel {
  width: 300px;
  flex-shrink: 0;
  border-right: 1px solid rgba(255,255,255,0.05);
  overflow-y: auto;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  background: rgba(17,17,36,0.3);
  backdrop-filter: blur(4px);
}

.context-section {
  background: rgba(17,17,36,0.6);
  border: 1px solid rgba(255,255,255,0.05);
  border-radius: 12px;
  padding: 14px;
}

.section-label {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 600;
  color: #888780;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  margin-bottom: 12px;
  font-family: 'JetBrains Mono', monospace;
}

.snapshot-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
}

.snap-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.snap-label {
  font-size: 11px;
  color: #666;
  font-family: 'JetBrains Mono', monospace;
}

.snap-value {
  font-size: 14px;
  font-family: 'JetBrains Mono', monospace;
  font-weight: 500;
  font-variant-numeric: tabular-nums;
  color: #ccc;
}
.snap-value em {
  font-style: normal;
  font-weight: 600;
}

.context-loading {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 0;
}

.loading-pulse {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: rgba(127,119,221,0.5);
  animation: pulse-dot 1.5s ease-in-out infinite;
}

@keyframes pulse-dot {
  0%, 100% { opacity: 0.3; transform: scale(0.8); }
  50% { opacity: 1; transform: scale(1.2); }
}

.risk-panel {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.risk-level-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.risk-badge {
  font-size: 11px;
  font-weight: 600;
  padding: 2px 10px;
  border-radius: 4px;
  font-family: 'JetBrains Mono', monospace;
  letter-spacing: 0.3px;
}
.risk-high {
  background: rgba(255,92,92,0.12);
  color: #FF5C5C;
}
.risk-mid {
  background: rgba(127,119,221,0.12);
  color: #9990f0;
}
.risk-low {
  background: rgba(29,158,117,0.12);
  color: #1D9E75;
}

.decision-text {
  font-size: 12px;
  line-height: 1.6;
  color: #999;
}

.index-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.index-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 0;
}

.index-name {
  font-size: 12px;
  color: #888;
  flex: 1;
  font-family: 'JetBrains Mono', monospace;
  font-size: 11px;
}

.index-price {
  font-size: 13px;
  font-family: 'JetBrains Mono', monospace;
  font-variant-numeric: tabular-nums;
  color: #ddd;
  text-align: right;
  width: 72px;
}

.index-change {
  font-size: 12px;
  font-family: 'JetBrains Mono', monospace;
  font-variant-numeric: tabular-nums;
  font-weight: 500;
  text-align: right;
  width: 64px;
}

/* ── Chat Panel ────────────────────────────────────── */
.chat-panel {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  background: rgba(13,13,26,0.4);
}

.messages-area {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
}

.messages-column {
  width: 100%;
  max-width: 720px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

/* Empty State */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  gap: 28px;
}

.empty-greeting {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.greeting-ring {
  position: relative;
  width: 64px;
  height: 64px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.ring-outer {
  position: absolute;
  inset: 0;
  border-radius: 50%;
  border: 1px solid rgba(127,119,221,0.2);
  animation: ring-spin 8s linear infinite;
}

.ring-inner {
  position: absolute;
  inset: 8px;
  border-radius: 50%;
  border: 1px solid rgba(127,119,221,0.15);
  animation: ring-spin 6s linear infinite reverse;
}

@keyframes ring-spin {
  to { transform: rotate(360deg); }
}

.greeting-emoji {
  font-size: 20px;
  color: #9990f0;
  position: relative;
  z-index: 1;
}

.greeting-title {
  font-size: 20px;
  font-weight: 600;
  color: #e8e8ed;
  letter-spacing: -0.3px;
}

.greeting-sub {
  font-size: 13px;
  color: #666;
  max-width: 360px;
  text-align: center;
  line-height: 1.6;
}

.suggestions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  justify-content: center;
  max-width: 500px;
}

.suggestion-chip {
  padding: 8px 16px;
  border-radius: 9999px;
  background: rgba(127,119,221,0.06);
  border: 1px solid rgba(127,119,221,0.12);
  color: #aaa;
  font-size: 13px;
  cursor: pointer;
  transition: all 0.2s;
  font-family: inherit;
}
.suggestion-chip:hover {
  color: #e8e8ed;
  background: rgba(127,119,221,0.12);
}

/* Message Rows */
.message-row {
  display: flex;
  gap: 10px;
  padding: 6px 0;
  max-width: 720px;
}
.message-row.user {
  align-self: flex-end;
  flex-direction: row-reverse;
}

.message-avatar {
  flex-shrink: 0;
  padding-top: 2px;
}

.avatar-ai {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: rgba(127,119,221,0.15);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #9990f0;
}

.avatar-user {
  width: 28px;
  height: 28px;
  border-radius: 8px;
  background: rgba(255,255,255,0.06);
  display: flex;
  align-items: center;
  justify-content: center;
  color: #888;
}

.message-body {
  min-width: 0;
}

.user-msg-text {
  padding: 10px 16px;
  background: rgba(127,119,221,0.08);
  border: 1px solid rgba(127,119,221,0.1);
  border-radius: 14px;
  border-bottom-right-radius: 4px;
  font-size: 13px;
  line-height: 1.6;
  color: #ddd;
}

.ai-msg {
  padding: 10px 16px;
  border-radius: 14px;
  border-bottom-left-radius: 4px;
  font-size: 13px;
  line-height: 1.6;
  color: #ccc;
}

.stream-cursor {
  padding: 2px 16px;
  max-width: 720px;
}

.cursor-blink {
  color: #9990f0;
  font-size: 14px;
  animation: blink 1s step-end infinite;
}

@keyframes blink {
  0%, 100% { opacity: 1; }
  50% { opacity: 0; }
}

/* ── Input Area ────────────────────────────────────── */
.input-area {
  padding: 16px 24px;
  border-top: 1px solid rgba(255,255,255,0.05);
  flex-shrink: 0;
  background: rgba(13,13,26,0.6);
  backdrop-filter: blur(8px);
}

.input-column {
  width: 100%;
  max-width: 720px;
  margin: 0 auto;
}

.input-wrapper {
  display: flex;
  align-items: center;
  gap: 8px;
  background: #111124;
  border: 1px solid rgba(255,255,255,0.08);
  border-radius: 14px;
  padding: 4px 4px 4px 16px;
  transition: border-color 0.2s;
}
.input-wrapper:focus-within {
  border-color: rgba(127,119,221,0.35);
  box-shadow: 0 0 0 3px rgba(127,119,221,0.06);
}

.chat-input {
  flex: 1;
  background: none;
  border: none;
  outline: none;
  color: #e8e8ed;
  font-size: 13px;
  font-family: 'JetBrains Mono', monospace;
  padding: 8px 0;
}
.chat-input::placeholder {
  color: #555;
}
.chat-input:disabled {
  opacity: 0.5;
}

.input-actions {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.input-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 10px;
  border: none;
  background: none;
  color: #666;
  cursor: pointer;
  transition: all 0.15s;
}
.input-btn:hover {
  color: #999;
  background: rgba(255,255,255,0.04);
}

.send-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: 10px;
  border: none;
  background: rgba(127,119,221,0.15);
  color: #9990f0;
  cursor: pointer;
  transition: all 0.15s;
}
.send-btn:hover:not(:disabled) {
  background: rgba(127,119,221,0.3);
  color: #b8b2ff;
}
.send-btn:disabled {
  opacity: 0.25;
  cursor: not-allowed;
}

.send-spinner {
  width: 16px;
  height: 16px;
  border: 2px solid rgba(127,119,221,0.2);
  border-top-color: #9990f0;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* ── Markdown overrides ────────────────────────────── */
.ai-msg :deep(.markstream-vue) {
  font-size: inherit;
  line-height: inherit;
}
.ai-msg :deep(.markstream-vue h1),
.ai-msg :deep(.markstream-vue h2) {
  font-size: 15px;
  font-weight: 700;
  color: #e8e8ed;
  margin: 10px 0 4px;
  border: none;
  padding: 0;
}
.ai-msg :deep(.markstream-vue h3) {
  font-size: 13px;
  font-weight: 700;
  color: #e8e8ed;
  margin: 8px 0 3px;
  border: none;
  padding: 0;
}
.ai-msg :deep(.markstream-vue p) {
  margin: 2px 0;
}
.ai-msg :deep(.markstream-vue ul) {
  list-style: none;
  padding-left: 0;
  margin: 4px 0;
}
.ai-msg :deep(.markstream-vue ul li)::before {
  content: '•';
  color: #7F77DD;
  margin-right: 6px;
}
.ai-msg :deep(.markstream-vue ol) {
  padding-left: 18px;
  margin: 4px 0;
}
.ai-msg :deep(.markstream-vue blockquote) {
  border-left: 2px solid #7F77DD;
  padding-left: 10px;
  margin: 6px 0;
  color: #aaa;
}
.ai-msg :deep(.markstream-vue hr) {
  border: none;
  border-top: 1px solid rgba(255,255,255,0.08);
  margin: 8px 0;
}
.ai-msg :deep(.markstream-vue a) {
  color: #7F77DD;
}
.ai-msg :deep(.markstream-vue table) {
  width: 100%;
  border-collapse: collapse;
  margin: 6px 0;
  font-size: 12px;
}
.ai-msg :deep(.markstream-vue th) {
  text-align: left;
  padding: 4px 8px;
  border-bottom: 1px solid rgba(255,255,255,0.15);
  color: #e8e8ed;
  font-weight: 600;
}
.ai-msg :deep(.markstream-vue td) {
  padding: 3px 8px;
  border-bottom: 1px solid rgba(255,255,255,0.06);
  color: #ccc;
}
.ai-msg :deep(.markstream-vue tbody tr:hover) {
  background: rgba(127,119,221,0.06);
}
</style>