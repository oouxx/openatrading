import { Routes, Route, NavLink } from 'react-router-dom'
import LandingPage from './LandingPage'
import TradingView from './TradingView'
import RoleView from './RoleView'
import AiCopilot from './AiCopilot'
import ReplayView from './ReplayView'

const isTauri = '__TAURI_INTERNALS__' in window

export default function App() {
  return (
    <div className="min-h-screen bg-bg">
      <header className="fixed top-0 w-full z-50 bg-black h-11">
        <div className="max-w-[1600px] mx-auto px-6 h-full flex items-center justify-between">
          <div className="flex items-center gap-6 flex-shrink-0">
            {isTauri ? (
              <span className="text-nav-link font-semibold text-white tracking-tight">
                OpenATrading
              </span>
            ) : (
              <NavLink
                to="/"
                className="text-nav-link font-semibold text-white tracking-tight hover:text-ai transition-colors"
              >
                OpenATrading
              </NavLink>
            )}
            <NavLink
              to="/trading"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? '!text-white' : ''}`
              }
            >
              交易终端
            </NavLink>
            <NavLink
              to="/replay"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? '!text-white' : ''}`
              }
            >
              行情回放
            </NavLink>
            <NavLink
              to="/roles"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? '!text-white' : ''}`
              }
            >
              角色策略
            </NavLink>
            <NavLink
              to="/copilot"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? '!text-white' : ''}`
              }
            >
              AI 副驾驶
            </NavLink>
          </div>
        </div>
      </header>

      <Routes>
        <Route path="/" element={isTauri ? <TradingView /> : <LandingPage />} />
        <Route path="/trading" element={<TradingView />} />
        <Route path="/roles" element={<RoleView />} />
        <Route path="/copilot" element={<AiCopilot />} />
        <Route path="/replay" element={<ReplayView />} />
      </Routes>
    </div>
  )
}