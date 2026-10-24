import { Bell, Cpu, Info, Palette, Plug } from "lucide-react"
import type { SettingsTab } from "@/stores/ui"

/**
 * 设置页签的唯一定义：侧栏导航与页面 h1 都从这里取，同一个 label 不写两遍。
 * `label` 是全页唯一需要出现的页签名——页面头部不再加同义副标题。
 */
export const SETTINGS_TABS: Record<SettingsTab, { label: string; icon: typeof Palette }> = {
  appearance: { label: "外观", icon: Palette },
  agent: { label: "Agent", icon: Cpu },
  social: { label: "通知", icon: Bell },
  mcp: { label: "MCP 服务器", icon: Plug },
  about: { label: "关于", icon: Info },
}
