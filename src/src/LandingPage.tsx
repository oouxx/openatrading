import { useState } from "react"
import { Link } from "react-router-dom"
import {
  ArrowRight,
  Check,
  Copy,
  Download,
  Flame,
  History,
  MessagesSquare,
  Monitor,
  MonitorSmartphone,
  Puzzle,
  Star,
  Terminal,
  Timer,
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
const DISPLAY = "text-[clamp(2rem,4.5vw,3.25rem)] leading-[1.15] font-semibold tracking-tight"
const SECTION_TITLE = "text-[1.35rem] leading-tight font-semibold"
const CARD_TITLE = "text-body font-medium"

const PRIMARY_CTA =
  "bg-brand text-brand-foreground inline-flex items-center gap-2 rounded-lg px-5 py-2.5 text-body font-medium transition-opacity hover:opacity-90"
const SECONDARY_CTA =
  "border-hairline text-primary hover:border-brand/50 hover:text-brand inline-flex items-center gap-2 rounded-lg border px-5 py-2.5 text-body font-medium transition-colors"

interface Feature {
  icon: LucideIcon
  title: string
  desc: string
}

const FEATURES: Feature[] = [
  {
    icon: MessagesSquare,
    title: "AI 行情对话",
    desc: "流式对话复盘 A 股：大盘研判、资金流向、个股与板块分析，Markdown 报告实时渲染。",
  },
  {
    icon: Puzzle,
    title: "Skill 编排",
    desc: "把选股口径、复盘清单写成 Skill，新建会话即生效，不用每次重复交代要求。",
  },
  {
    icon: Star,
    title: "自选与 K 线",
    desc: "实时行情、量比与涨速一览，分时 / 日 K / 周 K / 月 K 随手切换。",
  },
  {
    icon: Trophy,
    title: "龙虎榜",
    desc: "机构与游资动向、营业部买卖明细、净买入排行，上榜原因归类。",
  },
  {
    icon: Flame,
    title: "涨停与异动",
    desc: "昨今对比连板晋级，板块异动时间轴，涨速 / 涨幅实时排名。",
  },
  {
    icon: History,
    title: "行情回放",
    desc: "逐分钟复盘全天行情，异动信号与事件标注按时间轴回放。",
  },
  {
    icon: Timer,
    title: "定时自动化",
    desc: "按 cron 在后台巡检行情、推送财联社电报，结果进入收件箱。",
  },
  {
    icon: MonitorSmartphone,
    title: "多端入口",
    desc: "Web 与 Telegram 共用同一套对话能力，会话与自选在两端实时同步。",
  },
]

export default function LandingPage() {
  return (
    <div className="bg-canvas min-h-screen">
      {/* Hero —— 只说是什么、能做什么，导流按钮只留两个 */}
      <section className="mx-auto max-w-3xl px-6 pt-24 pb-16 text-center">
        <h1 className={`text-primary ${DISPLAY}`}>
          AI <span className="text-brand">行情助手</span>
        </h1>
        <p className="text-secondary text-h2 mx-auto mt-5 max-w-xl leading-relaxed font-normal">
          通过对话复盘 A 股行情，把选股口径与复盘清单写成 Skill 复用；
          模拟盘独立记账、按实时行情估值。
        </p>
        <div className="mt-8 flex flex-wrap justify-center gap-3">
          <Link to="/app" className={PRIMARY_CTA}>
            开始使用
            <ArrowRight className="size-4" />
          </Link>
          <a href="#install" className={SECONDARY_CTA}>
            <Download className="size-4" />
            下载安装
          </a>
        </div>
      </section>

      {/* 功能 —— 一项一句，不重复标题、不堆标签 */}
      <section className="mx-auto max-w-5xl px-6 pb-20">
        <h2 className={`text-primary ${SECTION_TITLE}`}>功能</h2>
        <div className="mt-5 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {FEATURES.map((f) => (
            <div key={f.title} className="bg-surface border-hairline rounded-lg border p-4">
              <f.icon className="text-brand size-4.5" aria-hidden />
              <h3 className={`text-primary mt-3 ${CARD_TITLE}`}>{f.title}</h3>
              <p className="text-secondary text-caption mt-1.5 leading-relaxed">{f.desc}</p>
            </div>
          ))}
        </div>
      </section>

      {/* 安装 */}
      <section id="install" className="mx-auto max-w-5xl scroll-mt-6 px-6 pb-16">
        <h2 className={`text-primary ${SECTION_TITLE}`}>安装</h2>
        <div className="mt-5 grid gap-4 md:grid-cols-2">
          <div className="bg-surface border-hairline flex flex-col rounded-lg border p-5">
            <h3 className={`text-primary flex items-center gap-2 ${CARD_TITLE}`}>
              <Monitor className="text-brand size-4" aria-hidden />
              桌面端
            </h3>
            <p className="text-secondary text-caption mt-2 leading-relaxed">
              macOS / Windows / Linux 安装包随每个版本发布在 GitHub Releases。
            </p>
            <a href={RELEASES} className={`${PRIMARY_CTA} mt-4 self-start`}>
              <Download className="size-4" />
              前往 Releases
            </a>
            <p className="text-tertiary text-caption mt-4">
              macOS 包是 ad-hoc 签名，首次打开需先解除隔离：
            </p>
            <CopyCodeBlock
              className="mt-1.5"
              text="xattr -dr com.apple.quarantine /Applications/OpenATrading.app"
            />
            <p className="text-tertiary text-caption mt-3">
              或用 Homebrew：
              <a href={HOMEBREW_TAP} className="text-brand hover:underline">
                brew install --cask openatrading
              </a>
            </p>
          </div>

          <div className="bg-surface border-hairline flex flex-col rounded-lg border p-5">
            <h3 className={`text-primary flex items-center gap-2 ${CARD_TITLE}`}>
              <Terminal className="text-brand size-4" aria-hidden />
              CLI / Gateway
            </h3>
            <p className="text-secondary text-caption mt-2 leading-relaxed">
              Linux / macOS 上启动行情网关与定时任务，桌面端与 Web 都连它取数据。
            </p>
            <CopyCodeBlock
              className="mt-4"
              text={`# 安装 CLI（二进制取自 GitHub Releases）
curl -fsSL ${INSTALL_SH} | bash

# 启动网关
openatrading gateway run`}
            />
            <a
              href={REPO}
              className="text-tertiary hover:text-brand text-caption mt-3 inline-flex items-center gap-1.5"
            >
              CLI 文档
              <ArrowRight className="size-3" aria-hidden />
            </a>
          </div>
        </div>
      </section>

      {/* 尾部 CTA —— 滚到底还想用的人不用再滚回去 */}
      <section className="px-6 pb-20 text-center">
        <Link to="/app" className={PRIMARY_CTA}>
          在浏览器中直接使用
          <ArrowRight className="size-4" />
        </Link>
      </section>

      <footer className="border-hairline text-tertiary text-caption flex flex-wrap items-center justify-center gap-x-6 gap-y-2 border-t px-6 py-8">
        <a href={REPO} className="hover:text-brand">
          GitHub
        </a>
        <a href={RELEASES} className="hover:text-brand">
          Releases
        </a>
        <span>openatrading · AI 行情助手</span>
      </footer>
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
      className={`bg-canvas border-hairline relative overflow-x-auto rounded-lg border py-2.5 pr-2.5 pl-3 ${className ?? ""}`}
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
      <code className="text-primary font-number text-caption block pr-16 leading-relaxed whitespace-pre-wrap">
        {text}
      </code>
    </div>
  )
}
