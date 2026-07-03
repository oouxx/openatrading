import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuItem,
  SidebarMenuButton,
} from "@/components/ui/sidebar"
import { ArrowLeft, Palette, Plug, Info, Bell, Cpu } from "lucide-react"
import { useAppMode } from "@/contexts/AppModeContext"

type SettingsTab = "appearance" | "agent" | "social" | "mcp" | "about"

const TABS: { id: SettingsTab; label: string; icon: typeof Palette }[] = [
  { id: "appearance", label: "外观", icon: Palette },
  { id: "agent", label: "Agent", icon: Cpu },
  { id: "social", label: "通知", icon: Bell },
  { id: "mcp", label: "MCP", icon: Plug },
  { id: "about", label: "关于", icon: Info },
]

interface SettingsSidebarProps {
  activeTab: SettingsTab
  onTabChange: (tab: SettingsTab) => void
}

export default function SettingsSidebar({ activeTab, onTabChange }: SettingsSidebarProps) {
  const { setMode } = useAppMode()

  return (
    <Sidebar collapsible="icon">
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={() => setMode("chat")} tooltip="返回">
              <ArrowLeft />
              <span>返回</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>

      <SidebarContent>
        <SidebarGroup>
          <SidebarMenu>
            {TABS.map((tab) => (
              <SidebarMenuItem key={tab.id}>
                <SidebarMenuButton
                  onClick={() => onTabChange(tab.id)}
                  isActive={activeTab === tab.id}
                  tooltip={tab.label}
                >
                  <tab.icon />
                  <span>{tab.label}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            ))}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
    </Sidebar>
  )
}

export type { SettingsTab }
