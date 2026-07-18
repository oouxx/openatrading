use std::cell::RefCell;
use std::sync::Arc;

use crossterm::event::{Event as CrosstermEvent, KeyCode, KeyModifiers, MouseEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use openatrading_core::llm::{AgentEvent, DynTool, LlmClient};

use crate::app::{App, Command, TerminalRuntime};
use crate::conversation::{
    render_conversation, ConversationViewMessage, ConversationViewState, MessageBlock,
};
use crate::input::TextInput;

/// Chat 子命令运行所需的依赖，由 CLI 层根据 `AppConfig` 构建后注入。
pub struct ChatArgs {
    pub llm: LlmClient,
    pub tools: Vec<Arc<DynTool>>,
    pub system_prompt: String,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Msg {
    ConvScroll(ConversationViewMessage),
    SubmitInput,
    SpawnLLM,
    LLMChunk(String),
    LLMDone,
    LLMError(String),
    Quit,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub conv: ConversationViewState,
    pub input: RefCell<TextInput>,
    message_count: usize,
    streaming_handle: Option<usize>,
    pub msg_tx: Option<tokio::sync::mpsc::UnboundedSender<Msg>>,
    llm: LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: String,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

pub struct ChatApp;

impl App for ChatApp {
    type State = AppState;
    type Message = Msg;
    type Args = ChatArgs;

    fn init(args: ChatArgs) -> (Self::State, Command<Msg>) {
        let conv = ConversationViewState::new();
        let input = TextInput::new("Type a message...");

        let state = AppState {
            conv,
            input: RefCell::new(input),
            message_count: 0,
            streaming_handle: None,
            msg_tx: None,
            llm: args.llm,
            tools: args.tools,
            system_prompt: args.system_prompt,
        };

        (state, Command::none())
    }

    fn update(state: &mut Self::State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::ConvScroll(m) => {
                state.conv.handle_scroll(&m);
            },
            Msg::SubmitInput => {
                let text = state.input.borrow().value();
                if text.trim().is_empty() {
                    return Command::none();
                }
                state.input.borrow_mut().clear();

                state.conv.push_user(&text);
                state.message_count = state.conv.message_count();
                return Command::message(Msg::SpawnLLM);
            },
            Msg::SpawnLLM => {
                let tx = match &state.msg_tx {
                    Some(tx) => tx.clone(),
                    None => return Command::none(),
                };
                let llm = state.llm.clone();
                let tools = state.tools.clone();
                let system_prompt = state.system_prompt.clone();
                let llm_messages = collect_llm_messages(&state.conv);

                let handle = state.conv.push_assistant("");
                state.streaming_handle = Some(handle);

                tokio::spawn(async move {
                    run_llm(
                        tx,
                        ChatArgs {
                            llm,
                            tools,
                            system_prompt,
                        },
                        llm_messages,
                    )
                    .await;
                });
            },
            Msg::LLMChunk(text) => {
                if let Some(idx) = state.streaming_handle {
                    state.conv.update_by_handle(idx, |msg| {
                        msg.set_blocks(vec![MessageBlock::text(&text)]);
                        msg.set_streaming(true);
                    });
                }
            },
            Msg::LLMDone => {
                if let Some(idx) = state.streaming_handle {
                    state.conv.update_by_handle(idx, |msg| {
                        msg.set_streaming(false);
                    });
                }
                state.streaming_handle = None;
            },
            Msg::LLMError(err) => {
                if let Some(idx) = state.streaming_handle {
                    state.conv.update_by_handle(idx, |msg| {
                        msg.set_blocks(vec![MessageBlock::error(&err)]);
                        msg.set_streaming(false);
                    });
                }
                state.streaming_handle = None;
            },
            Msg::Quit => return Command::quit(),
        }
        Command::none()
    }

    fn view(state: &Self::State, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Min(1),    // Conversation
            Constraint::Length(1), // Prompt input
        ])
        .split(area);

        // Conversation view (no border, full width)
        let mut conv = state.conv.clone();
        render_conversation(&mut conv, frame, chunks[0], false);

        // Draw separator line above input
        let sep_y = chunks[1].y.saturating_sub(1);
        if sep_y < area.height {
            let sep_style = Style::default().fg(Color::DarkGray).dim();
            let sep_line = Paragraph::new(
                std::iter::repeat_n("─", chunks[1].width as usize).collect::<String>(),
            )
            .style(sep_style);
            let sep_area = Rect::new(0, sep_y, chunks[1].width, 1);
            frame.render_widget(sep_line, sep_area);
        }

        // Input area — `> ` prompt style, always focused
        let input = state.input.borrow();
        input.render(frame, chunks[1], true);
        drop(input);

        // Hints line — only show when there are no messages
        if state.message_count == 0 {
            let hints = Paragraph::new("Enter to send · Esc to quit")
                .style(Style::default().fg(Color::DarkGray).italic());
            frame.render_widget(hints, chunks[0]);
        }
    }

    fn handle_event(event: &CrosstermEvent) -> Option<Msg> {
        if let CrosstermEvent::Key(key) = event {
            match key.code {
                KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
                    return Some(Msg::Quit);
                },
                KeyCode::Esc => return Some(Msg::Quit),
                _ => {},
            }
        }
        None
    }

    fn handle_event_with_state(state: &Self::State, event: &CrosstermEvent) -> Option<Msg> {
        // Mouse scroll → scroll conversation view
        if let CrosstermEvent::Mouse(mouse) = event {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    return Some(Msg::ConvScroll(ConversationViewMessage::ScrollUp));
                },
                MouseEventKind::ScrollDown => {
                    return Some(Msg::ConvScroll(ConversationViewMessage::ScrollDown));
                },
                _ => {},
            }
        }

        let key = match event {
            CrosstermEvent::Key(k) => k,
            _ => return None,
        };

        // Claude Code style: Enter sends, no focus switching needed
        if key.code == KeyCode::Enter {
            return Some(Msg::SubmitInput);
        }

        // Forward key events to TextInput
        match key.code {
            KeyCode::Char(c) => {
                state.input.borrow_mut().insert_char(c);
            },
            KeyCode::Backspace => {
                state.input.borrow_mut().backspace();
            },
            KeyCode::Delete => {
                state.input.borrow_mut().delete();
            },
            KeyCode::Left => {
                state.input.borrow_mut().cursor_left();
            },
            KeyCode::Right => {
                state.input.borrow_mut().cursor_right();
            },
            KeyCode::Home => {
                state.input.borrow_mut().home();
            },
            KeyCode::End => {
                state.input.borrow_mut().end();
            },
            KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown => {
                return Some(Msg::ConvScroll(ConversationViewMessage::ScrollUp));
            },
            _ => {},
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn collect_llm_messages(conv: &ConversationViewState) -> Vec<serde_json::Value> {
    let mut msgs = Vec::new();
    for msg in conv.messages() {
        match msg.role() {
            crate::conversation::ConversationRole::User
            | crate::conversation::ConversationRole::Assistant => {
                let content = msg.text_content();
                if !content.is_empty() {
                    let role = match msg.role() {
                        crate::conversation::ConversationRole::User => "user",
                        _ => "assistant",
                    };
                    msgs.push(serde_json::json!({"role": role, "content": content}));
                }
            },
        }
    }
    msgs
}

async fn run_llm(
    msg_tx: tokio::sync::mpsc::UnboundedSender<Msg>,
    args: ChatArgs,
    llm_messages: Vec<serde_json::Value>,
) {
    use pi_coding_agent::core::extensions::ToolDefinition;
    use pi_coding_agent::core::sdk::{self, CreateAgentSessionOptions, NoToolsMode};
    use pi_coding_agent::pi_agent_core::pi_ai_types::ToolExecutionMode;

    let tool_defs: Vec<ToolDefinition> = args
        .tools
        .iter()
        .map(|t| ToolDefinition {
            name: t.name.clone(),
            label: Some(t.label.clone()),
            description: t.description.clone(),
            prompt_snippet: None,
            prompt_guidelines: None,
            parameters: Some(t.parameters_schema.clone()),
            render_shell: None,
            execution_mode: t.execution_mode.as_ref().map(|m| match m {
                ToolExecutionMode::Sequential => "sequential".to_string(),
                ToolExecutionMode::Parallel => "parallel".to_string(),
            }),
        })
        .collect();

    let cwd = std::env::current_dir()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let (mut session, _) = sdk::create_agent_session(CreateAgentSessionOptions {
        cwd: cwd.clone(),
        model: Some(args.llm.model.clone()),
        no_tools: Some(NoToolsMode::All),
        custom_prompt: Some(args.system_prompt.clone()),
        custom_tools: Some(tool_defs),
        tools: Some(args.tools.iter().map(|t| t.name.clone()).collect()),
        agent_dir: None,
        thinking_level: None,
        scoped_models: None,
        exclude_tools: None,
        append_system_prompt: None,
        session_name: None,
        stream_fn: None,
        convert_to_llm: None,
        extension_paths: Vec::new(),
        enable_extensions: false,
        extension_registry: None,
        cli_provider: None,
        cli_model: None,
        persist_session: false,
        session_file: None,
        fork_from: None,
        session_dir: None,
    })
    .await
    .expect("create_agent_session failed");

    // Inject real tool implementations
    let agent = session.get_agent();
    agent.add_tools(args.tools).await;
    let _ = agent;

    // Combine conversation history into one user text
    let user_text: String = llm_messages
        .iter()
        .filter_map(|m| match m.get("role").and_then(|r| r.as_str()) {
            Some("user") => m.get("content").and_then(|c| c.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<AgentEvent>(256);
    let listener = std::sync::Arc::new(
        move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event).await;
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        },
    );

    session.subscribe(listener).await;
    session.add_user_text(&user_text).await;

    let mut response = String::new();
    while let Some(event) = rx.recv().await {
        if let AgentEvent::MessageUpdate {
            assistant_message_event:
                openatrading_core::llm::pi_ai_types::AssistantMessageEvent::TextDelta { delta, .. },
            ..
        } = event
        {
            response.push_str(&delta);
            let _ = msg_tx.send(Msg::LLMChunk(response.clone()));
        }
    }

    let _ = msg_tx.send(Msg::LLMDone).ok();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run_chat(args: ChatArgs) -> anyhow::Result<()> {
    let mut runtime = TerminalRuntime::<ChatApp>::new(args)?;

    // Inject msg_tx so spawned LLM tasks can dispatch messages.
    runtime.state_mut().msg_tx = Some(runtime.message_sender());

    let _final_state = runtime.run().await?;
    Ok(())
}
