use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
}

/// A tool that can be called by an LLM.
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    /// JSON Schema for the tool's parameters (OpenAI format)
    fn parameters(&self) -> serde_json::Value;
    /// Execute the tool. `arguments` is the JSON value from the LLM's tool call.
    fn execute(&self, arguments: &serde_json::Value) -> Result<String, ToolError>;
}

/// Registry of tools, keyed by name.
pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for ToolSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool. Returns self for chaining.
    pub fn add(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
        self
    }

    /// Convert all registered tools to OpenAI-compatible tool definitions.
    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters(),
                    }
                })
            })
            .collect()
    }

    /// Look up and execute a tool by name.
    pub fn call(&self, name: &str, arguments: &serde_json::Value) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(arguments)
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}
