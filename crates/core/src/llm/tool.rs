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
        args.get(name)
            .and_then(|v| v.as_i64())
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
            })
    }
}

impl ExtractParam for f64 {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                ToolError::ExecutionFailed(format!("missing or invalid parameter: {name}"))
            })
    }
}

impl ExtractParam for bool {
    fn extract(args: &serde_json::Value, name: &str) -> Result<Self, ToolError> {
        args.get(name)
            .and_then(|v| v.as_bool())
            .ok_or_else(|| {
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

        impl $crate::llm::tool::Tool for $struct_name {
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
                            "type": <$ptype as $crate::llm::tool::SchemaType>::schema_type(),
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

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::llm::tool::ToolError>> + Send + '_>> {
                $(
                    let $pname: $ptype = match <$ptype as $crate::llm::tool::ExtractParam>::extract(arguments, stringify!($pname)) {
                        Ok(v) => v,
                        Err(e) => return Box::pin(async move { Err(e) }),
                    };
                )*
                Box::pin(async move {
                    let __client = <$client_type>::new();
                    let __result = __client.$method($($pname),*).await
                        .map_err(|e| $crate::llm::tool::ToolError::ExecutionFailed(e.to_string()))?;
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

        impl $crate::llm::tool::Tool for $struct_name {
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
                            "type": <$ptype as $crate::llm::tool::SchemaType>::schema_type(),
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

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::llm::tool::ToolError>> + Send + '_>> {
                $(
                    let $pname: $ptype = match <$ptype as $crate::llm::tool::ExtractParam>::extract(arguments, stringify!($pname)) {
                        Ok(v) => v,
                        Err(e) => return Box::pin(async move { Err(e) }),
                    };
                )*
                Box::pin(async move {
                    let __result = $func($($pname),*).await
                        .map_err(|e| $crate::llm::tool::ToolError::ExecutionFailed(e.to_string()))?;
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

        impl $crate::llm::tool::Tool for $struct_name {
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
                            "type": <$ptype as $crate::llm::tool::SchemaType>::schema_type(),
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

            fn execute(&self, arguments: &serde_json::Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, $crate::llm::tool::ToolError>> + Send + '_>> {
                let f = $handler;
                Box::pin(async move { f(arguments).await })
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeClient {
        _base: String,
    }

    impl FakeClient {
        fn new() -> Self {
            Self {
                _base: "fake".to_string(),
            }
        }
    }

    impl FakeClient {
        async fn get_items(&self) -> anyhow::Result<Vec<String>> {
            Ok(vec!["item1".to_string(), "item2".to_string()])
        }

        async fn get_by_id(&self, id: i64) -> anyhow::Result<String> {
            Ok(format!("item_{}", id))
        }

        async fn search(&self, keyword: String, limit: i64) -> anyhow::Result<Vec<String>> {
            Ok(vec![format!("{}_{}", keyword, limit)])
        }

        async fn fail(&self) -> anyhow::Result<String> {
            Err(anyhow::anyhow!("intentional failure"))
        }
    }

    async fn pure_greet(name: String) -> anyhow::Result<String> {
        Ok(format!("hello, {}!", name))
    }

    async fn pure_add(a: i64, b: i64) -> anyhow::Result<i64> {
        Ok(a + b)
    }

    define_tool! {
        GetItems,
        "get_items",
        "get all items",
        params: {},
        client: FakeClient,
        method: get_items
    }

    define_tool! {
        GetById,
        "get_by_id",
        "get item by id",
        params: { id: i64 => "item id" },
        client: FakeClient,
        method: get_by_id
    }

    define_tool! {
        SearchItems,
        "search_items",
        "search items",
        params: { keyword: String => "search keyword", limit: i64 => "max results" },
        client: FakeClient,
        method: search
    }

    define_tool! {
        FailTool,
        "fail_tool",
        "always fails",
        params: {},
        client: FakeClient,
        method: fail
    }

    define_tool! {
        Greet,
        "greet",
        "greet someone",
        params: { name: String => "person name" },
        pure_greet
    }

    define_tool! {
        Add,
        "add",
        "add two numbers",
        params: { a: i64 => "first number", b: i64 => "second number" },
        pure_add
    }

    define_tool! {
        EchoHandler,
        "echo_handler",
        "echo back arguments",
        params: {},
        handler: |args: &serde_json::Value| async move {
            Ok(args.to_string())
        }
    }

    #[tokio::test]
    async fn test_client_method_no_params() {
        let tool = GetItems;
        assert_eq!(tool.name(), "get_items");
        assert_eq!(tool.description(), "get all items");
        let schema = tool.parameters();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].as_object().unwrap().is_empty());

        let result = tool.execute(&serde_json::json!({})).await.unwrap();
        assert_eq!(result, "[\"item1\",\"item2\"]");
    }

    #[tokio::test]
    async fn test_client_method_with_params() {
        let tool = GetById;
        assert_eq!(tool.name(), "get_by_id");
        let schema = tool.parameters();
        assert_eq!(schema["properties"]["id"]["type"], "integer");
        assert!(schema["required"].as_array().unwrap().contains(&serde_json::json!("id")));

        let result = tool.execute(&serde_json::json!({"id": 42})).await.unwrap();
        assert_eq!(result, "\"item_42\"");
    }

    #[tokio::test]
    async fn test_client_method_multi_params() {
        let tool = SearchItems;
        let result = tool
            .execute(&serde_json::json!({"keyword": "rust", "limit": 5}))
            .await
            .unwrap();
        assert_eq!(result, "[\"rust_5\"]");
    }

    #[tokio::test]
    async fn test_client_method_error() {
        let tool = FailTool;
        let err = tool.execute(&serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("intentional failure"));
    }

    #[tokio::test]
    async fn test_pure_function() {
        let tool = Greet;
        assert_eq!(tool.name(), "greet");
        let result = tool.execute(&serde_json::json!({"name": "world"})).await.unwrap();
        assert_eq!(result, "\"hello, world!\"");
    }

    #[tokio::test]
    async fn test_pure_function_multi_params() {
        let tool = Add;
        let result = tool.execute(&serde_json::json!({"a": 3, "b": 4})).await.unwrap();
        assert_eq!(result, "7");
    }

    #[tokio::test]
    async fn test_handler_closure() {
        let tool = EchoHandler;
        let result = tool
            .execute(&serde_json::json!({"msg": "hi"}))
            .await
            .unwrap();
        assert!(result.contains("hi"));
    }

    #[tokio::test]
    async fn test_toolset_register_and_call() {
        let tools = Arc::new(
            ToolSet::new()
                .add(GetItems)
                .add(GetById)
                .add(Greet),
        );

        assert!(!tools.is_empty());

        let r1 = tools.call("get_items", &serde_json::json!({})).await.unwrap();
        assert_eq!(r1, "[\"item1\",\"item2\"]");

        let r2 = tools.call("get_by_id", &serde_json::json!({"id": 1})).await.unwrap();
        assert_eq!(r2, "\"item_1\"");

        let r3 = tools.call("greet", &serde_json::json!({"name": "test"})).await.unwrap();
        assert_eq!(r3, "\"hello, test!\"");
    }

    #[tokio::test]
    async fn test_toolset_not_found() {
        let tools = Arc::new(ToolSet::new().add(GetItems));
        let err = tools.call("nonexistent", &serde_json::json!({})).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_toolset_to_openai_tools() {
        let tools = ToolSet::new().add(GetById).add(Greet);
        let schemas = tools.to_openai_tools();
        assert_eq!(schemas.len(), 2);

        let names: Vec<&str> = schemas
            .iter()
            .map(|s| s["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&"get_by_id"));
        assert!(names.contains(&"greet"));

        for s in &schemas {
            assert_eq!(s["type"], "function");
            assert!(s["function"]["parameters"]["type"].is_string());
        }
    }

    #[tokio::test]
    async fn test_toolset_is_empty() {
        let tools = ToolSet::new();
        assert!(tools.is_empty());
        let tools = tools.add(GetItems);
        assert!(!tools.is_empty());
    }

    #[tokio::test]
    async fn test_extract_param_string() {
        let v = serde_json::json!({"name": "alice"});
        let s: String = ExtractParam::extract(&v, "name").unwrap();
        assert_eq!(s, "alice");
    }

    #[tokio::test]
    async fn test_extract_param_i64() {
        let v = serde_json::json!({"count": 42});
        let n: i64 = ExtractParam::extract(&v, "count").unwrap();
        assert_eq!(n, 42);
    }

    #[tokio::test]
    async fn test_extract_param_f64() {
        let v = serde_json::json!({"price": 9.99});
        let f: f64 = ExtractParam::extract(&v, "price").unwrap();
        assert!((f - 9.99).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_extract_param_bool() {
        let v = serde_json::json!({"active": true});
        let b: bool = ExtractParam::extract(&v, "active").unwrap();
        assert!(b);
    }

    #[tokio::test]
    async fn test_extract_param_missing() {
        let v = serde_json::json!({});
        let err = ExtractParam::<String>::extract(&v, "missing").unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn test_missing_param_in_tool_call() {
        let tool = GetById;
        let err = tool.execute(&serde_json::json!({})).await.unwrap_err();
        assert!(err.to_string().contains("missing or invalid parameter: id"));
    }
}