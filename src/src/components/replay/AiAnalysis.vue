<template>
  <div
    class="fixed bottom-0 left-0 right-0 z-50 bg-surface-dark border-t border-hairline-dark transition-transform duration-300"
    :class="open ? 'translate-y-0' : 'translate-y-full'"
    style="max-height: 60vh"
  >
    <div class="flex items-center justify-between px-4 py-2 border-b border-hairline-dark">
      <span class="text-sm font-bold text-white">AI 行情分析</span>
      <div class="flex items-center gap-2">
        <span v-if="loading" class="text-xs text-white/40">分析中...</span>
        <button @click="$emit('close')" class="text-white/40 hover:text-white/60 text-lg leading-none">&times;</button>
      </div>
    </div>
    <div
      ref="contentRef"
      class="overflow-y-auto custom-scroll p-4 text-sm text-white/80 leading-relaxed"
      style="max-height: calc(60vh - 40px)"
      v-html="rendered"
    ></div>
  </div>
</template>

<script setup lang="ts">
import { computed, ref, watch, nextTick } from 'vue'
import { marked } from 'marked'

const props = defineProps<{
  open: boolean
  loading: boolean
  text: string
}>()

defineEmits<{
  close: []
}>()

const contentRef = ref<HTMLElement>()

const rendered = computed(() => {
  if (!props.text) return '<span class="text-white/30">等待分析结果...</span>'
  return marked.parse(props.text) as string
})

watch(
  () => props.text,
  async () => {
    await nextTick()
    if (contentRef.value) {
      contentRef.value.scrollTop = contentRef.value.scrollHeight
    }
  },
)
</script>

<style>
.custom-scroll :deep(h2) {
  font-size: 1.1em; font-weight: 700; color: #fff;
  margin: 1em 0 0.4em; padding-bottom: 0.3em;
  border-bottom: 1px solid rgba(255,255,255,0.08);
}
.custom-scroll :deep(h3) {
  font-size: 1em; font-weight: 600; color: rgba(255,255,255,0.9);
  margin: 0.8em 0 0.3em;
}
.custom-scroll :deep(h4) {
  font-size: 0.9em; font-weight: 600; color: rgba(255,255,255,0.7);
  margin: 0.6em 0 0.2em;
}
.custom-scroll :deep(strong) { color: #fff; }
.custom-scroll :deep(ul) { padding-left: 1.2em; margin: 0.3em 0; }
.custom-scroll :deep(li) { margin: 0.15em 0; }
.custom-scroll :deep(p) { margin: 0.3em 0; }
.custom-scroll :deep(table) {
  width: 100%; border-collapse: collapse; margin: 0.5em 0;
  font-size: 0.85em;
}
.custom-scroll :deep(th) {
  text-align: left; padding: 0.3em 0.6em;
  border-bottom: 1px solid rgba(255,255,255,0.15);
  color: rgba(255,255,255,0.6); font-weight: 600;
}
.custom-scroll :deep(td) {
  padding: 0.25em 0.6em;
  border-bottom: 1px solid rgba(255,255,255,0.05);
}
.custom-scroll :deep(code) {
  background: rgba(255,255,255,0.06); padding: 0.1em 0.3em;
  border-radius: 3px; font-size: 0.9em;
}
.custom-scroll :deep(hr) {
  border: none; border-top: 1px solid rgba(255,255,255,0.08);
  margin: 0.8em 0;
}
</style>
