<template>
  <div class="h-full flex flex-col">
    <!-- Messages -->
    <div ref="msgContainer" class="flex-1 overflow-y-auto p-4 space-y-3">
      <div v-if="messages.length === 0" class="text-center py-8">
        <p class="text-caption text-ink-muted mb-3">AI 交易副驾驶</p>
        <p class="text-caption text-ink-muted text-sm">结合实时行情数据，回答短线市场问题</p>
        <div class="mt-4 space-y-2">
          <motion.button
            v-for="(q, qi) in suggestedQuestions"
            :key="q"
            @click="send(q)"
            class="w-full text-left px-3 py-2 rounded-card bg-bg-hover text-caption text-ink-muted hover:text-white transition-colors cursor-pointer"
            :initial="{ opacity: 0, x: -10 }"
            :animate="{ opacity: 1, x: 0 }"
            :transition="{ duration: 0.25, delay: qi * 0.06 }"
            :whileHover="{ x: 4 }"
          >
            {{ q }}
          </motion.button>
        </div>
      </div>

      <motion.div
        v-for="(msg, idx) in messages"
        :key="idx"
        class="px-3 py-2 rounded-card text-caption"
        :class="msg.role === 'user'
          ? 'bg-ai/15 text-white ml-8'
          : 'bg-bg border border-hairline text-white/80 mr-4'"
        :initial="{ opacity: 0, y: 8 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.2, delay: idx * 0.02 }"
      >
        <div v-if="msg.role === 'ai'" class="md-body" v-html="msg.html"></div>
        <div v-else>{{ msg.text }}</div>
      </motion.div>

      <!-- Streaming indicator -->
      <div v-if="streaming" class="px-3 py-2 rounded-card text-caption bg-bg border border-hairline text-ai mr-4">
        <span class="animate-pulse">▌</span>
      </div>
    </div>

    <!-- Input -->
    <div class="flex gap-2 p-3 border-t border-hairline flex-shrink-0">
      <button
        v-if="messages.length > 0"
        @click="clearChat"
        class="px-2 py-2 bg-bg-hover text-ink-muted rounded-button text-caption hover:text-white transition-colors cursor-pointer flex-shrink-0"
        title="清空对话"
      >
        <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/></svg>
      </button>
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
import { Marked } from 'marked'
import hljs from 'highlight.js'
import { motion } from 'motion-v'
import { getApiBaseUrl } from '@/stores/data'

const marked = new Marked({
  breaks: true,
  gfm: true,
  renderer: {
    heading({ tokens, depth }: { tokens: any[]; depth: number }) {
      const text = this.parser.parseInline(tokens)
      const cls = depth <= 1 ? 'md-h2' : depth === 2 ? 'md-h3' : 'md-h4'
      return `<h${depth + 1} class="${cls}">${text}</h${depth + 1}>`
    },
    paragraph({ tokens }: { tokens: any[] }) {
      return `<p class="md-p">${this.parser.parseInline(tokens)}</p>`
    },
    list({ items, ordered }: { items: any[]; ordered: boolean }) {
      const body = items.map(item => this.listitem(item)).join('')
      return ordered ? `<ol class="md-ol">${body}</ol>` : `<ul class="md-ul">${body}</ul>`
    },
    listitem(item: any) {
      return `<li>${this.parser.parse(item.tokens)}</li>`
    },
    blockquote({ tokens }: { tokens: any[] }) {
      return `<blockquote class="md-blockquote">${this.parser.parse(tokens)}</blockquote>`
    },
    code({ text, lang }: { text: string; lang?: string }) {
      let highlighted: string
      if (lang && hljs.getLanguage(lang)) {
        highlighted = hljs.highlight(text, { language: lang }).value
      } else {
        highlighted = hljs.highlightAuto(text).value
      }
      return `<pre class="md-code-block"><code class="hljs">${highlighted}</code></pre>`
    },
    codespan({ text }: { text: string }) {
      return `<code class="md-inline-code">${text}</code>`
    },
    hr() {
      return '<hr class="md-hr">'
    },
    strong({ tokens }: { tokens: any[] }) {
      return `<strong class="text-white">${this.parser.parseInline(tokens)}</strong>`
    },
    em({ tokens }: { tokens: any[] }) {
      return `<em class="md-em">${this.parser.parseInline(tokens)}</em>`
    },
    del({ tokens }: { tokens: any[] }) {
      return `<del class="md-del">${this.parser.parseInline(tokens)}</del>`
    },
    link({ href, tokens }: { href: string; tokens: any[] }) {
      return `<a href="${href}" target="_blank" class="md-link">${this.parser.parseInline(tokens)}</a>`
    },
    image({ href, title, text }: { href: string; title: string | null; text: string }) {
      return `<img src="${href}" alt="${text}"${title ? ` title="${title}"` : ''} class="md-image">`
    },
    checkbox({ checked }: { checked: boolean }) {
      return `<input type="checkbox" ${checked ? 'checked' : ''} disabled class="md-checkbox">`
    },
  },
})

defineProps<{
  overview?: any
}>()

interface DisplayMessage {
  role: 'user' | 'ai'
  text: string
  html: string
}

interface HistoryMessage {
  role: string
  content: string
}

const input = ref('')
const messages = ref<DisplayMessage[]>([])
const streaming = ref(false)
const msgContainer = ref<HTMLElement>()

const suggestedQuestions = [
  '今天市场情绪怎么样？',
  '当前龙头是谁？',
  '涨停板块有哪些？',
  '今天适合短线操作吗？',
]

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
  messages.value.push({ role: 'user', text, html: '' })
  messages.value.push({ role: 'ai', text: '', html: '' })
  streaming.value = true
  scrollDown()

  try {
    const resp = await fetch(`${getApiBaseUrl()}/api/ai/chat`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ message: text, history }),
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
    last.html = renderMarkdown(last.text)
    scrollDown()
  }
}

function updateLastMessage(text: string) {
  const last = messages.value[messages.value.length - 1]
  if (last && last.role === 'ai') {
    last.text = text
    last.html = text.startsWith('[') ? text : renderMarkdown(text)
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
  return marked.parse(text) as string
}
</script>

<style scoped>
.md-body :deep(.md-h2) {
  font-size: 15px;
  font-weight: 700;
  color: #e8e8ed;
  margin: 10px 0 4px;
}
.md-body :deep(.md-h3) {
  font-size: 13px;
  font-weight: 700;
  color: #e8e8ed;
  margin: 8px 0 3px;
}
.md-body :deep(.md-h4) {
  font-size: 12px;
  font-weight: 600;
  color: #ccc;
  margin: 6px 0 2px;
}
.md-body :deep(.md-p) {
  margin: 2px 0;
  line-height: 1.6;
}
.md-body :deep(.md-ul) {
  list-style: none;
  padding-left: 0;
  margin: 4px 0;
}
.md-body :deep(.md-ul li)::before {
  content: '•';
  color: #7F77DD;
  margin-right: 6px;
}
.md-body :deep(.md-ol) {
  padding-left: 18px;
  margin: 4px 0;
}
.md-body :deep(.md-ol li) {
  margin: 1px 0;
}
.md-body :deep(.md-blockquote) {
  border-left: 2px solid #7F77DD;
  padding-left: 10px;
  margin: 6px 0;
  color: #aaa;
}
.md-body :deep(.md-code-block) {
  background: rgba(0, 0, 0, 0.4);
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 6px;
  padding: 10px 12px;
  margin: 6px 0;
  overflow-x: auto;
  font-size: 12px;
  line-height: 1.5;
}
.md-body :deep(.md-code-block code) {
  font-family: 'JetBrains Mono', monospace;
}
.md-body :deep(.md-inline-code) {
  background: rgba(127, 119, 221, 0.15);
  color: #b0a8e8;
  padding: 1px 5px;
  border-radius: 3px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 12px;
}
.md-body :deep(.md-hr) {
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  margin: 8px 0;
}
.md-body :deep(.md-em) {
  color: #bbb;
}
.md-body :deep(.md-del) {
  color: #888780;
}
.md-body :deep(.md-link) {
  color: #7F77DD;
}
.md-body :deep(.md-link:hover) {
  text-decoration: underline;
}
</style>