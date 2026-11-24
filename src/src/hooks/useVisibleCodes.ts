import { useCallback, useEffect, useRef, useState } from "react"

/**
 * 只要"看得见的那几十行"的代码。
 *
 * 榜单这类长表格现在整表定时重拉，一屏几十行其实只有可视区在变；把订阅范围收到
 * 可视区（外加一小段预取），上游和带宽就只跟"你在看多少行"有关。
 *
 * 不用虚拟列表：行数在几十到一百多，逐行 `IntersectionObserver` 比引一个虚拟化
 * 库改动小得多，也不会把表格的既有交互（点行、横向滚动）打散。
 *
 * 返回的是"可视区覆盖到的行号范围"，调用方按块（比如每 30 行）取整再订阅：
 * 订阅集合只在跨块时变化，滚动过程中不会每 200ms 重连一次 SSE。
 */

/** 滚动时合并多次可见性变化，避免每一帧都触发订阅重建。 */
const FLUSH_DELAY_MS = 200

export function useVisibleRows({
  rootMargin = "240px 0px",
}: {
  /** 预取范围：视口上下各多算这么远，滚起来不用等一帧。 */
  rootMargin?: string
} = {}) {
  /** 可视（含预取）行的序号区间；`null` 表示一行都还没看到。 */
  const [range, setRange] = useState<{ min: number; max: number } | null>(null)

  /** 当前可视的行元素。存元素而不是序号：快照刷新会让行号变化，元素不变。 */
  const visibleRef = useRef(new Set<HTMLElement>())
  const elementsRef = useRef(new Map<string, { el: HTMLElement; index: number }>())
  const refCacheRef = useRef(new Map<string, (el: HTMLElement | null) => void>())
  const observerRef = useRef<IntersectionObserver | null>(null)
  const flushTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null)

  const flush = useCallback(() => {
    flushTimerRef.current = null
    // 序号在 flush 时才读：快照刷新会重排，行号跟着变，元素本身不变
    const indexes: number[] = []
    for (const el of visibleRef.current) {
      const raw = el.dataset.index
      if (raw === undefined) continue
      const index = Number(raw)
      if (Number.isFinite(index)) indexes.push(index)
    }
    if (indexes.length === 0) {
      setRange((prev) => (prev === null ? prev : null))
      return
    }
    const min = Math.min(...indexes)
    const max = Math.max(...indexes)
    setRange((prev) => (prev && prev.min === min && prev.max === max ? prev : { min, max }))
  }, [])

  const scheduleFlush = useCallback(() => {
    if (flushTimerRef.current !== null) return
    flushTimerRef.current = setTimeout(flush, FLUSH_DELAY_MS)
  }, [flush])

  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        let changed = false
        for (const entry of entries) {
          const el = entry.target as HTMLElement
          if (el.dataset.index === undefined) continue
          if (entry.isIntersecting) {
            if (!visibleRef.current.has(el)) {
              visibleRef.current.add(el)
              changed = true
            }
          } else if (visibleRef.current.delete(el)) {
            changed = true
          }
        }
        if (changed) scheduleFlush()
      },
      { rootMargin }
    )
    observerRef.current = observer
    for (const { el } of elementsRef.current.values()) observer.observe(el)
    return () => {
      observer.disconnect()
      observerRef.current = null
      if (flushTimerRef.current !== null) clearTimeout(flushTimerRef.current)
      flushTimerRef.current = null
    }
  }, [rootMargin, scheduleFlush])

  /**
   * 挂到每一行：`ref={rowRef(index, code)}`。
   *
   * 同一个 key 必须拿到**同一个** ref 函数，否则每次渲染 React 都会先 null 再
   * 赋元素，`IntersectionObserver` 跟着 un/observe 抖一遍。
   */
  const rowRef = useCallback(
    (index: number, code: string) => {
      const key = `${index}:${code}`
      const cached = refCacheRef.current.get(key)
      if (cached) return cached
      const fn = (el: HTMLElement | null) => {
        const observer = observerRef.current
        const prev = elementsRef.current.get(key)
        if (prev?.el === el) return
        if (prev) {
          observer?.unobserve(prev.el)
          elementsRef.current.delete(key)
          if (visibleRef.current.delete(prev.el)) scheduleFlush()
        }
        if (!el) return
        el.dataset.index = String(index)
        el.dataset.code = code
        elementsRef.current.set(key, { el, index })
        observer?.observe(el)
      }
      refCacheRef.current.set(key, fn)
      return fn
    },
    [scheduleFlush]
  )

  return { range, rowRef }
}
