use std::sync::Arc;

use axum::{extract::State, response::Html, routing::get, Router};
use tokio::sync::Mutex;

use crate::report::report::Report;

#[derive(Clone)]
pub struct WebState {
    pub report: Arc<Mutex<Option<Report>>>,
}

impl Default for WebState {
    fn default() -> Self {
        Self::new()
    }
}

impl WebState {
    pub fn new() -> Self {
        Self {
            report: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn set_report(&self, report: Report) {
        let mut r = self.report.lock().await;
        *r = Some(report);
    }
}

pub async fn start_web_server(port: u16, state: WebState) {
    let app = Router::new()
        .route("/", get(index))
        .route("/report", get(get_report))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    println!("🌐 启动 Web UI: http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn index() -> Html<String> {
    Html(r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>A股市场 AI 分析报告</title>
    <style>
        * { box-sizing: border-box; margin: 0; padding: 0; }
        body { font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; background: #f5f5f5; padding: 20px; }
        .container { max-width: 1200px; margin: 0 auto; }
        h1 { color: #333; text-align: center; margin-bottom: 30px; }
        .nav { display: flex; justify-content: center; gap: 20px; margin-bottom: 30px; }
        .nav button { padding: 10px 20px; border: none; background: #007bff; color: white; border-radius: 5px; cursor: pointer; }
        .nav button:hover { background: #0056b3; }
        .nav button.active { background: #0056b3; }
        #content { background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,0.1); min-height: 500px; }
        .loading { text-align: center; color: #666; padding: 50px; }
    </style>
</head>
<body>
    <div class="container">
        <h1>📊 A股市场 AI 分析报告</h1>
        <div class="nav">
            <button id="btn-md" class="active" onclick="showFormat('markdown')">Markdown</button>
            <button id="btn-html" onclick="showFormat('html')">HTML</button>
        </div>
        <div id="content">
            <div class="loading">加载中...</div>
        </div>
    </div>
    <script>
        let currentFormat = 'markdown';
        async function loadReport() {
            const res = await fetch('/report?format=' + currentFormat);
            const text = await res.text();
            document.getElementById('content').innerHTML = text;
        }
        function showFormat(fmt) {
            currentFormat = fmt;
            document.getElementById('btn-md').className = fmt === 'markdown' ? 'active' : '';
            document.getElementById('btn-html').className = fmt === 'html' ? 'active' : '';
            loadReport();
        }
        loadReport();
    </script>
</body>
</html>
"#.to_string())
}

async fn get_report(
    State(state): State<WebState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Html<String> {
    let report = state.report.lock().await;
    if let Some(r) = report.as_ref() {
        let format = params.get("format").cloned().unwrap_or_default();
        let content = if format == "html" {
            r.render_html()
        } else {
            r.render_markdown()
        };
        if format == "html" {
            Html(content)
        } else {
            let escaped = content
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('\n', "<br>");
            Html(format!(
                r#"
<div style="white-space: pre-wrap; font-family: monospace; line-height: 1.6;">{}</div>
"#,
                escaped
            ))
        }
    } else {
        Html("<div style='text-align:center;color:#666;'>暂无报告数据</div>".to_string())
    }
}
