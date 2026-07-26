use crate::event::Platform;
use ahash::AHashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// 会话状态快照（用于监控）。
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub platform: Platform,
    pub user_id: String,
    pub channel_id: String,
    pub user_name: Option<String>,
    pub is_active: bool,
    pub idle_secs: u64,
    pub created_at: Instant,
    pub last_active_at: Instant,
}

/// 内部会话。
#[derive(Debug, Clone)]
struct Session {
    platform: Platform,
    user_id: String,
    channel_id: String,
    user_name: Option<String>,
    is_active: bool,
    created_at: Instant,
    last_active_at: Instant,
}

/// 会话守卫——管理多平台活跃会话。
///
/// 提供会话的创建、查找、超时清理等能力。
#[derive(Debug, Clone)]
pub struct SessionManager {
    inner: Arc<RwLock<SessionManagerInner>>,
}

#[derive(Debug, Default)]
struct SessionManagerInner {
    sessions: AHashMap<String, Session>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(SessionManagerInner::default())),
        }
    }

    /// 生成会话 key：`{platform}:{user_id}`
    fn session_key(platform: Platform, user_id: &str) -> String {
        format!("{platform}:{user_id}")
    }

    /// 创建或更新会话。
    pub async fn upsert(
        &self,
        platform: Platform,
        user_id: String,
        channel_id: String,
        user_name: Option<String>,
    ) -> String {
        let key = Self::session_key(platform, &user_id);
        {
            let mut inner = self.inner.write().await;
            let entry = inner
                .sessions
                .entry(key.clone())
                .or_insert_with(|| Session {
                    platform,
                    user_id: user_id.clone(),
                    channel_id: channel_id.clone(),
                    user_name: user_name.clone(),
                    is_active: true,
                    created_at: Instant::now(),
                    last_active_at: Instant::now(),
                });

            entry.channel_id = channel_id;
            entry.user_name = user_name;
            entry.is_active = true;
            entry.last_active_at = Instant::now();
        }
        key
    }

    /// 获取会话信息。
    pub async fn get(&self, platform: &Platform, user_id: &str) -> Option<SessionInfo> {
        let key = Self::session_key(*platform, user_id);
        let inner = self.inner.read().await;
        inner.sessions.get(&key).map(|s| SessionInfo {
            platform: s.platform,
            user_id: s.user_id.clone(),
            channel_id: s.channel_id.clone(),
            user_name: s.user_name.clone(),
            is_active: s.is_active,
            idle_secs: s.last_active_at.elapsed().as_secs(),
            created_at: s.created_at,
            last_active_at: s.last_active_at,
        })
    }

    /// 将会话标记为非活跃。
    pub async fn deactivate(&self, platform: &Platform, user_id: &str) {
        let key = Self::session_key(*platform, user_id);
        let mut inner = self.inner.write().await;
        if let Some(s) = inner.sessions.get_mut(&key) {
            s.is_active = false;
        }
    }

    /// 清理超过 `timeout` 未活动的会话。
    pub async fn prune(&self, timeout: std::time::Duration) -> usize {
        let mut inner = self.inner.write().await;
        let before = inner.sessions.len();
        inner
            .sessions
            .retain(|_, s| s.last_active_at.elapsed() < timeout);
        before - inner.sessions.len()
    }

    /// 列出所有活跃会话。
    pub async fn list_active(&self) -> Vec<SessionInfo> {
        let inner = self.inner.read().await;
        inner
            .sessions
            .values()
            .filter(|s| s.is_active)
            .map(|s| SessionInfo {
                platform: s.platform,
                user_id: s.user_id.clone(),
                channel_id: s.channel_id.clone(),
                user_name: s.user_name.clone(),
                is_active: s.is_active,
                idle_secs: s.last_active_at.elapsed().as_secs(),
                created_at: s.created_at,
                last_active_at: s.last_active_at,
            })
            .collect()
    }

    /// 当前会话总数。
    pub async fn count(&self) -> usize {
        self.inner.read().await.sessions.len()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_upsert_and_get() {
        let mgr = SessionManager::new();
        let key = mgr
            .upsert(
                Platform::Telegram,
                "user1".into(),
                "chan1".into(),
                Some("Alice".into()),
            )
            .await;
        assert_eq!(key, "telegram:user1");

        let info = mgr.get(&Platform::Telegram, "user1").await;
        assert!(info.is_some());
        let info = info.unwrap_or_else(|| panic!("session should exist"));
        assert_eq!(info.user_id, "user1");
        assert_eq!(info.channel_id, "chan1");
        assert_eq!(info.user_name, Some("Alice".into()));
        assert!(info.is_active);
    }

    #[tokio::test]
    async fn test_session_prune() {
        let mgr = SessionManager::new();
        mgr.upsert(Platform::Telegram, "user1".into(), "chan1".into(), None)
            .await;
        // 立即清理，超时 0s
        let pruned = mgr.prune(std::time::Duration::from_secs(0)).await;
        assert_eq!(pruned, 1);
        assert_eq!(mgr.count().await, 0);
    }

    #[tokio::test]
    async fn test_deactivate() {
        let mgr = SessionManager::new();
        mgr.upsert(Platform::Telegram, "user1".into(), "chan1".into(), None)
            .await;
        mgr.deactivate(&Platform::Telegram, "user1").await;

        let info = mgr
            .get(&Platform::Telegram, "user1")
            .await
            .unwrap_or_else(|| panic!("session should exist"));
        assert!(!info.is_active);

        let active = mgr.list_active().await;
        assert_eq!(active.len(), 0);
    }
}
