import { Link } from "react-router-dom"
import { getApiBaseUrl } from "@/stores/data"

const DL_BASE = `${getApiBaseUrl()}/downloads`

function dl(platform: string, arch: string): string {
  switch (platform) {
    case "macos":
      return `${DL_BASE}/openatrading_${arch}.dmg`
    case "linux":
      return `${DL_BASE}/openatrading_${arch === "x86_64" ? "amd64" : arch}.deb`
    case "windows":
      return `${DL_BASE}/openatrading_x64-setup.exe`
    default:
      return "#"
  }
}

const features = [
  {
    to: "/roles",
    title: "角色策略管理",
    badge: "核心",
    desc: "每个角色独立运行 AI 交易策略 — 趋势跟踪、网格交易、套利… 一键新增/删除/编辑角色，AI 自动执行并展示盈亏曲线。",
    tags: ["📝 策略编辑", "🤖 AI 执行", "📊 盈亏曲线"],
  },
  {
    to: "/copilot",
    title: "AI 行情问答",
    badge: "A 股专版",
    desc: "流式 Markdown 输出，大盘研判、北向资金、个股分析与板块轮动。支持自然语言交互。",
    tags: ["🤖 流式对话", "📈 实时行情", "📝 分析报告"],
  },
  {
    to: "/replay",
    title: "行情回放 & 短线精灵",
    desc: "板块信号时间轴回放，涨速/涨幅实时排名，利好利空事件标注。逐分钟复盘全天行情。",
    tags: ["⏪ 时间轴", "⚡ 异动标注", "📋 事件日志"],
  },
  {
    to: "/trading",
    title: "涨停晋级分析",
    desc: "昨今对比晋级追踪，个股概况（市值/量比/涨停原因），分时 & 日K/周K/月K 图表。",
    tags: ["🔥 连板追踪", "📈 晋级率", "📊 K 线"],
  },
  {
    to: "/dragon-tiger",
    title: "龙虎榜分析",
    badge: "新增",
    badgeColor: "#oklch(55%_0.16_55)",
    desc: "机构/游资动向透视，营业部买卖明细，净买入排行，上榜原因归类。跟踪主力资金足迹。",
    tags: ["🐉 龙虎榜", "🏦 营业部", "💰 净买入"],
  },
]

export default function LandingPage() {
  const apiBase = getApiBaseUrl()

  return (
    <div className="bg-bg min-h-screen">
      {/* Hero */}
      <section className="mx-auto max-w-[1100px] px-8 pt-20 pb-14 text-center">
        <h1
          className="mb-4 leading-tight font-medium text-white"
          style={{ fontSize: "clamp(2.2rem, 5vw, 3.8rem)" }}
        >
          AI 自动交易<span className="text-ai">策略引擎</span>
        </h1>
        <p className="text-ink-muted mx-auto mb-10 max-w-[640px] text-lg leading-relaxed">
          每个角色运行独立策略，AI 自动决策并执行交易。在 Telegram 中与角色对话、查看持仓、调整参数
          — 所有盈亏实时追踪。
        </p>

        <div className="text-ink-muted mb-10 flex flex-wrap items-center justify-center gap-4 text-sm">
          {[
            { icon: "📝", label: "角色策略" },
            { icon: "🧠", label: "AI 决策" },
            { icon: "📈", label: "自动交易" },
            { icon: "📊", label: "盈亏展示" },
          ].map((node, i) => (
            <span key={node.label} className="flex items-center gap-4">
              {i > 0 && <span className="text-ai text-lg font-bold">→</span>}
              <span className="border-hairline bg-bg-surface flex items-center gap-2 rounded-xl border px-5 py-3.5 font-medium">
                <span className="text-xl">{node.icon}</span>
                {node.label}
              </span>
            </span>
          ))}
        </div>

        <div className="flex flex-wrap justify-center gap-3">
          <Link
            to="/app"
            className="bg-ai inline-flex items-center gap-2 rounded-[10px] px-6 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90"
          >
            开始使用 →
          </Link>
          <Link
            to="/roles"
            className="bg-ai inline-flex items-center gap-2 rounded-[10px] px-6 py-2.5 text-sm font-medium text-white transition-opacity hover:opacity-90"
          >
            管理角色策略 →
          </Link>
          <a
            href="#install"
            className="border-hairline text-ink hover:border-ai hover:text-ai inline-flex items-center gap-2 rounded-[10px] border px-6 py-2.5 text-sm font-medium transition-colors"
          >
            📦 下载安装
          </a>
        </div>
      </section>

      {/* Stats Bar */}
      <div className="mx-auto mb-12 grid max-w-[1000px] grid-cols-2 gap-4 px-8 md:grid-cols-4">
        {[
          { num: "6", label: "核心页面" },
          { num: "A 股", label: "全市场覆盖" },
          { num: "+12.8%", label: "本月 AI 策略收益", up: true },
          { num: "8", label: "活跃角色" },
        ].map((stat) => (
          <div
            key={stat.label}
            className="border-hairline bg-bg-surface rounded-xl border px-4 py-5 text-center"
          >
            <div
              className={`font-mono text-[1.8rem] font-semibold ${stat.up ? "text-up" : "text-white"}`}
            >
              {stat.num}
            </div>
            <div className="text-ink-muted mt-1 text-xs tracking-wider uppercase">{stat.label}</div>
          </div>
        ))}
      </div>

      {/* Features */}
      <section className="mx-auto max-w-[1200px] px-8 pb-16">
        <h2 className="mb-6 flex items-center gap-2.5 text-2xl font-medium text-white">核心功能</h2>
        <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <Link
              key={f.title}
              to={f.to}
              className="border-hairline bg-bg-surface overflow-hidden rounded-[14px] border transition-all hover:-translate-y-0.5 hover:shadow-lg"
            >
              <div className="px-[18px] py-[22px]">
                <h4 className="mb-1.5 flex items-center gap-2 text-base font-medium text-white">
                  {f.title}
                  {f.badge === "核心" && (
                    <span className="bg-ai inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white">
                      核心
                    </span>
                  )}
                  {f.badge === "A 股专版" && (
                    <span className="bg-ai inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white">
                      A 股专版
                    </span>
                  )}
                  {f.badge === "新增" && (
                    <span
                      className="inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white"
                      style={{ background: "oklch(55% 0.16 55)" }}
                    >
                      新增
                    </span>
                  )}
                </h4>
                <p className="text-ink-muted mb-2.5 text-[0.8rem] leading-relaxed">{f.desc}</p>
                <div className="flex flex-wrap gap-2">
                  {f.tags.map((t) => (
                    <span
                      key={t}
                      className="rounded-badge text-ink-muted px-2.5 py-[3px] text-[11px]"
                      style={{ background: "var(--color-tag-bg, rgba(255,255,255,0.05))" }}
                    >
                      {t}
                    </span>
                  ))}
                </div>
              </div>
            </Link>
          ))}

          {/* Telegram Bot card — design special styling */}
          <div
            className="bg-bg-surface overflow-hidden rounded-[14px] transition-all hover:-translate-y-0.5 hover:shadow-lg"
            style={{ border: "1px solid oklch(55% 0.14 240 / 0.4)" }}
          >
            <div className="px-[18px] py-[22px]">
              <h4 className="mb-1.5 flex items-center gap-2 text-base font-medium text-white">
                Telegram Bot 交互
                <span
                  className="inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white"
                  style={{ background: "oklch(55% 0.14 240)" }}
                >
                  Beta
                </span>
              </h4>
              <p className="text-ink-muted mb-2.5 text-[0.8rem] leading-relaxed">
                在 Telegram 中与你的交易角色直接对话 —
                查看持仓盈亏、接收交易信号、调整策略参数，所有操作实时同步。
              </p>
              <div className="flex flex-wrap gap-2">
                {["💬 对话交互", "📲 实时推送", "⚙️ 参数调整"].map((t) => (
                  <span
                    key={t}
                    className="rounded-badge text-ink-muted px-2.5 py-[3px] text-[11px]"
                    style={{ background: "var(--color-tag-bg, rgba(255,255,255,0.05))" }}
                  >
                    {t}
                  </span>
                ))}
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Install */}
      <section id="install" className="mx-auto max-w-[1200px] px-8 pb-16">
        <h2 className="mb-6 flex items-center gap-2.5 text-2xl font-medium text-white">
          📦 下载安装
        </h2>
        <div className="grid grid-cols-1 gap-5 md:grid-cols-2">
          {/* Desktop */}
          <div className="border-hairline bg-bg-surface rounded-[14px] border px-6 py-7">
            <h4 className="mb-2 flex items-center gap-2 text-base font-medium text-white">
              🖥️ 桌面端
            </h4>
            <p className="text-ink-muted mb-5 text-xs leading-relaxed">
              支持 macOS / Windows / Linux
              三大平台，原生桌面体验，内置行情数据引擎。从官网下载对应安装包。
            </p>
            <div className="mb-3.5 flex flex-wrap gap-2">
              {[
                { icon: "🍎", label: "macOS .dmg", platform: "macos", arch: "x86_64" },
                { icon: "🪟", label: "Windows .exe", platform: "windows", arch: "x86_64" },
                { icon: "🐧", label: "Linux .AppImage", platform: "linux", arch: "x86_64" },
              ].map((d) => (
                <a
                  key={d.label}
                  href={dl(d.platform, d.arch)}
                  className="border-hairline bg-ink-dim hover:border-ai hover:text-ai inline-flex items-center gap-1.5 rounded-lg border px-[18px] py-2 text-xs font-medium text-white transition-colors"
                >
                  <span className="text-lg">{d.icon}</span>
                  {d.label}
                </a>
              ))}
            </div>
            <p className="text-ink-muted text-[11px]">
              或访问{" "}
              <a href="https://github.com/oouxx/openatrading/releases" className="text-ai">
                GitHub Releases
              </a>
            </p>
          </div>

          {/* CLI */}
          <div className="border-hairline bg-bg-surface rounded-[14px] border px-6 py-7">
            <h4 className="mb-2 flex items-center gap-2 text-base font-medium text-white">
              ⌨️ CLI 命令行
            </h4>
            <p className="text-ink-muted mb-5 text-xs leading-relaxed">
              适用于 Linux/macOS 环境，支持嵌入量化策略 pipeline、定时任务及 CI/CD 流程。
            </p>
            <CopyCodeBlock
              text={`# 下载 CLI 二进制
curl -fsSL ${apiBase}/install.sh | bash

# 验证安装
openatrading --version

# 启动 Gateway
openatrading gateway run`}
            />
            <p className="text-ink-muted mt-2.5 text-[11px]">
              完整 CLI 文档：
              <a href="https://github.com/oouxx/openatrading" className="text-ai">
                GitHub
              </a>
            </p>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="border-hairline text-ink-muted border-t px-8 py-10 text-center text-xs">
        <div className="mb-3 flex flex-wrap justify-center gap-6">
          <a href="https://github.com/oouxx/openatrading" className="text-ai">
            GitHub
          </a>
          <a href="#" className="text-ai">
            文档
          </a>
          <a href="#" className="text-ai">
            API 参考
          </a>
          <a href="#" className="text-ai">
            下载
          </a>
        </div>
        openatrading · AI 自动交易策略引擎
      </footer>
    </div>
  )
}

function CopyCodeBlock({ text }: { text: string }) {
  return (
    <div
      className="border-hairline relative overflow-x-auto rounded-[10px] border p-[14px_18px] font-mono text-xs leading-relaxed whitespace-pre-wrap text-white"
      style={{ background: "var(--color-tag-bg, rgba(255,255,255,0.05))" }}
    >
      <button
        onClick={() => navigator.clipboard.writeText(text)}
        className="border-hairline bg-bg-surface text-ink-muted hover:border-ai hover:text-ai absolute top-2.5 right-3 cursor-pointer rounded-md border px-2.5 py-1 text-[11px] transition-colors"
      >
        复制
      </button>
      <code>{text}</code>
    </div>
  )
}
