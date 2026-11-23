//! 工作区布局的持久化（`{atrading_home}/workspace-layouts.json`）。
//!
//! 布局树的结构由前端定义，而且会随界面演进不断加字段。后端不在 Rust 里重建一套
//! 同样的模型：只校验信封形状与体积，原样落盘、原样读回。这样前端调整一次面板
//! 数据结构，后端不需要跟着改；反过来也不会出现"后端校验把新字段丢掉"的静默丢数据。
//!
//! 写入是原子的（写临时文件再 rename）：布局在拖拽时会被频繁提交，写到一半被打断
//! 不能让用户丢掉整个工作区。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

/// 落盘上限。正常布局只有几 KB，512KB 既够极端自定义，也能挡住误传的大文件。
const MAX_BYTES: usize = 512 * 1024;
/// 自定义布局数量上限（与前端导入上限保持一致）。
const MAX_LAYOUTS: usize = 50;

fn layouts_path() -> std::path::PathBuf {
    openatrading_core::atrading_home().join("workspace-layouts.json")
}

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// 读取快照；文件不存在或内容损坏都返回 `None`（前端会回落到本地缓存并重新上传）。
fn read_snapshot() -> Option<Value> {
    let path = layouts_path();
    let text = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<Value>(&text) {
        Ok(value) if value.is_object() => Some(value),
        Ok(_) => None,
        Err(e) => {
            openatrading_core::log::warn!("workspace layouts parse failed: {e}");
            None
        }
    }
}

fn write_snapshot(text: &str) -> Result<(), String> {
    let path = layouts_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

/// 只校验"能不能安全搬运"，不校验树本身的语义——那是前端的事。
fn validate(body: &Value) -> Result<(), String> {
    let Some(object) = body.as_object() else {
        return Err("布局快照必须是 JSON 对象".into());
    };

    let Some(layouts) = object.get("layouts").and_then(Value::as_array) else {
        return Err("缺少 layouts 数组".into());
    };
    if layouts.len() > MAX_LAYOUTS {
        return Err(format!("自定义布局最多 {MAX_LAYOUTS} 个"));
    }

    for (index, layout) in layouts.iter().enumerate() {
        let position = index + 1;
        let Some(layout) = layout.as_object() else {
            return Err(format!("第 {position} 个布局不是对象"));
        };
        match layout.get("id").and_then(Value::as_str) {
            Some(id) if !id.is_empty() => {}
            _ => return Err(format!("第 {position} 个布局缺少 id")),
        }
        if !layout.get("root").map(Value::is_object).unwrap_or(false) {
            return Err(format!("第 {position} 个布局缺少 root"));
        }
        if layout.get("name").is_some_and(|name| !name.is_string()) {
            return Err(format!("第 {position} 个布局的 name 必须是字符串"));
        }
    }

    if object
        .get("presetViews")
        .is_some_and(|views| !views.is_object())
    {
        return Err("presetViews 必须是对象".into());
    }
    if object
        .get("activeId")
        .is_some_and(|id| !id.is_string() && !id.is_null())
    {
        return Err("activeId 必须是字符串".into());
    }

    Ok(())
}

/// GET /api/layouts —— 没有存档时返回 `null`，前端据此决定是否上传本地缓存。
pub async fn get_layouts() -> Response {
    match read_snapshot() {
        Some(snapshot) => Json(snapshot).into_response(),
        None => Json(Value::Null).into_response(),
    }
}

/// PUT /api/layouts —— 整份快照覆盖写入（布局改动本来就是整体性的，不做增量）。
pub async fn save_layouts(Json(body): Json<Value>) -> Response {
    if let Err(message) = validate(&body) {
        return error_response(StatusCode::BAD_REQUEST, &message);
    }
    let text = match serde_json::to_string_pretty(&body) {
        Ok(text) => text,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &format!("序列化失败: {e}")),
    };
    if text.len() > MAX_BYTES {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "布局快照过大");
    }
    match write_snapshot(&text) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_minimal_snapshot() {
        let body = json!({
            "version": 1,
            "activeId": "review",
            "presetViews": {},
            "layouts": [],
        });
        assert!(validate(&body).is_ok());
    }

    #[test]
    fn rejects_missing_or_malformed_layouts() {
        assert!(validate(&json!({})).is_err());
        assert!(validate(&json!({ "layouts": "nope" })).is_err());
        assert!(validate(&json!({ "layouts": [{ "id": "a" }] })).is_err());
        assert!(validate(&json!({ "layouts": [{ "root": {} }] })).is_err());
        assert!(validate(&json!({ "layouts": [{ "id": "a", "root": 1 }] })).is_err());
        assert!(validate(&json!({ "layouts": [{ "id": "a", "root": {}, "name": 3 }] })).is_err());
    }

    #[test]
    fn rejects_bad_envelope_fields() {
        assert!(validate(&json!({ "layouts": [], "presetViews": [] })).is_err());
        assert!(validate(&json!({ "layouts": [], "activeId": 1 })).is_err());
        // 前端可能把 activeId 传成 null（未选布局）——这是合法的
        assert!(validate(&json!({ "layouts": [], "activeId": null })).is_ok());
    }

    #[test]
    fn caps_layout_count() {
        let layouts: Vec<Value> = (0..=MAX_LAYOUTS)
            .map(|i| json!({ "id": format!("l{i}"), "root": { "type": "pane" } }))
            .collect();
        assert!(validate(&json!({ "layouts": layouts })).is_err());
    }
}
