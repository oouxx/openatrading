use openatrading_core::llm::build_market_tools;

#[test]
fn build_market_tools_returns_seventeen() {
    let tools = build_market_tools();
    assert_eq!(tools.len(), 17, "expected 17 public market tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    for required in [
        "get_limit_up",
        "get_market_overview",
        "get_kline",
        "get_telegraph",
        "get_field_items",
        "get_lhb_detail",
    ] {
        assert!(names.contains(&required), "missing tool: {required}");
    }
}
