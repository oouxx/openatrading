import { ref } from 'vue'

export function useSpeech() {
  const muted = ref(false)

  function speak(text: string) {
    if (muted.value) return
    if (!window.speechSynthesis) return
    const utter = new SpeechSynthesisUtterance(text)
    utter.lang = 'zh-CN'
    utter.rate = 1.2
    speechSynthesis.cancel()
    speechSynthesis.speak(utter)
  }

  function stop() {
    speechSynthesis?.cancel()
  }

  function toggleMute() {
    muted.value = !muted.value
    if (muted.value) stop()
  }

  return { muted, speak, stop, toggleMute }
}