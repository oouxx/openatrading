<template>
  <div class="h-full flex flex-col">
    <!-- Messages -->
    <div ref="msgContainer" class="flex-1 overflow-y-auto p-4 space-y-3">
      <div v-if="messages.length === 0" class="text-center py-8">
        <p class="text-caption text-ink-muted mb-3">AI 交易副驾驶</p>

        <div class="flex justify-center mb-4">
          <div class="relative">
            <button
              @click="showRoleDropdown = !showRoleDropdown"
              class="flex items-center gap-2 px-3 py-1.5 rounded-card bg-bg-hover text-caption text-white hover:bg-bg-hover/80 transition-colors cursor-pointer"
            >
              <span class="text-ai">{{ currentRoleDisplay }}</span>
              <svg xmlns="http://www.w3.org/2000/svg" class="w-3 h-3 text-ink-muted" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="m6 9 6 6 6-6"/></svg>
            </button>
            <div
              v-if="showRoleDropdown"
              class="absolute top-full left-1/2 -translate-x-1/2 mt-1 bg-bg border border-hairline rounded-card shadow-lg z-10 min-w-[140px] py-1"
            >
              <button
                v-for="r in aiRoles"
                :key="r.name"
                @click="selectRole(r.name)"
                class="w-full text-left px-3 py-1.5 text-caption hover:bg-bg-hover transition-colors cursor-pointer"
                :class="r.name === currentRole ? 'text-ai' : 'text-white'"
              >
                {{ r.display_name }}
              </button>
            </div>
          </div>
        </div>

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
        @click="showRoleDropdown = !showRoleDropdown"
        class="px-2 py-2 bg-bg-hover text-ai rounded-button text-caption hover:bg-bg-hover/80 transition-colors cursor-pointer flex-shrink-0 text-xs font-mono"
        :title="'当前角色: ' + currentRoleDisplay"
      >
        {{ currentRoleDisplay }}
      </button>
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

    <!-- Role dropdown (bottom) -->
    <div
      v-if="showRoleDropdown && messages.length > 0"
      class="absolute bottom-16 left-16 right-16 bg-bg border border-hairline rounded-card shadow-lg z-10 py-1"
    >
      <button
        v-for="r in aiRoles"
        :key="r.name"
        @click="selectRole(r.name)"
        class="w-full text-left px-3 py-1.5 text-caption hover:bg-bg-hover transition-colors cursor-pointer"
        :class="r.name === currentRole ? 'text-ai' : 'text-white'"
      >
        {{ r.display_name }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, nextTick, onMounted, computed } from 'vue'
import MarkdownRender from 'markstream-vue'
import 'markstream-vue/index.css'
import { motion } from 'motion-v'
import { apiFetch, apiChatStream } from '@/stores/data'

defineProps<{
  overview?: any
}>()

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
const aiRoles = ref<AiRole[]>([])
const currentRole = ref('zarili')
const showRoleDropdown = ref(false)

const currentRoleDisplay = computed(() => {
  const r = aiRoles.value.find(r => r.name === currentRole.value)
  return r ? r.display_name : currentRole.value
})

const suggestedQuestions = [
  '今天市场情绪怎么样？',
  '当前龙头是谁？',
  '涨停板块有哪些？',
  '今天适合短线操作吗？',
]

onMounted(async () => {
  try {
    const resp = await apiFetch('/api/ai/roles')
    if (resp.ok) {
      aiRoles.value = await resp.json()
    }
  } catch {}
})

function selectRole(name: string) {
  currentRole.value = name
  showRoleDropdown.value = false
  if (messages.value.length > 0) {
    clearChat()
  }
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
    await apiChatStream(
      { message: text, role: currentRole.value, history },
      {
        onData: (data) => appendToLastMessage(data),
        onDone: () => { streaming.value = false },
        onError: (err) => updateLastMessage(`[错误] ${err}`),
      },
    )
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
.ai-msg {
  font-size: 13px;
  line-height: 1.6;
}
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
.ai-msg :deep(.markstream-vue h4) {
  font-size: 12px;
  font-weight: 600;
  color: #ccc;
  margin: 6px 0 2px;
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
.ai-msg :deep(.markstream-vue ol li) {
  margin: 1px 0;
}
.ai-msg :deep(.markstream-vue blockquote) {
  border-left: 2px solid #7F77DD;
  padding-left: 10px;
  margin: 6px 0;
  color: #aaa;
}
.ai-msg :deep(.markstream-vue hr) {
  border: none;
  border-top: 1px solid rgba(255, 255, 255, 0.08);
  margin: 8px 0;
}
.ai-msg :deep(.markstream-vue a) {
  color: #7F77DD;
}
.ai-msg :deep(.markstream-vue a:hover) {
  text-decoration: underline;
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
  border-bottom: 1px solid rgba(255, 255, 255, 0.15);
  color: #e8e8ed;
  font-weight: 600;
}
.ai-msg :deep(.markstream-vue td) {
  padding: 3px 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  color: #ccc;
}
.ai-msg :deep(.markstream-vue tbody tr:hover) {
  background: rgba(127, 119, 221, 0.06);
}
</style>