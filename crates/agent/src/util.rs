use pi_coding_agent::pi_agent_core::types::AgentMessage;
use pi_coding_agent::pi_ai_types::ContentBlock;
use serde_json::json;

/// Build a display transcript from agent messages.
///
/// Preserves structured content blocks (text/thinking/toolCall) instead of
/// flattening to plain text. Tool results are merged onto their corresponding
/// toolCall blocks so the frontend can render `ToolCallCard` with
/// `status`/`result`/`isError` both after a turn completes and on session reload.
#[must_use]
pub fn build_display_transcript(msgs: &[AgentMessage]) -> Vec<serde_json::Value> {
    // First pass: collect tool results keyed by tool_call_id.
    let mut tool_results: ahash::AHashMap<String, (String, bool)> = ahash::AHashMap::new();
    for msg in msgs {
        if let AgentMessage::ToolResult {
            tool_call_id,
            content,
            is_error,
            ..
        } = msg
        {
            let text: String = content
                .iter()
                .filter_map(|b| {
                    if let ContentBlock::Text { text, .. } = b {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            tool_results.insert(tool_call_id.clone(), (text, *is_error));
        }
    }

    // Second pass: emit user/assistant messages with structured content blocks.
    let mut out = Vec::new();
    for msg in msgs {
        let (role, content, ts) = match msg {
            AgentMessage::User { content, timestamp } => ("user", content, *timestamp),
            AgentMessage::Assistant {
                content, timestamp, ..
            } => ("assistant", content, *timestamp),
            _ => continue,
        };

        // Serialize full content blocks, then inject tool execution state
        // onto toolCall blocks from the matching toolResult message.
        let mut blocks_val = serde_json::to_value(content).unwrap_or(json!([]));
        if let Some(arr) = blocks_val.as_array_mut() {
            for b in arr.iter_mut() {
                if b.get("type").and_then(|t| t.as_str()) == Some("toolCall") {
                    if let Some(id) = b.get("id").and_then(|i| i.as_str()) {
                        if let Some((result, is_error)) = tool_results.get(id) {
                            b["status"] = json!(if *is_error { "error" } else { "success" });
                            b["result"] = json!(result);
                            b["isError"] = json!(is_error);
                        }
                    }
                }
            }
        }

        // Flattened text is kept for backward compatibility; the frontend
        // prefers the structured `content` array when present.
        let text: String = content
            .iter()
            .filter_map(|b| {
                if let ContentBlock::Text { text, .. } = b {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .collect();

        let created = chrono::DateTime::from_timestamp(ts / 1000, 0).map_or_else(
            || chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            |dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        );

        out.push(json!({
            "id": format!("msg-{}", ts),
            "kind": "message",
            "role": role,
            "text": text,
            "content": blocks_val,
            "createdAt": created,
        }));
    }
    out
}
