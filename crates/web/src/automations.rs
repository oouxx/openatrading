//! 自动化任务的 HTTP API(CRUD + 手动触发 + 收件箱)。
//!
//! 路由注册见 [`crate::build_router`];调度执行在 `openatrading_core::automation`。

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

use openatrading_core::automation::{
    AutomationHub, AutomationId, AutomationPatch, NewAutomation, RunListQuery,
};

use crate::AppState;

fn error_response(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

/// GET /api/automations — 全部自动化任务(含下次运行时间)
pub async fn list_automations(State(state): State<Arc<AppState>>) -> Response {
    match list_automations_data(&state.automation).await {
        Ok(value) => Json(value).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

/// POST /api/automations — 新建任务,创建后立即热更新调度器
pub async fn create_automation(
    State(state): State<Arc<AppState>>,
    Json(draft): Json<NewAutomation>,
) -> Response {
    let created = match state.automation.store().create(&draft) {
        Ok(created) => created,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    if let Err(e) = state.automation.reload().await {
        openatrading_core::log::error!("[automation] reload after create failed: {e}");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "created but reload failed",
        );
    }
    Json(created).into_response()
}

/// PUT /api/automations/:id — 更新任务
pub async fn update_automation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(patch): Json<AutomationPatch>,
) -> Response {
    let updated = match state.automation.store().update(&AutomationId(id), &patch) {
        Ok(updated) => updated,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    };
    if let Err(e) = state.automation.reload().await {
        openatrading_core::log::error!("[automation] reload after update failed: {e}");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "updated but reload failed",
        );
    }
    Json(updated).into_response()
}

/// DELETE /api/automations/:id — 删除任务
pub async fn delete_automation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.automation.store().delete(&AutomationId(id)) {
        Ok(()) => {}
        Err(e) => return error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
    if let Err(e) = state.automation.reload().await {
        openatrading_core::log::error!("[automation] reload after delete failed: {e}");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "deleted but reload failed",
        );
    }
    Json(json!({ "ok": true })).into_response()
}

/// POST /api/automations/:id/trigger — 手动立即触发
pub async fn trigger_automation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    match state.automation.trigger_now(&AutomationId(id)).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => error_response(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

/// GET /api/automations/runs?automation_id=&unread=true&active=true&limit=50 — 运行记录
pub async fn list_runs(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RunListQuery>,
) -> Response {
    match state.automation.store().list_runs(&query) {
        Ok(runs) => Json(json!({ "runs": runs })).into_response(),
        Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// POST /api/automations/runs/:id/read — 标记已读
pub async fn mark_run_read(State(state): State<Arc<AppState>>, Path(id): Path<i64>) -> Response {
    match state.automation.store().mark_read(id) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

/// POST /api/automations/runs/:id/archive — 归档/取消归档(body: {"archived": bool})
pub async fn archive_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    archived: Option<Json<serde_json::Value>>,
) -> Response {
    let flag = archived
        .as_ref()
        .and_then(|Json(v)| v.get("archived").and_then(serde_json::Value::as_bool))
        .unwrap_or(true);
    match state.automation.store().set_archived(id, flag) {
        Ok(()) => Json(json!({ "ok": true, "archived": flag })).into_response(),
        Err(e) => error_response(StatusCode::NOT_FOUND, &e.to_string()),
    }
}

// ── Tauri IPC 包装(dispatch_get/dispatch_post 调用,签名与 axum handler 解耦)──

fn ipc_error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// GET /api/automations
pub async fn list_automations_data(hub: &Arc<AutomationHub>) -> Result<serde_json::Value, String> {
    let list = hub.store().list().map_err(ipc_error)?;
    Ok(json!({ "automations": list }))
}

/// GET /api/automations/runs?automation_id=&unread=&active=&limit=
pub async fn list_runs_data(
    hub: &Arc<AutomationHub>,
    query: &RunListQuery,
) -> Result<serde_json::Value, String> {
    let runs = hub.store().list_runs(query).map_err(ipc_error)?;
    Ok(json!({ "runs": runs }))
}

/// POST /api/automations — body 反序列化为 NewAutomation,创建后热更新调度器
pub async fn create_automation_data(
    hub: &Arc<AutomationHub>,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let draft: NewAutomation = serde_json::from_value(body.clone()).map_err(ipc_error)?;
    let created = hub.store().create(&draft).map_err(ipc_error)?;
    hub.reload().await.map_err(ipc_error)?;
    serde_json::to_value(created).map_err(ipc_error)
}

/// PUT /api/automations/:id — body 非空为更新,None/空为删除(与 mcp/skills 的 tauri 约定一致)
pub async fn update_automation_data(
    hub: &Arc<AutomationHub>,
    id: &str,
    body: Option<&serde_json::Value>,
) -> Result<serde_json::Value, String> {
    let empty = serde_json::Value::Object(serde_json::Map::new());
    let patch: AutomationPatch =
        serde_json::from_value(body.unwrap_or(&empty).clone()).map_err(ipc_error)?;
    let updated = hub
        .store()
        .update(&AutomationId(id.to_string()), &patch)
        .map_err(ipc_error)?;
    hub.reload().await.map_err(ipc_error)?;
    serde_json::to_value(updated).map_err(ipc_error)
}

/// DELETE /api/automations/:id
pub async fn delete_automation_data(
    hub: &Arc<AutomationHub>,
    id: &str,
) -> Result<serde_json::Value, String> {
    hub.store()
        .delete(&AutomationId(id.to_string()))
        .map_err(ipc_error)?;
    hub.reload().await.map_err(ipc_error)?;
    Ok(json!({ "ok": true }))
}

/// POST /api/automations/:id/trigger
pub async fn trigger_automation_data(
    hub: &Arc<AutomationHub>,
    id: &str,
) -> Result<serde_json::Value, String> {
    hub.trigger_now(&AutomationId(id.to_string()))
        .await
        .map_err(ipc_error)?;
    Ok(json!({ "ok": true }))
}

/// POST /api/automations/runs/:id/read
pub async fn mark_run_read_data(
    hub: &Arc<AutomationHub>,
    run_id: i64,
) -> Result<serde_json::Value, String> {
    hub.store().mark_read(run_id).map_err(ipc_error)?;
    Ok(json!({ "ok": true }))
}

/// POST /api/automations/runs/:id/archive — archived 缺省为 true
pub async fn archive_run_data(
    hub: &Arc<AutomationHub>,
    run_id: i64,
    body: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let archived = body
        .get("archived")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(true);
    hub.store()
        .set_archived(run_id, archived)
        .map_err(ipc_error)?;
    Ok(json!({ "ok": true, "archived": archived }))
}
