import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { ArrowLeft, Bell, Cpu, Info, Palette, Plug } from "lucide-react"
import { useAppMode } from "@/contexts/useAppMode"
import { useSettingsStore } from "@/stores/settingsStore"

type SettingsTab = "appearance" | "agent" | "social" | "mcp" | "about"

const TAB_META: Record<SettingsTab, { label: string; icon: typeof Palette }> = {
  appearance: { label: "外观", icon: Palette },
  agent: { label: "Agent", icon: Cpu },
  social: { label: "通知", icon: Bell },
  mcp: { label: "MCP 服务器", icon: Plug },
  about: { label: "关于", icon: Info },
}

const NAV_GROUPS: { label: string; tabs: SettingsTab[] }[] = [
  { label: "偏好", tabs: ["appearance", "social"] },
  { label: "Agent", tabs: ["agent", "mcp"] },
  { label: "系统", tabs: ["about"] },
]

interface SettingsSidebarProps {
  activeTab: SettingsTab
  onTabChange: (tab: SettingsTab) => void
}

export default function SettingsSidebar({ activeTab, onTabChange }: SettingsSidebarProps) {
  const { setMode } = useAppMode()
  const apiUrl = useSettingsStore((s) => s.apiUrl)

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
        {NAV_GROUPS.map((group) => (
          <SidebarGroup key={group.label}>
            <SidebarGroupLabel className="text-tertiary text-caption font-medium">
              {group.label}
            </SidebarGroupLabel>
            <SidebarMenu>
              {group.tabs.map((tab) => {
                const { label, icon: Icon } = TAB_META[tab]
                return (
                  <SidebarMenuItem key={tab}>
                    <SidebarMenuButton
                      onClick={() => onTabChange(tab)}
                      isActive={activeTab === tab}
                      tooltip={label}
                    >
                      <Icon />
                      <span>{label}</span>
                    </SidebarMenuButton>
                  </SidebarMenuItem>
                )
              })}
            </SidebarMenu>
          </SidebarGroup>
        ))}
      </SidebarContent>

      <SidebarFooter className="group-data-[collapsible=icon]:hidden">
        <div className="px-2 py-1">
          <p className="text-tertiary text-caption">网关</p>
          {apiUrl ? (
            <p className="text-secondary font-number text-caption mt-0.5 truncate" title={apiUrl}>
              {apiUrl}
            </p>
          ) : (
            <p className="text-tertiary text-caption mt-0.5">跟随应用默认</p>
          )}
        </div>
      </SidebarFooter>
    </Sidebar>
  )
}

export type { SettingsTab }
