<template>
  <div class="h-full flex flex-col">
    <!-- Messages -->
    <div ref="msgContainer" class="flex-1 overflow-y-auto p-4 space-y-3">
      <div v-if="messages.length === 0" class="text-center py-8">
        <p class="text-caption text-ink-muted mb-3">AI 交易副驾驶</p>
        <p class="text-caption text-ink-muted text-sm">结合实时行情数据，回答短线市场问题</p>
        <div class="mt-4 space-y-2">
          <button
            v-for="q in suggestedQuestions"
            :key="q"
            @click="send(q)"
            class="w-full text-left px-3 py-2 rounded-card bg-bg-hover text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
          >
            {{ q }}
          </button>
        </div>
      </div>

      <div
        v-for="(msg, idx) in messages"
        :key="idx"
        class="px-3 py-2 rounded-card text-caption"
        :class="msg.role === 'user'
          ? 'bg-ai/15 text-white ml-8'
          : 'bg-bg border border-hairline text-white/80 mr-4'"
      >
        <div v-if="msg.role === 'ai'" v-html="renderMarkdown(msg.text)"></div>
        <div v-else>{{ msg.text }}</div>
      </div>

      <!-- Streaming indicator -->
      <div v-if="streaming" class="px-3 py-2 rounded-card text-caption bg-bg border border-hairline text-ai mr-4">
        <span class="animate-pulse">▌</span>
      </div>
    </div>

    <!-- Input -->
    <div class="flex gap-2 p-3 border-t border-hairline flex-shrink-0">
      <input
        v-model="input"
        class="flex-1 bg-bg border border-hairline rounded-input px-3 py-2 text-caption text-white placeholder-ink-muted outline-none focus:border-ai/50 transition-colors font-mono"
        placeholder="问任何短线问题..."
        @keydown="onKeydown"
        :disabled="streaming"
      />
      <button
        @click="send()"
        :disabled="streaming || !input.trim()"
        class="px-4 py-2 bg-ai/20 text-ai rounded-button text-caption font-mono hover:bg-ai/30 transition-colors cursor-pointer disabled:opacity-30 disabled:cursor-not-allowed"
      >
        发送
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick } from 'vue'

defineProps<{
  overview?: any
}>()

const input = ref('')
const messages = ref<{ role: 'user' | 'ai'; text: string }[]>([])
const streaming = ref(false)
const msgContainer = ref<HTMLElement>()

const suggestedQuestions = [
  '今天市场情绪怎么样？',
  '当前龙头是谁？',
  '涨停板块有哪些？',
  '今天适合短线操作吗？',
]

function getApiBase(): string {
  return localStorage.getItem('trading-agent-api-url') || (window as any).__API_BASE_URL__ || ''
}

async function send(question?: string) {
  const text = (question || input.value).trim()
  if (!text || streaming.value) return

  input.value = ''
  messages.value.push({ role: 'user', text })
  messages.value.push({ role: 'ai', text: '' })
  streaming.value = true
  scrollDown()

  try {
    const resp = await fetch(`${getApiBase()}/api/ai/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: text }),
    })

    if (!resp.ok) {
      updateLastMessage('[错误] AI 服务不可用，请检查 OPENROUTER_API_KEY 配置')
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
            appendToLastMessage(data)
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

function renderMarkdown(text: string): string {
  return text
    .replace(/\*\*(.+?)\*\*/g, '<strong class="text-white">$1</strong>')
    .replace(/^- (.+)$/gm, '<span class="text-ai">•</span> $1')
    .replace(/\n/g, '<br>')
}
</script>
