use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("tool execution failed: {0}")]
    ExecutionFailed(String),
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;
    fn execute(
        &self,
        arguments: &serde_json::Value,
    ) -> Pin<Box<dyn Future<Output = Result<String, ToolError>> + Send + '_>>;
}

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

    pub fn with_tool(mut self, tool: impl Tool + 'static) -> Self {
        self.tools.insert(tool.name().to_string(), Arc::new(tool));
        self
    }

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

    pub async fn call(
        &self,
        name: &str,
        arguments: &serde_json::Value,
    ) -> Result<String, ToolError> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::NotFound(name.to_string()))?;
        tool.execute(arguments).await
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
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

        impl $crate::tool::Tool for $struct_name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn description(&self) -> &str {
                $tool_desc
            }

            fn parameters(&self) -> serde_json::Value {
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
                serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                })
            }

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::tool::ToolError>> + Send + '_>> {
                $(
                    let $pname: $ptype = match <$ptype as $crate::tool::ExtractParam>::extract(arguments, stringify!($pname)) {
                        Ok(v) => v,
                        Err(e) => return Box::pin(async move { Err(e) }),
                    };
                )*
                Box::pin(async move {
                    let __client = <$client_type>::new();
                    let __result = __client.$method($($pname.as_ref()),*).await
                        .map_err(|e| $crate::tool::ToolError::ExecutionFailed(e.to_string()))?;
                    Ok(serde_json::to_string(&__result).unwrap_or_default())
                })
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

        impl $crate::tool::Tool for $struct_name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn description(&self) -> &str {
                $tool_desc
            }

            fn parameters(&self) -> serde_json::Value {
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
                serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                })
            }

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::tool::ToolError>> + Send + '_>> {
                $(
                    let $pname: $ptype = match <$ptype as $crate::tool::ExtractParam>::extract(arguments, stringify!($pname)) {
                        Ok(v) => v,
                        Err(e) => return Box::pin(async move { Err(e) }),
                    };
                )*
                Box::pin(async move {
                    let __result = $func($($pname),*).await
                        .map_err(|e| $crate::tool::ToolError::ExecutionFailed(e.to_string()))?;
                    Ok(serde_json::to_string(&__result).unwrap_or_default())
                })
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

        impl $crate::tool::Tool for $struct_name {
            fn name(&self) -> &str {
                $tool_name
            }

            fn description(&self) -> &str {
                $tool_desc
            }

            fn parameters(&self) -> serde_json::Value {
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
                serde_json::json!({
                    "type": "object",
                    "properties": properties,
                    "required": required,
                })
            }

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::tool::ToolError>> + Send + '_>> {
                let f = $handler;
                let __args = arguments.clone();
                Box::pin(async move { f(&__args).await })
            }
        }
    };
}
