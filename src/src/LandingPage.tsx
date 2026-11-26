import { useState } from "react"
import { Link } from "react-router-dom"
import {
  ArrowRight,
  ArrowUpRight,
  Check,
  Copy,
  Download,
  Flame,
  History,
  MessagesSquare,
  Monitor,
  MonitorSmartphone,
  Puzzle,
  ShieldCheck,
  Star,
  Terminal,
  Trophy,
  type LucideIcon,
} from "lucide-react"

import { Button } from "@/components/ui/button"

// 安装包与 CLI 二进制统一走 GitHub Releases（自建下载目录已下线）。
const RELEASES = "https://github.com/oouxx/openatrading/releases/latest"
const INSTALL_SH = "https://raw.githubusercontent.com/oouxx/openatrading/main/install.sh"
const HOMEBREW_TAP = "https://github.com/oouxx/homebrew-openatrading"
const REPO = "https://github.com/oouxx/openatrading"

/**
 * 落地页是营销面，标题字号比应用内的 6 档体系大一级。
 *
 * 集中定义在这里而不是逐元素写 inline style：全页只有一套字号关系，
 * 改版时只动这三个常量。
 */
const DISPLAY =
  "text-[clamp(2.35rem,5.2vw,3.6rem)] leading-[1.08] font-semibold tracking-[-0.022em]"
const SECTION_TITLE = "text-[1.5rem] leading-[1.25] font-semibold tracking-[-0.01em]"

const NAV = [
  { href: "#features", label: "功能" },
  { href: "#install", label: "安装" },
  { href: REPO, label: "GitHub" },
]

interface Feature {
  icon: LucideIcon
  title: string
  desc: string
}

const FEATURES: Feature[] = [
  {
    icon: MessagesSquare,
    title: "AI 行情对话",
    desc: "流式复盘大盘、资金与个股，报告以 Markdown 实时渲染。",
  },
  {
    icon: Puzzle,
    title: "Skill 编排",
    desc: "把选股口径和复盘清单写成 Skill，新建会话即生效。",
  },
  {
    icon: Star,
    title: "自选与 K 线",
    desc: "实时行情、量比与涨速，分时和日 / 周 / 月 K 随手切换。",
  },
  {
    icon: Trophy,
    title: "龙虎榜",
    desc: "机构与游资席位、营业部买卖明细、净买入排行一目了然。",
  },
  {
    icon: Flame,
    title: "涨停与异动",
    desc: "连板晋级、炸板率、板块涨跌榜与成分股资金流。",
  },
  {
    icon: History,
    title: "行情回放",
    desc: "逐分钟复盘全天走势，异动信号沿时间轴还原。",
  },
  {
    icon: MonitorSmartphone,
    title: "多端同步",
    desc: "Web、桌面端与 Telegram 共用会话与自选。",
  },
  {
    icon: ShieldCheck,
    title: "开源可控",
    desc: "本地网关取数，配置与数据都留在自己的机器上。",
  },
]

export default function LandingPage() {
  return (
    <div className="bg-canvas min-h-screen">
      <header className="border-hairline bg-canvas sticky top-0 z-30 border-b">
        <div className="mx-auto flex h-14 max-w-6xl items-center gap-8 px-6">
          <Link to="/" className="flex items-center gap-2.5" aria-label="openatrading 首页">
            <img src="/app-icon.png" alt="" className="size-6 rounded-sm" />
            <span className="text-primary text-body font-semibold tracking-tight">
              openatrading
            </span>
          </Link>

          <div className="ml-auto flex items-center gap-8">
            <nav className="text-caption text-secondary hidden items-center gap-6 sm:flex">
              {NAV.map((item) => (
                <a
                  key={item.href}
                  href={item.href}
                  className="hover:text-primary transition-colors"
                >
                  {item.label}
                </a>
              ))}
            </nav>

            <Button asChild size="sm" className="h-8 px-3">
              <Link to="/app">
                打开应用
                <ArrowUpRight data-icon="inline-end" />
              </Link>
            </Button>
          </div>
        </div>
      </header>

      {/* Hero —— 一句话说清产品，右侧给出可感知的界面形态 */}
      <section className="hero-glow border-hairline overflow-hidden border-b">
        <div className="mx-auto grid max-w-6xl items-center gap-14 px-6 pt-20 pb-20 lg:grid-cols-[1.05fr_0.95fr] lg:gap-20 lg:pt-28 lg:pb-28">
          <div>
            <p className="text-tertiary text-caption flex items-center gap-2">
              <span className="bg-brand size-1.5 rounded-full" aria-hidden />
              开源 · 本地优先 · A 股复盘
            </p>
            <h1 className={`text-primary mt-6 ${DISPLAY}`}>
              A 股复盘，
              <br />
              从一句话开始。
            </h1>
            <p className="text-secondary text-h2 mt-6 max-w-lg leading-[1.7] font-normal">
              对话式复盘、Skill 复用、实时行情与模拟盘，收在同一个界面里。 数据走本地网关，Web
              与桌面端共用一套会话。
            </p>
            <div className="mt-9 flex flex-wrap items-center gap-3">
              <Button asChild size="lg" className="text-body h-10 px-4">
                <Link to="/app">
                  开始使用
                  <ArrowRight data-icon="inline-end" />
                </Link>
              </Button>
              <Button asChild size="lg" variant="outline" className="text-body h-10 px-4">
                <a href="#install">
                  <Download />
                  安装到本地
                </a>
              </Button>
            </div>
            <p className="text-tertiary text-caption mt-6">开源免费 · macOS / Windows / Linux</p>
          </div>

          <ProductPreview />
        </div>
      </section>

      {/* 功能 —— 左侧一段说明，右侧用细分隔线代替卡片堆叠 */}
      <section id="features" className="border-hairline border-b">
        <div className="mx-auto grid max-w-6xl gap-10 px-6 py-20 lg:grid-cols-[280px_1fr] lg:gap-16 lg:py-24">
          <div className="lg:sticky lg:top-24 lg:self-start">
            <p className="text-tertiary text-caption">功能</p>
            <h2 className={`text-primary mt-3 ${SECTION_TITLE}`}>围绕复盘设计的工作台</h2>
            <p className="text-secondary text-body mt-4 leading-[1.7]">
              不堆指标，也不做另一个行情终端。把看盘、研判、记录和验证放进同一条工作流，
              每个环节都能被下一次复用。
            </p>
          </div>

          <div className="border-hairline bg-hairline grid overflow-hidden rounded-lg border sm:grid-cols-2">
            {FEATURES.map((f) => (
              <div key={f.title} className="bg-canvas p-6">
                <f.icon className="text-brand size-4" aria-hidden />
                <h3 className="text-primary text-body mt-4 font-medium">{f.title}</h3>
                <p className="text-secondary text-caption mt-1.5 leading-[1.7]">{f.desc}</p>
              </div>
            ))}
          </div>
        </div>
      </section>

      {/* 安装 */}
      <section id="install" className="scroll-mt-14">
        <div className="mx-auto max-w-6xl px-6 py-20 lg:py-24">
          <div className="flex flex-wrap items-end justify-between gap-4">
            <div>
              <p className="text-tertiary text-caption">安装</p>
              <h2 className={`text-primary mt-3 ${SECTION_TITLE}`}>三分钟跑起来</h2>
            </div>
            <p className="text-tertiary text-caption">桌面端连本地网关，Web 可连远程网关。</p>
          </div>

          <div className="mt-10 grid gap-5 md:grid-cols-2">
            <div className="bg-surface border-hairline flex min-w-0 flex-col rounded-lg border p-6">
              <h3 className="text-primary text-body flex items-center gap-2 font-medium">
                <Monitor className="text-brand size-4" aria-hidden />
                桌面端
              </h3>
              <p className="text-secondary text-caption mt-2">macOS / Windows / Linux</p>
              <Button asChild size="sm" className="mt-5 self-start">
                <a href={RELEASES}>
                  <Download />
                  下载安装包
                </a>
              </Button>
              <CopyCodeBlock
                className="mt-5"
                text="xattr -dr com.apple.quarantine /Applications/OpenATrading.app"
              />
              <p className="text-tertiary text-caption mt-4">
                Homebrew：
                <a href={HOMEBREW_TAP} className="text-brand hover:underline">
                  brew install --cask openatrading
                </a>
              </p>
            </div>

            <div className="bg-surface border-hairline flex min-w-0 flex-col rounded-lg border p-6">
              <h3 className="text-primary text-body flex items-center gap-2 font-medium">
                <Terminal className="text-brand size-4" aria-hidden />
                CLI / Gateway
              </h3>
              <p className="text-secondary text-caption mt-2">
                行情网关与定时任务，桌面端和 Web 都连它。
              </p>
              <CopyCodeBlock
                className="mt-5"
                text={`# 安装 CLI
curl -fsSL ${INSTALL_SH} | bash

# 启动网关
openatrading gateway run`}
              />
              <a
                href={REPO}
                className="text-tertiary hover:text-brand text-caption mt-4 inline-flex items-center gap-1.5 self-start transition-colors"
              >
                CLI 文档
                <ArrowRight className="size-3" aria-hidden />
              </a>
            </div>
          </div>
        </div>
      </section>

      {/* 尾部 CTA */}
      <section className="border-hairline border-t">
        <div className="mx-auto flex max-w-6xl flex-col items-start justify-between gap-6 px-6 py-16 sm:flex-row sm:items-center">
          <div>
            <p className={`text-primary ${SECTION_TITLE}`}>今天收盘后，试一次新的复盘。</p>
            <p className="text-secondary text-caption mt-2">无需注册，浏览器里直接开始。</p>
          </div>
          <Button asChild size="lg" className="h-10 px-4">
            <Link to="/app">
              在浏览器中使用
              <ArrowRight data-icon="inline-end" />
            </Link>
          </Button>
        </div>
      </section>

      <footer className="border-hairline border-t">
        <div className="mx-auto flex max-w-6xl flex-wrap items-center gap-x-6 gap-y-3 px-6 py-8">
          <div className="flex items-center gap-2">
            <img src="/app-icon.png" alt="" className="size-4 rounded-sm opacity-80" />
            <span className="text-tertiary text-caption">openatrading · AI 行情助手</span>
          </div>
          <div className="text-tertiary text-caption ml-auto flex items-center gap-6">
            <a href={REPO} className="hover:text-primary transition-colors">
              GitHub
            </a>
            <a href={RELEASES} className="hover:text-primary transition-colors">
              Releases
            </a>
            <a href={REPO} className="hover:text-primary transition-colors">
              文档
            </a>
          </div>
        </div>
      </footer>
    </div>
  )
}

/**
 * 产品形态预览。
 *
 * 不写具体行情数字：营销页展示的是「界面长什么样」，不是当日数据快照，
 * 编造涨跌数字反而会让人误以为是实时行情。
 */
function ProductPreview() {
  return (
    <div className="relative" role="img" aria-label="产品界面预览：一段关于今日复盘的对话">
      <div
        className="bg-brand/10 absolute -inset-x-8 -top-10 -bottom-12 -z-10 rounded-full blur-3xl"
        aria-hidden
      />
      <div className="bg-surface border-hairline shadow-modal overflow-hidden rounded-lg border">
        <div className="border-hairline flex items-center gap-2 border-b px-4 py-3">
          <img src="/app-icon.png" alt="" className="size-4 rounded-sm" />
          <span className="text-primary text-caption font-medium">今日复盘</span>
          <span className="bg-hover text-tertiary text-micro ml-auto rounded-sm px-1.5 py-0.5">
            本地运行
          </span>
        </div>

        <div className="space-y-4 px-4 py-5">
          <div className="flex justify-end">
            <p className="bg-hover text-primary text-caption max-w-[85%] rounded-md px-3 py-2 leading-[1.6]">
              复盘今天的市场：资金流向、涨停梯队和明日重点。
            </p>
          </div>

          <div className="space-y-2.5">
            <p className="text-secondary text-caption leading-[1.7]">
              指数窄幅震荡，成交额较昨日回落。资金偏向低位补涨，高位股分歧加大。
            </p>
            <ul className="text-secondary text-caption space-y-1.5 leading-[1.7]">
              <li className="flex gap-2">
                <span className="bg-brand/50 mt-2 size-1 shrink-0 rounded-full" aria-hidden />
                涨停梯队集中在低价与题材补涨，连板高度有限。
              </li>
              <li className="flex gap-2">
                <span className="bg-brand/50 mt-2 size-1 shrink-0 rounded-full" aria-hidden />
                明日重点看首板晋级与板块承接，回避高位放量。
              </li>
            </ul>
          </div>

          <div className="flex flex-wrap gap-1.5">
            {["行情数据", "涨停池", "资金流向"].map((tag) => (
              <span
                key={tag}
                className="border-hairline text-tertiary text-micro rounded-sm border px-2 py-1"
              >
                {tag}
              </span>
            ))}
          </div>
        </div>

        <div className="border-hairline border-t px-4 py-3">
          <div className="border-hairline bg-canvas text-tertiary text-caption flex h-9 items-center rounded-md border px-3">
            继续追问…
          </div>
        </div>
      </div>
    </div>
  )
}

/** 代码块 + 复制。复制必须有反馈，否则用户不知道点没点成功。 */
function CopyCodeBlock({ text, className }: { text: string; className?: string }) {
  const [copied, setCopied] = useState(false)

  const copy = () => {
    navigator.clipboard.writeText(text).then(() => {
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    })
  }

  return (
    <div
      className={`bg-canvas border-hairline relative min-w-0 overflow-x-auto rounded-md border py-2.5 pr-2.5 pl-3 ${className ?? ""}`}
    >
      <Button
        variant="ghost"
        size="xs"
        onClick={copy}
        aria-label={copied ? "已复制" : "复制命令"}
        className="text-tertiary hover:text-primary absolute top-1.5 right-1.5"
      >
        {copied ? <Check className="text-success" /> : <Copy />}
        {copied ? "已复制" : "复制"}
      </Button>
      <code className="text-primary font-number text-caption block pr-16 leading-[1.7] whitespace-pre-wrap">
        {text}
      </code>
    </div>
  )
}
