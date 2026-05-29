use crate::api::xuangubao::XuanguBaoClient;
use crate::llm::define_tool;

define_tool! {
    GetLimitUp,
    "get_limit_up",
    "获取当前涨停股票列表，包含股票代码、名称、连板天数、封板时间、炸板次数、涨停原因等详细信息",
    params: {},
    client: XuanguBaoClient,
    method: fetch_limit_up
}

define_tool! {
    GetLimitDown,
    "get_limit_down",
    "获取当前跌停股票列表",
    params: {},
    client: XuanguBaoClient,
    method: fetch_limit_down
}

define_tool! {
    GetMarketOverview,
    "get_market_overview",
    "获取当前市场概览数据，包含涨跌家数、涨停跌停数、炸板率、市场温度等",
    params: {},
    client: XuanguBaoClient,
    method: fetch_market_overview
}

#[cfg(test)]
mod tests {
    use crate::llm::tools::{GetLimitDown, GetLimitUp, GetMarketOverview};
    use crate::llm::Tool;

    #[tokio::test]
    async fn test_get_limit_up_tool_schema() {
        let tool = GetLimitUp;
        assert_eq!(tool.name(), "get_limit_up");
        assert!(!tool.description().is_empty());
        let schema = tool.parameters();
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn test_get_limit_down_tool_schema() {
        let tool = GetLimitDown;
        assert_eq!(tool.name(), "get_limit_down");
        let schema = tool.parameters();
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    async fn test_get_market_overview_tool_schema() {
        let tool = GetMarketOverview;
        assert_eq!(tool.name(), "get_market_overview");
        let schema = tool.parameters();
        assert_eq!(schema["type"], "object");
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_limit_up_tool_call() {
        let tool = GetLimitUp;
        let result = tool.execute(&serde_json::json!({})).await;
        assert!(result.is_ok(), "fetch_limit_up failed: {:?}", result.err());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json.is_array(), "expected JSON array, got: {}", json);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_limit_down_tool_call() {
        let tool = GetLimitDown;
        let result = tool.execute(&serde_json::json!({})).await;
        assert!(
            result.is_ok(),
            "fetch_limit_down failed: {:?}",
            result.err()
        );
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json.is_array(), "expected JSON array, got: {}", json);
    }

    #[tokio::test]
    #[ignore]
    async fn test_get_market_overview_tool_call() {
        let tool = GetMarketOverview;
        let result = tool.execute(&serde_json::json!({})).await;
        assert!(
            result.is_ok(),
            "fetch_market_overview failed: {:?}",
            result.err()
        );
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json.is_object(), "expected JSON object, got: {}", json);
        assert!(json.get("limit_up_count").is_some());
        assert!(json.get("market_temperature").is_some());
    }

    #[tokio::test]
    #[ignore]
    async fn test_toolset_with_real_tools() {
        use crate::llm::ToolSet;
        use std::sync::Arc;

        let tools = Arc::new(
            ToolSet::new()
                .with_tool(GetLimitUp)
                .with_tool(GetLimitDown)
                .with_tool(GetMarketOverview),
        );

        assert_eq!(tools.to_openai_tools().len(), 3);

        let result = tools.call("get_limit_up", &serde_json::json!({})).await;
        assert!(result.is_ok());
        let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert!(json.is_array());
    }
}
