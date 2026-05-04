# 静态报告部署到 Vercel/Cloudflare Pages 设计

## 概述

将 `trending-agent` 生成的 HTML 报告转换成静态站点结构，部署到 Vercel 或 Cloudflare Pages。

## 核心思路

- 报告生成：`cargo run -- run --format html --output ./dist/reports/`
- 索引生成：`cargo run -- index --dir ./dist/` 生成 `dist/index.html`
- 部署：推送 `dist/` 目录，Vercel/Cloudflare Pages 自动托管

## 目录结构

```
dist/
├── index.html              # 索引页（列出所有历史报告）
├── reports/
│   ├── 2026-05-04-1530.html
│   ├── 2026-05-03-1530.html
│   └── ...
├── assets/               # 可选：CSS/JS
├── _routes.json          # Cloudflare Pages 配置
└── vercel.json          # Vercel 配置
```

## 索引页设计

`dist/index.html` 自动扫描 `reports/` 目录，生成带时间戳的报告列表：

```html
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <title>A股异动报告</title>
    <style>
        body { font-family: sans-serif; max-width: 800px; margin: 40px auto; }
        li { margin: 8px 0; }
    </style>
</head>
<body>
    <h1>📊 A股市场异动报告</h1>
    <ul>
        <li><a href="reports/2026-05-04-1530.html">2026-05-04 15:30</a></li>
        ...
    </ul>
</body>
</html>
```

## 部署配置

### Vercel (`vercel.json`)

```json
{
  "buildCommand": null,
  "outputDirectory": "dist",
  "routes": [
    { "src": "/", "dest": "/index.html" },
    { "src": "/reports/(.*)", "dest": "/reports/$1" }
  ]
}
```

### Cloudflare Pages (`_routes.json`)

```json
{
  "routes": [
    { "src": "/reports/.*", "headers": { "Cache-Control": "max-age=3600" } }
  ]
}
```

## 工作流

### 本地生成与部署

```bash
# 1. 生成 HTML 报告
cargo run -- run --format html --output ./dist/reports/

# 2. 生成索引页（新命令）
cargo run -- index --dir ./dist/

# 3. 提交并推送（Vercel/Cloudflare 自动部署）
git add dist/
git commit -m "chore: update reports"
git push
```

### CI 自动部署（可选）

```yaml
# .github/workflows/deploy.yml
name: Deploy Reports
on:
  push:
    paths:
      - 'dist/**'
jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: amondnet/vercel-action@v20
        with:
          vercel-token: ${{ secrets.VERCEL_TOKEN }}
          vercel-org-id: ${{ secrets.ORG_ID }}
          vercel-project-id: ${{ secrets.PROJECT_ID }}
          working-directory: ./dist
```

## 代码改造

### 1. 新增 `index` 子命令

**`src/cli/app.rs`**：
```rust
#[derive(Subcommand)]
pub enum Commands {
    // ...existing commands...
    /// 生成报告索引页
    Index(IndexOpts),
}

#[derive(clap::Args)]
pub struct IndexOpts {
    /// 输出目录（包含 reports/ 的父目录）
    #[arg(short, long, default_value = "./dist")]
    pub dir: String,
}
```

**`src/cli/mod.rs`**：
```rust
async fn run_index(opts: app::IndexOpts) -> Result<()> {
    let reports_dir = std::path::Path::new(&opts.dir).join("reports");
    // 扫描 reports/ 目录，生成 index.html
    // ...
    Ok(())
}
```

### 2. 修改 `run` 命令支持 `--output`

（已支持）

### 3. 添加部署配置文件

- `vercel.json` → 项目根目录
- `_routes.json` → `dist/` 目录

## 错误处理

- 报告生成失败：保留上次成功的报告
- 索引生成失败：输出错误日志，不影响已生成的报告
- 部署失败：Vercel/Cloudflare 提供构建日志

## 测试计划

1. 本地运行 `cargo run -- run --format html --output ./dist/reports/`
2. 验证 `dist/reports/*.html` 生成
3. 运行 `cargo run -- index --dir ./dist/`
4. 验证 `dist/index.html` 包含报告链接
5. 本地启动静态服务器测试：`cd dist && python3 -m http.server 8080`
6. 推送到 Git，验证 Vercel/Cloudflare 自动部署

## 后续扩展

- **定时自动生成**：GitHub Actions 定时运行 `cargo run` 并推送
- **报告过期清理**：索引页只显示最近 N 篇报告
- **搜索功能**：前端添加报告搜索
