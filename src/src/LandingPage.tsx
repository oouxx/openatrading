import { Link } from "react-router-dom"
import { Button } from "@/components/ui/button"
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
    title: "AI 行情对话",
    badge: "核心",
    desc: "与 AI 流式对话复盘 A 股：大盘研判、北向资金、个股分析、板块轮动，Markdown 报告实时渲染。",
    tags: ["💬 流式对话", "📈 个股分析", "📝 复盘报告"],
  },
  {
    title: "Skill 编排分析",
    badge: "模拟盘",
    desc: "把选股口径、复盘清单写成 Skill，新建会话即生效。模拟盘账户在面板里手动记账，AI 只出报告、不下单。",
    tags: ["🧩 Skill 编辑", "🤖 分析编排", "💹 手动记账"],
  },
  {
    title: "自选行情",
    desc: "实时行情与自选股管理，分时 / 日K / 周K / 月K，量比与涨速一览，一键关注跟踪。",
    tags: ["📊 实时行情", "⭐ 自选股", "📈 K 线"],
  },
  {
    title: "龙虎榜分析",
    desc: "机构 / 游资动向透视，营业部买卖明细，净买入排行，上榜原因归类，跟踪主力资金足迹。",
    tags: ["🐉 龙虎榜", "🏦 营业部", "💰 净买入"],
  },
  {
    title: "涨停晋级 & 板块异动",
    desc: "昨今对比连板晋级追踪，板块异动时间轴，涨速 / 涨幅实时排名，逐分钟捕捉盘口变化。",
    tags: ["🔥 连板追踪", "🧭 板块异动", "⏱️ 实时排名"],
  },
  {
    title: "行情回放",
    desc: "逐分钟复盘全天行情，异动信号时间轴回放，利好 / 利空事件标注，离线也能完整复盘。",
    tags: ["⏪ 时间轴", "⚡ 异动标注", "📋 事件日志"],
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
          AI<span className="text-ai">行情助手</span>
        </h1>
        <p className="text-ink-muted mx-auto mb-10 max-w-[640px] text-lg/relaxed">
          通过对话复盘 A 股行情，把选股口径和复盘清单写成 Skill 复用。
          模拟盘账户独立记账，行情实时估值，盈亏随时可查。
        </p>

        <div className="text-ink-muted mb-10 flex flex-wrap items-center justify-center gap-4 text-sm">
          {[
            { icon: "💬", label: "行情对话" },
            { icon: "🧩", label: "Skill 编排" },
            { icon: "💹", label: "模拟盘" },
            { icon: "📊", label: "复盘追踪" },
          ].map((node, i) => (
            <span key={node.label} className="flex items-center gap-4">
              {i > 0 && <span className="text-ai text-lg font-bold">→</span>}
              <span className="bg-bg-surface border-hairline flex items-center gap-2 rounded-xl border px-5 py-3.5 font-medium">
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
          <a
            href="#install"
            className="border-hairline hover:border-ai hover:text-ai text-ink inline-flex items-center gap-2 rounded-[10px] border px-6 py-2.5 text-sm font-medium transition-colors"
          >
            📦 下载安装
          </a>
        </div>
      </section>

      {/* Features */}
      <section className="mx-auto max-w-[1200px] px-8 pb-16">
        <h2 className="mb-6 flex items-center gap-2.5 text-2xl font-medium text-white">核心功能</h2>
        <div className="grid grid-cols-1 gap-5 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <Link
              key={f.title}
              to="/app"
              className="bg-bg-surface border-hairline overflow-hidden rounded-[14px] border transition-all hover:-translate-y-0.5 hover:shadow-lg"
            >
              <div className="px-[18px] py-[22px]">
                <h4 className="mb-1.5 flex items-center gap-2 text-base font-medium text-white">
                  {f.title}
                  {f.badge && (
                    <span
                      className="inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white"
                      style={{
                        background:
                          f.badge === "核心"
                            ? "var(--color-ai, oklch(62% 0.19 250))"
                            : "oklch(55% 0.16 145)",
                      }}
                    >
                      {f.badge}
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

          {/* Multi-entry card (Web + Telegram) — same chat, different entry */}
          <div
            className="bg-bg-surface overflow-hidden rounded-[14px] transition-all hover:-translate-y-0.5 hover:shadow-lg"
            style={{ border: "1px solid oklch(55% 0.14 240 / 0.4)" }}
          >
            <div className="px-[18px] py-[22px]">
              <h4 className="mb-1.5 flex items-center gap-2 text-base font-medium text-white">
                多端入口
                <span
                  className="inline-block rounded-[7px] px-2.5 py-[3px] text-[11px] font-medium text-white"
                  style={{ background: "oklch(55% 0.14 240)" }}
                >
                  Web + Telegram
                </span>
              </h4>
              <p className="text-ink-muted mb-2.5 text-[0.8rem] leading-relaxed">
                Web 与 Telegram 共用同一套对话能力，会话与自选在两端实时同步。
              </p>
              <div className="flex flex-wrap gap-2">
                {["🌐 Web", "✈️ Telegram", "🔄 实时同步"].map((t) => (
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
          <div className="bg-bg-surface border-hairline rounded-[14px] border px-6 py-7">
            <h4 className="mb-2 flex items-center gap-2 text-base font-medium text-white">
              🖥️ 桌面端
            </h4>
            <p className="text-ink-muted mb-5 text-xs/relaxed">
              支持 macOS / Windows / Linux
              三大平台，原生桌面体验，内置行情数据引擎。从官网下载对应安装包。
            </p>
            <div className="mb-3.5 flex flex-wrap gap-2">
              {[
                { icon: "🍎", label: "macOS .dmg", platform: "macos", arch: "x86_64" },
                { icon: "🪟", label: "Windows .exe", platform: "windows", arch: "x86_64" },
                { icon: "🐧", label: "Linux .deb", platform: "linux", arch: "x86_64" },
              ].map((d) => (
                <a
                  key={d.label}
                  href={dl(d.platform, d.arch)}
                  className="bg-ink-dim border-hairline hover:border-ai hover:text-ai inline-flex items-center gap-1.5 rounded-lg border px-[18px] py-2 text-xs font-medium text-white transition-colors"
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
          <div className="bg-bg-surface border-hairline rounded-[14px] border px-6 py-7">
            <h4 className="mb-2 flex items-center gap-2 text-base font-medium text-white">
              ⌨️ CLI 命令行
            </h4>
            <p className="text-ink-muted mb-5 text-xs/relaxed">
              适用于 Linux/macOS 环境，可启动 Gateway 服务、跑行情数据 pipeline 与定时新闻推送。
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
          <a href="#install" className="text-ai">
            下载
          </a>
        </div>
        openatrading · AI 行情助手
      </footer>
    </div>
  )
}

function CopyCodeBlock({ text }: { text: string }) {
  return (
    <div
      className="border-hairline relative overflow-x-auto rounded-[10px] border p-[14px_18px] font-mono text-xs/relaxed whitespace-pre-wrap text-white"
      style={{ background: "var(--color-tag-bg, rgba(255,255,255,0.05))" }}
    >
      <Button
        variant="outline"
        size="xs"
        onClick={() => navigator.clipboard.writeText(text)}
        className="bg-surface border-hairline text-ink-muted hover:border-ai hover:text-ai absolute top-2.5 right-3"
      >
        复制
      </Button>
      <code>{text}</code>
    </div>
  )
}
