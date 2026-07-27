/// GUI 收到的事件，包含 session 标识
use pi_coding_agent::core::agent_session::AgentSessionEvent;
pub enum GuiEvent {
    SessionEvent {
        session_id: String,
        event: AgentSessionEvent,
    },
    SessionError {
        session_id: String,
        error: String,
    },
}
// async fn gui_event_loop(mut rx: mpsc::Receiver<GuiEvent>) {
//     while let Some(event) = rx.recv().await {
//         match event {
//             GuiEvent::SessionEvent { session_id, event } => {
//                 match event {
//                     AgentSessionEvent::AgentEvent(agent_event) => {
//                         match agent_event {
//                             AgentEvent::TextChunk(chunk) => {
//                                 // 追加到 session_id 对应的文本框
//                                 append_text(&session_id, &chunk);
//                             }
//                             AgentEvent::ToolCall(tc) => {
//                                 // 显示 tool call 状态
//                                 show_tool_call(&session_id, &tc);
//                             }
//                             AgentEvent::AgentEnd(result) => {
//                                 // 流式输出结束
//                                 on_stream_end(&session_id, result);
//                             }
//                             _ => {}
//                         }
//                     }
//                     AgentSessionEvent::SessionStats(stats) => {
//                         update_stats(&session_id, stats);
//                     }
//                     _ => {}
//                 }
//             }
//             GuiEvent::SessionError { session_id, error } => {
//                 show_error(&session_id, &error);
//             }
//         }
//     }
// }
