import {
  Bell,
  Blocks,
  Cpu,
  HardDrive,
  Info,
  LayoutPanelLeft,
  Palette,
  Plug,
  Puzzle,
} from "lucide-react"
import type { SettingsTab } from "@/stores/ui"

/**
 * 设置页签的唯一定义：侧栏导航与页面 h1 都从这里取，同一个 label 不写两遍。
 * `label` 是全页唯一需要出现的页签名——页面头部不再加同义副标题。
 */
export const SETTINGS_TABS: Record<SettingsTab, { label: string; icon: typeof Palette }> = {
  appearance: { label: "外观", icon: Palette },
  layouts: { label: "布局", icon: LayoutPanelLeft },
  agent: { label: "Agent", icon: Cpu },
  skills: { label: "技能", icon: Puzzle },
  extensions: { label: "扩展", icon: Blocks },
  social: { label: "通知", icon: Bell },
  mcp: { label: "MCP 服务器", icon: Plug },
  cache: { label: "缓存", icon: HardDrive },
  about: { label: "关于", icon: Info },
}
