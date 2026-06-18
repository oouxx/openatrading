import { Routes, Route, NavLink, Navigate } from "react-router-dom";
import LandingPage from "./LandingPage";
import TradingView from "./TradingView";
import RoleView from "./RoleView";
import AiCopilot from "./AiCopilot";
import ReplayView from "./ReplayView";
import BreakoutView from "./BreakoutView";
import DragonTigerView from "./DragonTigerView";
import ThemeToggle from "./components/ThemeToggle";
import { useIndexQuotes } from "@/hooks/query/useIndexQuotes";

const isTauri = "__TAURI_INTERNALS__" in window;

function IndexTicker() {
  const { data: quotes = [] } = useIndexQuotes();
  if (!quotes.length) return null;
  return (
    <div className="hidden md:flex items-center gap-3 overflow-x-auto text-[11px] font-mono text-ink-muted">
      {quotes.map((q) => {
        const up = q.px_change_rate >= 0;
        return (
          <span key={q.symbol} className="flex items-center gap-1 whitespace-nowrap">
            <span className="text-white text-[11px]">{q.name.replace("指数", "")}</span>
            <span className="text-white text-[11px]">
              {q.last_px.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 })}
            </span>
            <span className={up ? "text-up" : "text-down"}>
              {up ? "+" : ""}{q.px_change_rate.toFixed(2)}%
            </span>
          </span>
        );
      })}
    </div>
  );
}

export default function App() {
  return (
    <div className="min-h-screen bg-bg">
      <header
        className="fixed top-0 w-full z-50 h-11 border-b border-hairline"
        style={{ background: "var(--color-header-bg)" }}
      >
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
              to="/dragon-tiger"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              龙虎榜
            </NavLink>
            <NavLink
              to="/breakout"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              晋级分析
            </NavLink>
            <NavLink
              to="/replay"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              行情回放
            </NavLink>
            <NavLink
              to="/roles"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              角色策略
            </NavLink>
            <NavLink
              to="/copilot"
              className={({ isActive }) =>
                `text-nav-link font-mono text-ink-muted hover:text-white transition-colors ${isActive ? "!text-white" : ""}`
              }
            >
              AI 问答
            </NavLink>
          </div>
          <div className="flex items-center gap-4">
            <IndexTicker />
            <ThemeToggle />
          </div>
        </div>
      </header>

      <Routes>
        <Route path="/" element={isTauri ? <TradingView /> : <LandingPage />} />
        <Route path="/trading" element={<TradingView />} />
        <Route path="/roles" element={<RoleView />} />
        <Route path="/copilot" element={<AiCopilot />} />
        <Route path="/replay" element={<ReplayView />} />
        <Route path="/breakout" element={<BreakoutView />} />
        <Route path="/dragon-tiger" element={<DragonTigerView />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Routes>
    </div>
  );
}
