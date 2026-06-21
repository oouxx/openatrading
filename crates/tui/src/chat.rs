use std::sync::Arc;

use envision::component::{
    ConversationRole, ConversationView, ConversationViewMessage, ConversationViewState,
    MessageBlock, MessageHandle, StatusBar, StatusBarItem, StatusBarState,
};
use envision::prelude::*;
use envision::{TextArea, TextAreaMessage, TextAreaState};

use openatrading_core::llm::{
    pi_ai_types::ContentBlock, AgentEvent, AgentMessage, DynTool, LlmClient,
};

/// Chat 子命令运行所需的依赖，由 CLI 层根据 `AppConfig` 构建后注入。
///
/// 与 `cmd_decision` / `web::ai_chat` 走同一套装配：`LlmClient::from_config` +
/// `build_tools(ToolConfig)` + `SkillRegistry::build_chat_prompt`。
pub struct ChatArgs {
    pub llm: LlmClient,
    pub tools: Vec<Arc<DynTool>>,
    pub system_prompt: String,
}

// ---------------------------------------------------------------------------
// Focus
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Focus {
    Conversation,
    Input,
}

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Msg {
    Conv(ConversationViewMessage),
    Input(TextAreaMessage),

    SubmitInput,
    SpawnLLM,
    LLMChunk(String),
    LLMDone,
    LLMError(String),
    FocusToggle,
    Quit,
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

pub struct AppState {
    conv: ConversationViewState,
    input: TextAreaState,
    status: StatusBarState,
    focus: FocusManager<Focus>,
    streaming_handle: Option<MessageHandle>,
    msg_tx: Option<tokio::sync::mpsc::Sender<Msg>>,
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
        let conv = ConversationViewState::new()
            .with_markdown(true)
            .with_show_role_labels(true)
            .with_title("AI Chat");

        let input = TextAreaState::new().with_placeholder("Type a message... (Ctrl+Enter to send)");

        let mut status = StatusBarState::new();
        status.set_center(vec![StatusBarItem::new(
            "Tab: switch focus │ Ctrl+Enter: send",
        )]);

        let state = AppState {
            conv,
            input,
            status,
            focus: FocusManager::with_initial_focus(vec![Focus::Input, Focus::Conversation]),
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
            Msg::Conv(m) => {
                ConversationView::update(&mut state.conv, m);
            },
            Msg::Input(m) => {
                TextArea::update(&mut state.input, m);
            },
            Msg::SubmitInput => {
                let text = state.input.value();
                if text.trim().is_empty() {
                    return Command::none();
                }
                state.input.set_value("");
                state.conv.push_user(&text);
                update_status(state);
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
                if let Some(handle) = &state.streaming_handle {
                    state.conv.update_by_handle(*handle, |msg| {
                        msg.set_blocks(vec![MessageBlock::text(&text)]);
                        msg.set_streaming(true);
                    });
                }
                // Only auto-scroll if user hasn't manually scrolled up
                if state.conv.auto_scroll() {
                    return Command::message(Msg::Conv(ConversationViewMessage::ScrollToBottom));
                }
            },
            Msg::LLMDone => {
                if let Some(handle) = &state.streaming_handle {
                    state.conv.update_by_handle(*handle, |msg| {
                        msg.set_streaming(false);
                    });
                }
                state.streaming_handle = None;
                if state.conv.auto_scroll() {
                    return Command::message(Msg::Conv(ConversationViewMessage::ScrollToBottom));
                }
            },
            Msg::LLMError(err) => {
                if let Some(handle) = &state.streaming_handle {
                    state.conv.update_by_handle(*handle, |msg| {
                        msg.set_blocks(vec![MessageBlock::error(&err)]);
                        msg.set_streaming(false);
                    });
                }
                state.streaming_handle = None;
            },
            Msg::FocusToggle => {
                state.focus.focus_next();
            },
            Msg::Quit => return Command::quit(),
        }
        Command::none()
    }

    fn view(state: &Self::State, frame: &mut Frame) {
        let theme = Theme::default();
        let area = frame.area();

        let chunks = Layout::vertical([
            Constraint::Min(1),    // Conversation
            Constraint::Length(3), // Input
            Constraint::Length(1), // Status bar
        ])
        .split(area);

        let conv_focused = state.focus.is_focused(&Focus::Conversation);
        let input_focused = state.focus.is_focused(&Focus::Input);

        ConversationView::view(
            &state.conv,
            &mut RenderContext::new(frame, chunks[0], &theme).focused(conv_focused),
        );

        TextArea::view(
            &state.input,
            &mut RenderContext::new(frame, chunks[1], &theme).focused(input_focused),
        );

        StatusBar::view(
            &state.status,
            &mut RenderContext::new(frame, chunks[2], &theme),
        );
    }

    fn handle_event(_event: &Event) -> Option<Msg> {
        None
    }

    fn handle_event_with_state(state: &Self::State, event: &Event) -> Option<Msg> {
        // Mouse scroll → scroll conversation view regardless of focus
        if let Some(mouse) = event.as_mouse() {
            match mouse.kind {
                MouseEventKind::ScrollUp => {
                    return Some(Msg::Conv(ConversationViewMessage::ScrollUp));
                },
                MouseEventKind::ScrollDown => {
                    return Some(Msg::Conv(ConversationViewMessage::ScrollDown));
                },
                _ => {},
            }
        }

        let key = event.as_key()?;
        let ctrl = key.modifiers.ctrl();

        // Global shortcuts
        match key.code {
            Key::Char('q') if ctrl => return Some(Msg::Quit),
            Key::Esc => return Some(Msg::Quit),
            Key::Tab => return Some(Msg::FocusToggle),
            _ => {},
        }

        // Input-focused
        if state.focus.is_focused(&Focus::Input) {
            if key.code == Key::Enter {
                return Some(Msg::SubmitInput);
            }
            return TextArea::handle_event(&state.input, event, &EventContext::new().focused(true))
                .map(Msg::Input);
        }

        // Conversation-focused
        if state.focus.is_focused(&Focus::Conversation) {
            return ConversationView::handle_event(
                &state.conv,
                event,
                &EventContext::new().focused(true),
            )
            .map(Msg::Conv);
        }

        None
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn update_status(state: &mut AppState) {
    state.status.set_center(vec![StatusBarItem::new(format!(
        "Messages: {} │ Tab: switch focus │ Ctrl+Enter: send",
        state.conv.message_count(),
    ))]);
}

fn collect_llm_messages(conv: &ConversationViewState) -> Vec<serde_json::Value> {
    let mut msgs = Vec::new();
    for msg in conv.messages() {
        match msg.role() {
            ConversationRole::User | ConversationRole::Assistant => {
                let content = msg.text_content();
                if !content.is_empty() {
                    let role = match msg.role() {
                        ConversationRole::User => "user",
                        _ => "assistant",
                    };
                    msgs.push(serde_json::json!({"role": role, "content": content}));
                }
            },
            _ => {},
        }
    }
    msgs
}

async fn run_llm(
    msg_tx: tokio::sync::mpsc::Sender<Msg>,
    args: ChatArgs,
    llm_messages: Vec<serde_json::Value>,
) {
    use std::pin::Pin;

    let agent = Arc::new(args.llm.create_agent(args.tools, &args.system_prompt));

    let mut pi_messages: Vec<AgentMessage> = Vec::new();
    for msg in &llm_messages {
        if msg["role"] == "user" {
            pi_messages.push(AgentMessage::User {
                content: vec![ContentBlock::Text {
                    text: msg["content"].as_str().unwrap_or("").to_string(),
                    text_signature: None,
                }],
                timestamp: 0,
            });
        }
    }

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<AgentEvent>();
    let listener = Arc::new(
        move |event: AgentEvent, _signal: Option<tokio::sync::watch::Receiver<bool>>| {
            let tx = tx.clone();
            Box::pin(async move {
                let _ = tx.send(event);
            }) as Pin<Box<dyn std::future::Future<Output = ()> + Send>>
        },
    );

    let handle = agent.subscribe(listener).await;
    let agent_clone = Arc::clone(&agent);
    tokio::spawn(async move {
        let _ = agent_clone.process(pi_messages).await;
        drop(handle);
    });

    let mut response = String::new();
    while let Some(event) = rx.recv().await {
        if let AgentEvent::MessageUpdate {
            assistant_message_event:
                openatrading_core::llm::pi_ai_types::AssistantMessageEvent::TextDelta { delta, .. },
            ..
        } = event
        {
            response.push_str(&delta);
            let _ = msg_tx.try_send(Msg::LLMChunk(response.clone()));
        }
    }

    let _ = msg_tx.try_send(Msg::LLMDone).ok();
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run_chat(args: ChatArgs) -> anyhow::Result<()> {
    let mut runtime = Runtime::<ChatApp, _>::terminal_builder()?
        .with_args(args)
        .build()?;

    // Inject msg_tx so spawned LLM tasks can dispatch messages.
    runtime.state_mut().msg_tx = Some(runtime.message_sender());

    let _final_state = runtime.run_terminal().await?;
    Ok(())
}
