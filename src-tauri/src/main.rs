#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tauri::State;
use trending_agent_core::agents::main::run_main_agent;
use trending_agent_core::api::Client;
use trending_agent_core::roles::role::Role;

struct ApiClient(Arc<Client>);

#[derive(serde::Serialize)]
struct AnalyzeResult {
    sentiment: SentimentInfo,
    technical: TechnicalInfo,
    role_analysis: String,
}

#[derive(serde::Serialize)]
struct SentimentInfo {
    trend: String,
    summary: String,
    hot_stocks: Vec<(String, String, i32)>,
    hot_articles: Vec<HotArticleInfo>,
}

#[derive(serde::Serialize)]
struct HotArticleInfo {
    title: String,
    heat_score: i32,
    nickname: String,
}

#[derive(serde::Serialize)]
struct TechnicalInfo {
    summary: String,
    hot_sectors: Vec<(String, u32)>,
    risk_signals: Vec<String>,
    levels: Vec<LevelInfo>,
}

#[derive(serde::Serialize)]
struct LevelInfo {
    days: u32,
    stocks: Vec<(String, String, f64, f64)>,
}

fn main() {
    let client = Arc::new(Client::new());

    tauri::Builder::default()
        .manage(ApiClient(client))
        .invoke_handler(tauri::generate_handler![
            analyze_market,
            get_available_roles
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn get_available_roles() -> Result<Vec<serde_json::Value>, String> {
    Ok(vec![
        serde_json::json!({"key": "trader", "name": "游资操盘手"}),
        serde_json::json!({"key": "quant", "name": "量化分析师"}),
        serde_json::json!({"key": "risk_officer", "name": "风控官"}),
        serde_json::json!({"key": "zarili", "name": "zarili量价派"}),
    ])
}

#[tauri::command]
async fn analyze_market(
    role_type: String,
    api_key: Option<String>,
    client: State<'_, ApiClient>,
) -> Result<AnalyzeResult, String> {
    let role = match role_type.as_str() {
        "trader" => Role::Trader,
        "quant" => Role::Quant,
        "risk_officer" => Role::RiskOfficer,
        "zarili" => Role::Zarili,
        _ => return Err(format!("无效的角色类型: {}", role_type)),
    };

    // Use provided api_key or fall back to env (LLM_API_KEY > DEEPSEEK_API_KEY > OPENROUTER_API_KEY)
    let key = api_key
        .filter(|k| !k.is_empty())
        .or_else(|| std::env::var("LLM_API_KEY").ok())
        .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
        .or_else(|| std::env::var("OPENROUTER_API_KEY").ok())
        .ok_or("未配置 LLM_API_KEY，请在 .env 中设置或传入 api_key 参数")?;

    let result = run_main_agent(role, &client.0, &key)
        .await
        .map_err(|e| format!("分析失败: {}", e))?;

    Ok(AnalyzeResult {
        sentiment: SentimentInfo {
            trend: result.sentiment.trend,
            summary: result.sentiment.summary,
            hot_stocks: result.sentiment.hot_stocks,
            hot_articles: result
                .sentiment
                .hot_articles
                .into_iter()
                .map(|a| HotArticleInfo {
                    title: a.title,
                    heat_score: a.heat_score,
                    nickname: a.nickname,
                })
                .collect(),
        },
        technical: TechnicalInfo {
            summary: result.technical.summary,
            hot_sectors: result.technical.hot_sectors,
            risk_signals: result.technical.risk_signals,
            levels: result
                .technical
                .levels
                .into_iter()
                .map(|l| LevelInfo {
                    days: l.days,
                    stocks: l.stocks,
                })
                .collect(),
        },
        role_analysis: result.role_analysis,
    })
}
