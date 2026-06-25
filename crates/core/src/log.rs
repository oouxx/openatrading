/// 统一日志模块——封装 tracing，项目中所有日志调用统一经过此模块。
///
/// 用法：
/// ```ignore
/// use openatrading_core::log;
/// log::info!("hello {name}", name = "world");
/// log::warn!("something unusual");
/// log::error!("something broke: {e}");
/// ```
///
/// 后续如需切换日志后端（如 log crate + env_logger），只需改此一处。
pub use tracing::{debug, error, info, trace, warn, Level};
