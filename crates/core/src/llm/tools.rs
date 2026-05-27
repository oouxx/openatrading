use crate::api::xuangubao::XuanguBaoClient;
use crate::define_tool;

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