import { useState, useEffect, useCallback } from "react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useUiStore } from "@/stores/ui";
import { fetchJson } from "@/stores/data";
import type { SettingsTab } from "@/components/SettingsSidebar";

interface McpServerConfig {
  name: string;
  transport: { type: string; url?: string; command?: string; args?: string[] };
  enabled: boolean;
}

const THEME_LABELS: Record<string, string> = { dark: "暗色", light: "浅色" };

export default function SettingsView({ activeTab }: { activeTab: SettingsTab }) {
  const theme = useUiStore((s) => s.theme);
  const setTheme = useUiStore((s) => s.setTheme);

  const [apiKey, setApiKey] = useState(
    () => localStorage.getItem("openatrading-api-key") || ""
  );
  const [model, setModel] = useState(
    () => localStorage.getItem("openatrading-model") || "claude-sonnet-4-6"
  );
  const [apiUrl, setApiUrl] = useState(
    () => localStorage.getItem("openatrading-api-url") || "http://localhost:8080"
  );
  const [servers, setServers] = useState<McpServerConfig[]>([]);
  const [showAddForm, setShowAddForm] = useState(false);
  const [newServer, setNewServer] = useState({ name: "", url: "" });

  const loadServers = useCallback(async () => {
    try {
      const data = await fetchJson<McpServerConfig[]>("/api/mcp/servers");
      setServers(data);
    } catch {
      // MCP API may not be available
    }
  }, []);

  useEffect(() => { loadServers(); }, [loadServers]);

  return (
    <div className="flex flex-col h-full p-6 gap-5 overflow-y-auto flex-1">
      {activeTab === "appearance" && (
        <>
          <section>
            <h2 className="text-xs text-ink-muted font-medium uppercase tracking-wider mb-3">外观</h2>
            <div className="bg-bg-surface border border-hairline rounded-lg p-4">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm text-white">主题</span>
                  <p className="text-[11px] text-ink-muted mt-0.5">选择深色或浅色主题</p>
                </div>
                <div className="flex gap-0 rounded-md overflow-hidden border border-hairline">
                  {(["dark", "light"] as const).map((t) => (
                    <button
                      key={t}
                      onClick={() => setTheme(t)}
                      className={`px-3 py-1.5 text-xs transition-colors ${
                        theme === t ? "bg-ai text-white" : "bg-transparent text-ink-muted hover:text-white hover:bg-bg-hover"
                      }`}
                    >
                      {THEME_LABELS[t]}
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-xs text-ink-muted font-medium uppercase tracking-wider mb-3">Agent 配置</h2>
            <div className="bg-bg-surface border border-hairline rounded-lg p-4 space-y-3">
              <div>
                <label className="text-xs text-ink-muted block mb-1">API Key</label>
                <Input type="password" value={apiKey}
                  onChange={(e) => { setApiKey(e.target.value); localStorage.setItem("openatrading-api-key", e.target.value); }}
                  placeholder="sk-..." className="font-mono text-xs" />
              </div>
              <div>
                <label className="text-xs text-ink-muted block mb-1">默认模型</label>
                <Input value={model}
                  onChange={(e) => { setModel(e.target.value); localStorage.setItem("openatrading-model", e.target.value); }}
                  placeholder="claude-sonnet-4-6" className="font-mono text-xs" />
              </div>
            </div>
          </section>

          <section>
            <h2 className="text-xs text-ink-muted font-medium uppercase tracking-wider mb-3">数据源</h2>
            <div className="bg-bg-surface border border-hairline rounded-lg p-4">
              <label className="text-xs text-ink-muted block mb-1">API 地址</label>
              <Input value={apiUrl}
                onChange={(e) => { setApiUrl(e.target.value); localStorage.setItem("openatrading-api-url", e.target.value); }}
                placeholder="http://localhost:8080" className="font-mono text-xs" />
            </div>
          </section>
        </>
      )}

      {activeTab === "mcp" && (
        <section>
          <div className="flex items-center justify-between mb-3">
            <h2 className="text-xs text-ink-muted font-medium uppercase tracking-wider">
              MCP 服务器 ({servers.length})
            </h2>
            <Button variant="outline" size="sm" onClick={() => setShowAddForm(!showAddForm)}>
              + 添加
            </Button>
          </div>

          {showAddForm && (
            <div className="bg-bg-surface border border-hairline rounded-lg p-4 mb-3 space-y-3">
              <div className="flex gap-2">
                <div className="flex-1">
                  <label className="text-xs text-ink-muted block mb-1">名称</label>
                  <Input value={newServer.name}
                    onChange={(e) => setNewServer((s) => ({ ...s, name: e.target.value }))}
                    placeholder="例如: 文件系统" className="text-xs" />
                </div>
                <div className="flex-[2]">
                  <label className="text-xs text-ink-muted block mb-1">URL</label>
                  <Input value={newServer.url}
                    onChange={(e) => setNewServer((s) => ({ ...s, url: e.target.value }))}
                    placeholder="http://localhost:8081/mcp" className="text-xs font-mono" />
                </div>
              </div>
              <div className="flex gap-2 justify-end">
                <Button variant="outline" size="sm" onClick={() => setShowAddForm(false)}>取消</Button>
                <Button size="sm" disabled={!newServer.name || !newServer.url}
                  onClick={async () => {
                    const config: McpServerConfig = {
                      name: newServer.name, enabled: true,
                      transport: { type: "streamable-http", url: newServer.url },
                    };
                    try {
                      await fetchJson("/api/mcp/servers", { method: "POST", body: JSON.stringify(config) });
                      setNewServer({ name: "", url: "" });
                      setShowAddForm(false);
                      loadServers();
                    } catch { /* silently fail */ }
                  }}>
                  添加
                </Button>
              </div>
            </div>
          )}

          <div className="space-y-2">
            {servers.length === 0 && !showAddForm && (
              <div className="bg-bg-surface border border-hairline rounded-lg p-8 text-center">
                <p className="text-xs text-ink-muted">暂无 MCP 服务器配置</p>
              </div>
            )}
            {servers.map((s) => (
              <div key={s.name} className="flex items-center gap-3 bg-bg-surface border border-hairline rounded-lg p-3 hover:border-white/10 transition-colors">
                <span className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${s.enabled ? "bg-up" : "bg-ink-muted"}`} />
                <div className="flex-1 min-w-0">
                  <span className="text-sm text-white">{s.name}</span>
                  <p className="text-[11px] text-ink-muted font-mono truncate">
                    {s.transport.type === "stdio"
                      ? `${s.transport.command} ${(s.transport as { args: string[] }).args?.join(" ") ?? ""}`
                      : (s.transport as { url: string }).url}
                  </p>
                </div>
                <div className="flex gap-1 flex-shrink-0">
                  <button className="px-2 py-1 text-[10px] text-ink-muted hover:text-white hover:bg-bg-hover rounded transition-colors"
                    onClick={async () => { try { await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}/toggle`); loadServers(); } catch { } }}>
                    {s.enabled ? "停用" : "启用"}
                  </button>
                  <button className="px-2 py-1 text-[10px] text-ink-muted hover:text-down hover:bg-bg-hover rounded transition-colors"
                    onClick={async () => { try { await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}`, { method: "DELETE" }); loadServers(); } catch { } }}>
                    删除
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      {activeTab === "about" && (
        <section>
          <h2 className="text-xs text-ink-muted font-medium uppercase tracking-wider mb-3">关于</h2>
          <div className="bg-bg-surface border border-hairline rounded-lg divide-y divide-hairline">
            {[
              ["应用名称", "OpenATrading"],
              ["版本", "0.12.0"],
              ["构建日期", "2025.06"],
              ["引擎", "AI Agent v4"],
            ].map(([label, value]) => (
              <div key={label} className="flex justify-between px-4 py-2 text-xs">
                <span className="text-ink-muted">{label}</span>
                <span className="text-white font-mono text-[11px]">{value}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
