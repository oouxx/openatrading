use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use pi_coding_agent::pi_agent_core::types::AgentToolResult;
use pi_coding_agent::pi_agent_core::types::DynTool;
use pi_coding_agent::pi_ai_types::ContentBlock;

pub type ExecuteFn = Arc<
    dyn Fn(
            String,
            serde_json::Value,
            Option<tokio::sync::watch::Receiver<bool>>,
            Option<Arc<dyn Fn(AgentToolResult<serde_json::Value>) + Send + Sync>>,
        ) -> Pin<
            Box<
                dyn Future<
                        Output = Result<
                            AgentToolResult<serde_json::Value>,
                            Box<dyn Error + Send + Sync>,
                        >,
                    > + Send,
            >,
        > + Send
        + Sync,
>;

#[must_use]
pub fn empty_schema() -> serde_json::Value {
    serde_json::json!({"type": "object", "properties": {}, "required": []})
}

#[must_use]
pub fn str_schema(params: &[(&str, &str)]) -> serde_json::Value {
    let properties: serde_json::Map<String, serde_json::Value> = params
        .iter()
        .map(|(name, desc)| {
            (
                (*name).to_string(),
                serde_json::json!({"type": "string", "description": desc}),
            )
        })
        .collect();
    let required: Vec<String> = params.iter().map(|(n, _)| (*n).to_string()).collect();
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
    })
}

#[must_use]
pub fn str_arg(args: &serde_json::Value, name: &str) -> String {
    args.get(name)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

pub fn text_result<T: serde::Serialize>(value: &T) -> AgentToolResult<serde_json::Value> {
    AgentToolResult {
        content: vec![ContentBlock::text(
            serde_json::to_string(value).unwrap_or_default(),
        )],
        details: serde_json::Value::Null,
        terminate: None,
    }
}

pub fn to_box_err(e: impl std::fmt::Display) -> Box<dyn Error + Send + Sync> {
    e.to_string().into()
}

pub fn tool(
    name: &str,
    description: &str,
    parameters_schema: serde_json::Value,
    execute: ExecuteFn,
) -> Arc<DynTool> {
    Arc::new(DynTool {
        name: name.to_string(),
        description: description.to_string(),
        parameters_schema,
        label: String::new(),
        execution_mode: None,
        prepare_arguments: None,
        execute,
    })
}
