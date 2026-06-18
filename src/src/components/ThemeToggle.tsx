import { useState, useEffect } from 'react'

const STORAGE_KEY = 'openatrading-theme'

function getInitialTheme(): 'dark' | 'light' {
  if (typeof window === 'undefined') return 'light'
  const saved = localStorage.getItem(STORAGE_KEY)
  if (saved === 'light' || saved === 'dark') return saved
  return 'light'
}

function applyTheme(theme: 'dark' | 'light') {
  if (theme === 'dark') {
    document.documentElement.setAttribute('data-theme', 'dark');
    document.documentElement.classList.add('dark');
  } else {
    document.documentElement.removeAttribute('data-theme');
    document.documentElement.classList.remove('dark');
  }
}

export default function ThemeToggle() {
  const [theme, setTheme] = useState<'dark' | 'light'>(getInitialTheme)

  useEffect(() => {
    applyTheme(theme)
  }, [theme])

  const toggle = () => {
    setTheme((prev) => {
      const next = prev === 'dark' ? 'light' : 'dark'
      localStorage.setItem(STORAGE_KEY, next)
      return next
    })
  }

  const isLight = theme === 'light'

  return (
    <button
      onClick={toggle}
      title={isLight ? '切换暗色模式' : '切换亮色模式'}
      aria-label="切换主题"
      className="w-12 h-[26px] rounded-[13px] border border-hairline cursor-pointer relative flex-shrink-0 bg-ink-dim"
    >
      <span
        className="absolute top-[3px] w-[18px] h-[18px] rounded-full flex items-center justify-center text-[10px] bg-ai transition-transform duration-300"
        style={{ transform: isLight ? 'translateX(22px)' : 'translateX(3px)' }}
      >
        {isLight ? '☀️' : '🌙'}
      </span>
    </button>
  )
}
