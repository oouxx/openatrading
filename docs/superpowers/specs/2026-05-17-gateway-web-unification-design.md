# Gateway-Web 统一管理

## 目标

将 web 服务器集成到 core crate，由 Gateway 统一管理生命周期。一个进程同时跑 cron scheduler 和 web API。

## 模块变化

```
crates/core/src/
├── gateway/
│   └── mod.rs          # Gateway struct → builder 模式
├── web/                # ← 从 crates/web/src/ 移入
│   ├── mod.rs          #   路由注册、start(port)
│   ├── handlers.rs     #   所有 API handler
│   ├── chat.rs         #   AI chat SSE
│   └── download.rs     #   install.sh / downloads 代理
├── cron/
├── memory/
├── trade/
└── ...

crates/web/             # 删除
```

## Gateway Builder

```rust
pub struct Gateway {
    pub memory: Arc<SqliteMemoryStore>,
    pub trade: Arc<SqliteTradeManager>,
}

impl Gateway {
    pub fn init(db_path: &Path) -> Result<Self>;

    /// 按配置启动 cron + web，阻塞至 Ctrl+C
    pub async fn start(self, config: GatewayConfig) -> Result<()>;
}

pub struct GatewayConfig {
    pub cron: Option<(CronConfig, Arc<dyn NotificationSender>)>,
    pub web_port: Option<u16>,
}
```

`start()`: `tokio::spawn(cron)` + `tokio::spawn(axum)`，Ctrl+C 一起停。

## CLI 入口

```bash
openatrading gateway run              # cron + web (port 8080)
openatrading gateway run --web-only   # 仅 web
openatrading gateway run --cron-only  # 仅 cron
```

## 数据共享

Web handler 通过 `Gateway` 的 Arc 直接访问：

- `sentiment-history` → 读 memory store 快照历史，不再逐日调 API
- AI chat → prompt 注入 N 日快照 + 持仓
- 实时数据（overview/limit-up）→ 保持调 API

## 部署

### 单 docker-compose.yaml

```yaml
services:
  gateway:
    image: ghcr.io/oouxx/openatrading:latest
    ports:
      - "8080:8080"
    env_file: .env
    volumes:
      - ~/.atrading:/root/.atrading
      - ./downloads:/app/downloads
    restart: unless-stopped

  frontend:
    image: ghcr.io/oouxx/openatrading-frontend:latest
    ports:
      - "80:80"
      - "443:443"
    depends_on:
      - gateway
    restart: unless-stopped
```

### nginx.conf

proxy_pass 直连域名，不再依赖环境变量：

```
proxy_pass http://trending.example.com:8080;
```

### 部署命令

```bash
make deploy-gateway  # docker compose up -d --build gateway
make deploy-web      # docker compose up -d --build frontend
make deploy          # docker compose up -d --build
```

## 清理

| 删除 | 原因 |
|------|------|
| `crates/web/` | 已移入 core |
| `openatrading-web` 二进制 | gateway 接管 |
| `docker-compose-web.yaml` | 合并到主 compose |
| `Dockerfile.backend` | 无需独立 web 镜像 |
| `Dockerfile.frontend` BACKEND_HOST env | nginx 直连域名 |
