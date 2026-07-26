use super::gui_event::GuiEvent;
use pi_coding_agent::core::agent_session_runtime::*;
use pi_coding_agent::core::session_manager::SessionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
struct SessionPool {
    /// 所有活跃的 session runtime
    runtimes: HashMap<String, Arc<Mutex<AgentSessionRuntime>>>,
    /// 向 GUI 发送事件的 channel
    gui_tx: mpsc::Sender<GuiEvent>,
    /// 创建 runtime 的 factory（所有 session 共用）
    factory: Arc<CreateAgentSessionRuntimeFactory>,
    agent_dir: String,
}

impl SessionPool {
    /// 创建新 session
    async fn create_session(&mut self, cwd: &str) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        let session_dir = format!("{}/sessions/{}", self.agent_dir, session_id);

        let session_manager = SessionManager::new(cwd, &session_dir, None, true, None);

        let runtime = create_agent_session_runtime(
            (*self.factory).clone(),
            CreateAgentSessionRuntimeParams {
                cwd: cwd.to_string(),
                agent_dir: self.agent_dir.clone(),
                session_manager,
                session_start_event: None,
            },
        )
        .await;

        // 订阅事件，带上 session_id 转发到 GUI
        let gui_tx = self.gui_tx.clone();
        let sid = session_id.clone();
        runtime.session().subscribe(Arc::new(move |event| {
            gui_tx
                .try_send(GuiEvent::SessionEvent {
                    session_id: sid.clone(),
                    event,
                })
                .ok();
        }));

        let runtime = Arc::new(Mutex::new(runtime));
        self.runtimes.insert(session_id.clone(), runtime);
        Ok(session_id)
    }

    /// 向指定 session 发送消息（流式输出）
    async fn prompt(&self, session_id: &str, text: &str) -> Result<(), String> {
        let runtime = self.runtimes.get(session_id).ok_or("Session not found")?;
        let mut rt = runtime.lock().await;
        rt.session_mut().prompt(text, None).await;
        Ok(())
    }

    /// 切换指定 session 到另一个 session 文件
    async fn switch_session(&self, session_id: &str, path: &str) -> Result<bool, String> {
        let runtime = self.runtimes.get(session_id).ok_or("Session not found")?;
        let mut rt = runtime.lock().await;
        rt.switch_session(path, None, None).await
    }

    /// 关闭指定 session
    async fn close_session(&mut self, session_id: &str) {
        if let Some(runtime) = self.runtimes.remove(session_id) {
            let rt = runtime.lock().await;
            rt.dispose().await;
        }
    }
}
/*
let (gui_tx, gui_rx) = mpsc::channel(1024);

let factory = create_factory(&agent_dir);  // 你的 factory 实现

let mut pool = SessionPool {
    runtimes: HashMap::new(),
    gui_tx,
    factory: Arc::new(factory),
    agent_dir: agent_dir.to_string(),
};

// 创建两个 session 同时运行
let s1 = pool.create_session("/project/a").await.unwrap();
let s2 = pool.create_session("/project/b").await.unwrap();

// 同时发消息
pool.prompt(&s1, "解释这段代码").await;
pool.prompt(&s2, "重构这个函数").await;

// GUI 事件循环
gui_event_loop(gui_rx).await;
*/
