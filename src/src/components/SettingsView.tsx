import { useState, useEffect } from "react";
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

  const loadServers = async () => {
    try {
      const data = await fetchJson<McpServerConfig[]>("/api/mcp/servers");
      setServers(data);
    } catch {
      // MCP API may not be available
    }
  };

  // Load on mount — initialization fetch outside React data flow
  // eslint-disable-next-line react-hooks/set-state-in-effect
  useEffect(() => { loadServers(); }, []);

  return (
    <div className="flex h-full flex-1 flex-col gap-5 overflow-y-auto p-6">
      {activeTab === "appearance" && (
        <>
          <section>
            <h2 className="mb-3 text-xs font-medium tracking-wider text-ink-muted uppercase">外观</h2>
            <div className="rounded-lg border border-hairline bg-bg-surface p-4">
              <div className="flex items-center justify-between">
                <div>
                  <span className="text-sm text-white">主题</span>
                  <p className="mt-0.5 text-[11px] text-ink-muted">选择深色或浅色主题</p>
                </div>
                <div className="flex gap-0 overflow-hidden rounded-md border border-hairline">
                  {(["dark", "light"] as const).map((t) => (
                    <button
                      key={t}
                      onClick={() => setTheme(t)}
                      className={`px-3 py-1.5 text-xs transition-colors ${
                        theme === t ? "bg-ai text-white" : "bg-transparent text-ink-muted hover:bg-bg-hover hover:text-white"
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
            <h2 className="mb-3 text-xs font-medium tracking-wider text-ink-muted uppercase">Agent 配置</h2>
            <div className="space-y-3 rounded-lg border border-hairline bg-bg-surface p-4">
              <div>
                <label className="mb-1 block text-xs text-ink-muted">API Key</label>
                <Input type="password" value={apiKey}
                  onChange={(e) => { setApiKey(e.target.value); localStorage.setItem("openatrading-api-key", e.target.value); }}
                  placeholder="sk-..." className="font-mono text-xs" />
              </div>
              <div>
                <label className="mb-1 block text-xs text-ink-muted">默认模型</label>
                <Input value={model}
                  onChange={(e) => { setModel(e.target.value); localStorage.setItem("openatrading-model", e.target.value); }}
                  placeholder="claude-sonnet-4-6" className="font-mono text-xs" />
              </div>
            </div>
          </section>

          <section>
            <h2 className="mb-3 text-xs font-medium tracking-wider text-ink-muted uppercase">数据源</h2>
            <div className="rounded-lg border border-hairline bg-bg-surface p-4">
              <label className="mb-1 block text-xs text-ink-muted">API 地址</label>
              <Input value={apiUrl}
                onChange={(e) => { setApiUrl(e.target.value); localStorage.setItem("openatrading-api-url", e.target.value); }}
                placeholder="http://localhost:8080" className="font-mono text-xs" />
            </div>
          </section>
        </>
      )}

      {activeTab === "mcp" && (
        <section>
          <div className="mb-3 flex items-center justify-between">
            <h2 className="text-xs font-medium tracking-wider text-ink-muted uppercase">
              MCP 服务器 ({servers.length})
            </h2>
            <Button variant="outline" size="sm" onClick={() => setShowAddForm(!showAddForm)}>
              + 添加
            </Button>
          </div>

          {showAddForm && (
            <div className="mb-3 space-y-3 rounded-lg border border-hairline bg-bg-surface p-4">
              <div className="flex gap-2">
                <div className="flex-1">
                  <label className="mb-1 block text-xs text-ink-muted">名称</label>
                  <Input value={newServer.name}
                    onChange={(e) => setNewServer((s) => ({ ...s, name: e.target.value }))}
                    placeholder="例如: 文件系统" className="text-xs" />
                </div>
                <div className="flex-[2]">
                  <label className="mb-1 block text-xs text-ink-muted">URL</label>
                  <Input value={newServer.url}
                    onChange={(e) => setNewServer((s) => ({ ...s, url: e.target.value }))}
                    placeholder="http://localhost:8081/mcp" className="font-mono text-xs" />
                </div>
              </div>
              <div className="flex justify-end gap-2">
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
              <div className="rounded-lg border border-hairline bg-bg-surface p-8 text-center">
                <p className="text-xs text-ink-muted">暂无 MCP 服务器配置</p>
              </div>
            )}
            {servers.map((s) => (
              <div key={s.name} className="flex items-center gap-3 rounded-lg border border-hairline bg-bg-surface p-3 transition-colors hover:border-white/10">
                <span className={`size-1.5 flex-shrink-0 rounded-full ${s.enabled ? "bg-up" : "bg-ink-muted"}`} />
                <div className="min-w-0 flex-1">
                  <span className="text-sm text-white">{s.name}</span>
                  <p className="truncate font-mono text-[11px] text-ink-muted">
                    {s.transport.type === "stdio"
                      ? `${s.transport.command} ${(s.transport as { args: string[] }).args?.join(" ") ?? ""}`
                      : (s.transport as { url: string }).url}
                  </p>
                </div>
                <div className="flex flex-shrink-0 gap-1">
                  <button className="rounded px-2 py-1 text-[10px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-white"
                    onClick={async () => { try { await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}/toggle`); loadServers(); } catch { /* noop */ } }}>
                    {s.enabled ? "停用" : "启用"}
                  </button>
                  <button className="rounded px-2 py-1 text-[10px] text-ink-muted transition-colors hover:bg-bg-hover hover:text-down"
                    onClick={async () => { try { await fetchJson(`/api/mcp/servers/${encodeURIComponent(s.name)}`, { method: "DELETE" }); loadServers(); } catch { /* noop */ } }}>
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
          <h2 className="mb-3 text-xs font-medium tracking-wider text-ink-muted uppercase">关于</h2>
          <div className="divide-y rounded-lg border border-hairline bg-bg-surface">
            {[
              ["应用名称", "OpenATrading"],
              ["版本", "0.12.0"],
              ["构建日期", "2026.06"],
              ["引擎", "pi-rs v1.78.0"],
            ].map(([label, value]) => (
              <div key={label} className="flex justify-between px-4 py-2 text-xs">
                <span className="text-ink-muted">{label}</span>
                <span className="font-mono text-[11px] text-white">{value}</span>
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
}
