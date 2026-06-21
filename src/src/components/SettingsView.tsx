import { useState } from "react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { useAppMode } from "@/contexts/AppModeContext";

export default function SettingsView() {
  const { setMode } = useAppMode();
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");

  return (
    <div className="flex flex-col h-full p-6 gap-6 overflow-y-auto">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-medium text-white">设置</h1>
        <Button variant="outline" size="sm" onClick={() => setMode("chat")}>
          返回
        </Button>
      </div>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">Agent 配置</h2>
        <div className="space-y-3 bg-bg-surface border border-hairline rounded-lg p-4">
          <div>
            <label className="text-xs text-ink-muted block mb-1">API Key</label>
            <Input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
              className="font-mono text-xs"
            />
          </div>
          <div>
            <label className="text-xs text-ink-muted block mb-1">默认模型</label>
            <Input
              value={model}
              onChange={(e) => setModel(e.target.value)}
              placeholder="claude-sonnet-4-6"
              className="font-mono text-xs"
            />
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">MCP 服务器</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <p className="text-xs text-ink-muted mb-3">
            管理 MCP 连接（名称、URL、密钥）
          </p>
          <Button variant="outline" size="sm">+ 添加服务器</Button>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">外观</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <div className="flex items-center justify-between">
            <span className="text-sm text-white">主题</span>
            <Button variant="outline" size="sm">暗色 (默认)</Button>
          </div>
        </div>
      </section>

      <section>
        <h2 className="text-sm font-medium text-white mb-3">数据源</h2>
        <div className="bg-bg-surface border border-hairline rounded-lg p-4">
          <label className="text-xs text-ink-muted block mb-1">API 地址</label>
          <Input value="http://localhost:8080" className="font-mono text-xs" readOnly />
        </div>
      </section>
    </div>
  );
}
