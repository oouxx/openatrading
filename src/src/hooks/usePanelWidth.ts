// src/hooks/usePanelWidth.ts
import { useState, useCallback, useEffect } from "react"

export function usePanelWidth(
  storageKey: string,
  defaultWidth: number,
  min: number,
  max: number,
  collapsedKey: string
) {
  const [width, setWidthRaw] = useState(() => {
    try {
      return parseInt(localStorage.getItem(storageKey) || String(defaultWidth), 10)
    } catch {
      return defaultWidth
    }
  })
  const [collapsed, setCollapsed] = useState(() => {
    try {
      return localStorage.getItem(collapsedKey) === "true"
    } catch {
      return false
    }
  })

  useEffect(() => {
    localStorage.setItem(storageKey, String(width))
  }, [width, storageKey])
  useEffect(() => {
    localStorage.setItem(collapsedKey, String(collapsed))
  }, [collapsed, collapsedKey])

  const setWidth = useCallback(
    (fn: number | ((prev: number) => number)) => {
      setWidthRaw((prev) => Math.max(min, Math.min(max, typeof fn === "function" ? fn(prev) : fn)))
    },
    [min, max]
  )

  const toggleCollapse = useCallback(() => setCollapsed((c) => !c), [])

  return {
    width,
    setWidth,
    collapsed,
    toggleCollapse,
    displayWidth: collapsed ? 0 : width,
    min,
    max,
  }
}
