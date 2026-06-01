use std::collections::HashMap;

use pi_agent_core::pi_ai_types::ContentBlock;

pub use pi_agent_core::types::{AgentTool, AgentToolResult};

pub fn text_content_block(text: impl Into<String>) -> ContentBlock {
    ContentBlock::text(text)
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
}

pub struct ToolSet {
    tools: HashMap<String, AgentTool<serde_json::Value, serde_json::Value>>,
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

    pub fn with_tool(mut self, tool: AgentTool<serde_json::Value, serde_json::Value>) -> Self {
        self.tools.insert(tool.name.clone(), tool);
        self
    }

    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters_schema,
                    }
                })
            })
            .collect()
    }

    pub fn to_pi_tools(&self) -> Vec<pi_agent_core::pi_ai_types::Tool> {
        self.tools
            .values()
            .map(|t| pi_agent_core::pi_ai_types::Tool {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters_schema.clone(),
            })
            .collect()
    }

    pub fn to_agent_tools(&self) -> Vec<std::sync::Arc<AgentTool<serde_json::Value, serde_json::Value>>> {
        self.tools.values().map(|t| std::sync::Arc::new(t.clone())).collect()
    }

    pub async fn call(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;

        let params = if let Some(prepare) = &tool.prepare_arguments {
            prepare(arguments)
        } else {
            arguments.clone()
        };

        let result = (tool.execute)(
            name.to_string(),
            params,
            None,
            None,
        )
        .await
        .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let text: String = result
            .content
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();

        Ok(text)
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    pub fn register(&mut self, name: String, tool: AgentTool<serde_json::Value, serde_json::Value>) {
        self.tools.insert(name, tool);
    }
}

pub trait ExtractParam: Sized {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError>;
}

impl ExtractParam for String {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
            })
    }
}

impl ExtractParam for i64 {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name).and_then(|v| v.as_i64()).ok_or_else(|| {
            ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
        })
    }
}

impl ExtractParam for f64 {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name).and_then(|v| v.as_f64()).ok_or_else(|| {
            ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
        })
    }
}

impl ExtractParam for bool {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name).and_then(|v| v.as_bool()).ok_or_else(|| {
            ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
        })
    }
}

pub trait SchemaType {
    fn schema_type() -> &'static str;
}

impl SchemaType for String {
    fn schema_type() -> &'static str {
        "string"
    }
}
impl SchemaType for i64 {
    fn schema_type() -> &'static str {
        "integer"
    }
}
impl SchemaType for f64 {
    fn schema_type() -> &'static str {
        "number"
    }
}
impl SchemaType for bool {
    fn schema_type() -> &'static str {
        "boolean"
    }
}

#[macro_export]
macro_rules! define_tool {
    (
        $struct_name:ident,
        $tool_name:literal,
        $tool_desc:literal,
        params: {
            $( $pname:ident : $ptype:ty => $pdesc:literal ),* $(,)?
        },
        client: $client_type:ty,
        method: $method:ident
    ) => {
        pub struct $struct_name;

        impl $struct_name {
            pub fn to_agent_tool() -> $crate::tool::AgentTool<serde_json::Value, serde_json::Value> {
                let mut properties = serde_json::Map::new();
                let mut required: Vec<String> = Vec::new();
                $(
                    properties.insert(
                        stringify!($pname).to_string(),
                        serde_json::json!({
                            "type": <$ptype as $crate::tool::SchemaType>::schema_type(),
                            "description": $pdesc,
                        }),
                    );
                    required.push(stringify!($pname).to_string());
                )*
                let parameters_schema = serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                });

                $crate::tool::AgentTool {
                    name: $tool_name.to_string(),
                    description: $tool_desc.to_string(),
                    label: $tool_name.to_string(),
                    parameters_schema,
                    execution_mode: None,
                    prepare_arguments: None,
                    execute: std::sync::Arc::new(move |_tool_call_id, args, _signal, _progress| {
                        $(
                            let $pname: $ptype = match <$ptype as $crate::tool::ExtractParam>::extract(&args, stringify!($pname)) {
                                Ok(v) => v,
                                Err(e) => return Box::pin(async move {
                                    Err::<$crate::tool::AgentToolResult<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>>(
                                        e.to_string().into()
                                    )
                                }),
                            };
                        )*
                        Box::pin(async move {
                            let __client = <$client_type>::new();
                            let __result = __client.$method($($pname.as_ref()),*).await
                                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })?;
                            Ok($crate::tool::AgentToolResult {
                                content: vec![$crate::tool::text_content_block(
                                    serde_json::to_string(&__result).unwrap_or_default()
                                )],
                                details: serde_json::Value::Null,
                                terminate: None,
                            })
                        })
                    }),
                }
            }
        }

        impl $crate::tool::ToolSetExt for $struct_name {
            fn register(self, set: &mut $crate::tool::ToolSet) {
                set.register($tool_name.to_string(), Self::to_agent_tool());
            }
        }
    };

    (
        $struct_name:ident,
        $tool_name:literal,
        $tool_desc:literal,
        params: {
            $( $pname:ident : $ptype:ty => $pdesc:literal ),* $(,)?
        },
        $func:path
    ) => {
        pub struct $struct_name;

        impl $struct_name {
            pub fn to_agent_tool() -> $crate::tool::AgentTool<serde_json::Value, serde_json::Value> {
                let mut properties = serde_json::Map::new();
                let mut required: Vec<String> = Vec::new();
                $(
                    properties.insert(
                        stringify!($pname).to_string(),
                        serde_json::json!({
                            "type": <$ptype as $crate::tool::SchemaType>::schema_type(),
                            "description": $pdesc,
                        }),
                    );
                    required.push(stringify!($pname).to_string());
                )*
                let parameters_schema = serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                });

                $crate::tool::AgentTool {
                    name: $tool_name.to_string(),
                    description: $tool_desc.to_string(),
                    label: $tool_name.to_string(),
                    parameters_schema,
                    execution_mode: None,
                    prepare_arguments: None,
                    execute: std::sync::Arc::new(move |_tool_call_id, args, _signal, _progress| {
                        $(
                            let $pname: $ptype = match <$ptype as $crate::tool::ExtractParam>::extract(&args, stringify!($pname)) {
                                Ok(v) => v,
                                Err(e) => return Box::pin(async move {
                                    Err::<$crate::tool::AgentToolResult<serde_json::Value>, Box<dyn std::error::Error + Send + Sync>>(
                                        e.to_string().into()
                                    )
                                }),
                            };
                        )*
                        Box::pin(async move {
                            let __result = $func($($pname),*).await
                                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })?;
                            Ok($crate::tool::AgentToolResult {
                                content: vec![$crate::tool::text_content_block(
                                    serde_json::to_string(&__result).unwrap_or_default()
                                )],
                                details: serde_json::Value::Null,
                                terminate: None,
                            })
                        })
                    }),
                }
            }
        }

        impl $crate::tool::ToolSetExt for $struct_name {
            fn register(self, set: &mut $crate::tool::ToolSet) {
                set.register($tool_name.to_string(), Self::to_agent_tool());
            }
        }
    };

    (
        $struct_name:ident,
        $tool_name:literal,
        $tool_desc:literal,
        params: {
            $( $pname:ident : $ptype:ty => $pdesc:literal ),* $(,)?
        },
        handler: $handler:expr
    ) => {
        pub struct $struct_name;

        impl $struct_name {
            pub fn to_agent_tool() -> $crate::tool::AgentTool<serde_json::Value, serde_json::Value> {
                let mut properties = serde_json::Map::new();
                let mut required: Vec<String> = Vec::new();
                $(
                    properties.insert(
                        stringify!($pname).to_string(),
                        serde_json::json!({
                            "type": <$ptype as $crate::tool::SchemaType>::schema_type(),
                            "description": $pdesc,
                        }),
                    );
                    required.push(stringify!($pname).to_string());
                )*
                let parameters_schema = serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                });

                let f = $handler;
                $crate::tool::AgentTool {
                    name: $tool_name.to_string(),
                    description: $tool_desc.to_string(),
                    label: $tool_name.to_string(),
                    parameters_schema,
                    execution_mode: None,
                    prepare_arguments: None,
                    execute: std::sync::Arc::new(move |_tool_call_id, args, _signal, _progress| {
                        let __args = args.clone();
                        let f = f.clone();
                        Box::pin(async move {
                            let result_str = f(&__args).await;
                            Ok($crate::tool::AgentToolResult {
                                content: vec![$crate::tool::text_content_block(result_str)],
                                details: serde_json::Value::Null,
                                terminate: None,
                            })
                        })
                    }),
                }
            }
        }

        impl $crate::tool::ToolSetExt for $struct_name {
            fn register(self, set: &mut $crate::tool::ToolSet) {
                set.tools.insert($tool_name.to_string(), Self::to_agent_tool());
            }
        }
    };
}

pub trait ToolSetExt {
    fn register(self, set: &mut ToolSet);
}
