import { useState } from 'react'

export function useSpeech() {
  const [muted, setMuted] = useState(false)

  function speak(text: string) {
    if (muted) return
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
    setMuted((m) => {
      if (!m) stop()
      return !m
    })
  }

  return { muted, speak, stop, toggleMute }
}