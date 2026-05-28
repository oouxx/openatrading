import { useState, useRef, useEffect, useCallback } from 'react'

interface MiniChartPreviewProps {
  symbol: string
  name: string
  children: React.ReactNode
}

function normalizeSymbol(raw: string): string {
  const dot = raw.indexOf('.')
  if (dot !== -1) {
    const code = raw.substring(0, dot)
    let suffix = raw.substring(dot + 1).toLowerCase()
    if (suffix === 'ss') suffix = 'sh'
    return suffix + code
  }
  return (raw.startsWith('6') ? 'sh' : 'sz') + raw
}

export default function MiniChartPreview({ symbol, name, children }: MiniChartPreviewProps) {
  const [visible, setVisible] = useState(false)
  const [enlarged, setEnlarged] = useState(false)
  const [scale, setScale] = useState(1)
  const [pos, setPos] = useState({ x: 0, y: 0 })
  const triggerRef = useRef<HTMLSpanElement>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const baseWidth = 240
  const baseHeight = 180

  const gifUrl = `https://image.sinajs.cn/newchart/min/n/${normalizeSymbol(symbol)}.gif`

  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const updatePosition = useCallback(() => {
    if (!triggerRef.current) return
    const rect = triggerRef.current.getBoundingClientRect()
    const cardW = 256
    const cardH = 228
    let x = rect.right + 12
    let y = rect.top - 24
    if (x + cardW > window.innerWidth) x = rect.left - cardW - 12
    if (y + cardH > window.innerHeight) y = window.innerHeight - cardH - 8
    if (y < 8) y = 8
    setPos({ x, y })
  }, [])

  const onEnter = useCallback(() => {
    clearTimer()
    updatePosition()
    setVisible(true)
  }, [clearTimer, updatePosition])

  const onLeave = useCallback(() => {
    timerRef.current = setTimeout(() => setVisible(false), 200)
  }, [])

  useEffect(() => {
    if (!enlarged) return
    const onKeydown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setEnlarged(false)
    }
    document.addEventListener('keydown', onKeydown)
    return () => document.removeEventListener('keydown', onKeydown)
  }, [enlarged])

  useEffect(() => {
    if (enlarged) {
      const w = (window.innerWidth / 3) / baseWidth
      const h = (window.innerHeight / 3) / baseHeight
      setScale(Math.min(w, h))
    }
  }, [enlarged])

  return (
    <>
      <span ref={triggerRef} className="inline-block" onMouseEnter={onEnter} onMouseLeave={onLeave}>
        {children}
      </span>

      {visible && (
        <div
          className="fixed z-[60] bg-bg-surface rounded-card border border-hairline p-2"
          style={{ top: pos.y + 'px', left: pos.x + 'px' }}
          onMouseEnter={clearTimer}
          onMouseLeave={onLeave}
        >
          <img
            src={gifUrl}
            width={baseWidth}
            height={baseHeight}
            className="rounded-[8px] block cursor-pointer"
            alt=""
            onClick={(e) => {
              e.stopPropagation()
              setEnlarged(true)
            }}
          />
          <p className="text-caption font-mono text-center text-ink-muted mt-1">
            {symbol} {name}
          </p>
        </div>
      )}

      {enlarged && (
        <div
          className="fixed inset-0 z-[70] flex items-center justify-center"
          onClick={() => setEnlarged(false)}
        >
          <div
            className="bg-bg-surface rounded-card border border-hairline p-4"
            onClick={(e) => e.stopPropagation()}
            onWheel={(e) => {
              const step = 0.5
              setScale((s) => Math.max(0.5, Math.min(8, s + (e.deltaY < 0 ? step : -step))))
            }}
          >
            <img
              src={gifUrl}
              style={{ width: baseWidth * scale + 'px', height: baseHeight * scale + 'px' }}
              className="rounded-[8px] block"
              alt=""
            />
            <p className="text-caption font-mono text-center text-ink-muted mt-2">
              {symbol} {name} · {scale.toFixed(2)}x 滚轮缩放
            </p>
          </div>
        </div>
      )}
    </>
  )
}