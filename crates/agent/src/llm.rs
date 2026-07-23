use anyhow::Result;

pub use pi_coding_agent::pi_agent_core::types::{StreamFn, StreamFnOptions};

// ── Type aliases ────────────────────────────────────────────────────────────

#[cfg(not(target_arch = "wasm32"))]
pub type StreamResult = std::pin::Pin<Box<dyn futures::Stream<Item = Result<String>> + Send>>;
#[cfg(target_arch = "wasm32")]
pub type StreamResult = std::pin::Pin<Box<dyn futures::Stream<Item = Result<String>>>>;

#[derive(Debug, serde::Deserialize)]
pub struct ToolCallMessage {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCallItem>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolCallItem {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub function: ToolCallFunction,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToolChatChoice {
    pub message: ToolCallMessage,
}

#[derive(Debug, serde::Deserialize)]
pub struct ToolChatResponse {
    pub choices: Vec<ToolChatChoice>,
}
