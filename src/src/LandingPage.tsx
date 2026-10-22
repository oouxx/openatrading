import { Link } from "react-router-dom"
import { Button } from "@/components/ui/button"

// 安装包与 CLI 二进制统一走 GitHub Releases（自建下载目录已下线）。
const RELEASES = "https://github.com/oouxx/openatrading/releases/latest"
const INSTALL_SH =
  "https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh"

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
  return (
    <div className="bg-bg min-h-screen">
      {/* Hero */}
      <section className="mx-auto max-w-[1100px] px-8 pt-20 pb-14 text-center">
        <h1
          className="text-primary mb-4 leading-tight font-medium"
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
            className="bg-ai text-brand-foreground inline-flex items-center gap-2 rounded-lg px-6 py-2.5 text-sm font-medium transition-opacity hover:opacity-90"
          >
            开始使用 →
          </Link>
          <a
            href="#install"
            className="border-hairline hover:border-ai hover:text-ai text-ink inline-flex items-center gap-2 rounded-lg border px-6 py-2.5 text-sm font-medium transition-colors"
          >
            📦 下载安装
          </a>
        </div>
      </section>

      {/* Features */}
      <section className="mx-auto max-w-[1200px] px-8 pb-16">
        <h2 className="text-primary mb-6 flex items-center gap-2.5 text-2xl font-medium">
          核心功能
        </h2>
        <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
          {features.map((f) => (
            <Link
              key={f.title}
              to="/app"
              className="bg-bg-surface border-hairline hover:border-brand/40 overflow-hidden rounded-lg border transition-colors"
            >
              <div className="px-[18px] py-[22px]">
                <h4 className="text-primary mb-1.5 flex items-center gap-2 text-base font-medium">
                  {f.title}
                  {f.badge && (
                    <span
                      className="text-brand-foreground text-caption inline-block rounded-sm px-2.5 py-[3px] font-medium"
                      style={{
                        background: f.badge === "核心" ? "var(--color-brand)" : "var(--color-info)",
                      }}
                    >
                      {f.badge}
                    </span>
                  )}
                </h4>
                <p className="text-ink-muted text-body mb-2.5 leading-relaxed">{f.desc}</p>
                <div className="flex flex-wrap gap-2">
                  {f.tags.map((t) => (
                    <span
                      key={t}
                      className="rounded-badge text-ink-muted text-caption px-2.5 py-[3px]"
                      style={{ background: "var(--color-tag-bg)" }}
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
            className="bg-bg-surface border-hairline hover:border-brand/40 overflow-hidden rounded-lg border transition-colors"
            style={{}}
          >
            <div className="px-[18px] py-[22px]">
              <h4 className="text-primary mb-1.5 flex items-center gap-2 text-base font-medium">
                多端入口
                <span
                  className="text-brand-foreground text-caption inline-block rounded-sm px-2.5 py-[3px] font-medium"
                  style={{ background: "var(--color-info)" }}
                >
                  Web + Telegram
                </span>
              </h4>
              <p className="text-ink-muted text-body mb-2.5 leading-relaxed">
                Web 与 Telegram 共用同一套对话能力，会话与自选在两端实时同步。
              </p>
              <div className="flex flex-wrap gap-2">
                {["🌐 Web", "✈️ Telegram", "🔄 实时同步"].map((t) => (
                  <span
                    key={t}
                    className="rounded-badge text-ink-muted text-caption px-2.5 py-[3px]"
                    style={{ background: "var(--color-tag-bg)" }}
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
        <h2 className="text-primary mb-6 flex items-center gap-2.5 text-2xl font-medium">
          📦 下载安装
        </h2>
        <div className="grid grid-cols-1 gap-6 md:grid-cols-2">
          {/* Desktop */}
          <div className="bg-bg-surface border-hairline rounded-lg border px-6 py-7">
            <h4 className="text-primary mb-2 flex items-center gap-2 text-base font-medium">
              🖥️ 桌面端
            </h4>
            <p className="text-ink-muted mb-5 text-xs/relaxed">
              支持 macOS / Windows / Linux 三大平台，原生桌面体验，内置行情数据引擎。
              安装包随每个版本发布在 GitHub Releases，按平台选择即可。
            </p>
            <div className="mb-3.5 flex flex-wrap gap-2">
              <a
                href={RELEASES}
                className="bg-ai text-brand-foreground inline-flex items-center gap-1.5 rounded-lg px-[18px] py-2 text-xs font-medium transition-opacity hover:opacity-90"
              >
                <span className="text-lg">⬇</span> 前往 Releases 下载
              </a>
              {[
                { icon: "🍎", label: "macOS .dmg" },
                { icon: "🪟", label: "Windows .exe" },
                { icon: "🐧", label: "Linux .deb / .AppImage" },
              ].map((d) => (
                <span
                  key={d.label}
                  className="bg-ink-dim border-hairline text-ink-muted inline-flex items-center gap-1.5 rounded-lg border px-[14px] py-2 text-xs"
                >
                  <span className="text-lg">{d.icon}</span>
                  {d.label}
                </span>
              ))}
            </div>
            <p className="text-ink-muted text-caption">
              macOS 安装包是 ad-hoc 签名，首次打开需先
              <code className="bg-ink-dim mx-1 rounded px-1">xattr -dr com.apple.quarantine /Applications/OpenATrading.app</code>
              ；或使用 Homebrew：{" "}
              <a href="https://github.com/oouxx/homebrew-openatrading" className="text-ai">
                brew install --cask openatrading
              </a>
            </p>
          </div>

          {/* CLI */}
          <div className="bg-bg-surface border-hairline rounded-lg border px-6 py-7">
            <h4 className="text-primary mb-2 flex items-center gap-2 text-base font-medium">
              ⌨️ CLI 命令行
            </h4>
            <p className="text-ink-muted mb-5 text-xs/relaxed">
              适用于 Linux/macOS 环境，可启动 Gateway 服务、跑行情数据 pipeline 与定时新闻推送。
            </p>
            <CopyCodeBlock
              text={`# 安装 CLI（二进制从 GitHub Releases 获取）
curl -fsSL ${INSTALL_SH} | bash

# 验证安装
openatrading --version

# 启动 Gateway
openatrading gateway run`}
            />
            <p className="text-ink-muted text-caption mt-2.5">
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
          <a href={RELEASES} className="text-ai">
            Releases
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
      className="border-hairline text-primary relative overflow-x-auto rounded-lg border p-[14px_18px] font-mono text-xs/relaxed whitespace-pre-wrap"
      style={{ background: "var(--color-tag-bg)" }}
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
