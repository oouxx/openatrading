import { Globe, Radio, Terminal, type LucideIcon } from "lucide-react"
import { Button } from "@/components/ui/button"
import type { McpTransportType } from "../types/mcp"

interface McpTemplate {
  type: McpTransportType
  title: string
  label: string
  description: string
  icon: LucideIcon
}

const TEMPLATES: McpTemplate[] = [
  {
    type: "stdio",
    title: "本地进程",
    label: "STDIO",
    description: "通过命令启动本地 MCP server，适合文件、数据库等本机工具。",
    icon: Terminal,
  },
  {
    type: "http",
    title: "远程服务",
    label: "HTTP",
    description: "连接支持 Streamable HTTP 的远端 server，适合托管服务。",
    icon: Globe,
  },
  {
    type: "sse",
    title: "旧版远程",
    label: "SSE",
    description: "兼容 2024-11-05 的 HTTP+SSE 协议，仅在服务端不支持 HTTP 时使用。",
    icon: Radio,
  },
]

export function McpTemplateGrid({ onUse }: { onUse: (type: McpTransportType) => void }) {
  return (
    <div className="grid gap-3 sm:grid-cols-3">
      {TEMPLATES.map((t) => {
        const Icon = t.icon
        return (
          <div
            key={t.type}
            className="bg-surface border-hairline flex flex-col rounded-lg border p-4"
          >
            <div className="flex items-start justify-between gap-3">
              <span className="bg-hover text-secondary grid size-9 place-items-center rounded-lg">
                <Icon className="size-4" />
              </span>
              <span className="text-tertiary border-hairline font-number text-caption rounded-md border px-1.5 py-0.5 leading-none">
                {t.label}
              </span>
            </div>
            <h3 className="text-primary text-body mt-3 font-medium">{t.title}</h3>
            <p className="text-secondary text-caption mt-1 flex-1 leading-5">{t.description}</p>
            <Button
              type="button"
              variant="outline"
              size="xs"
              className="mt-3 w-fit"
              onClick={() => onUse(t.type)}
            >
              使用模板…
            </Button>
          </div>
        )
      })}
    </div>
  )
}
