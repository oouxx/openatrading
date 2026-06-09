use std::sync::Arc;

use envision::component::{
    ConversationRole, ConversationView, ConversationViewMessage, ConversationViewState,
    MessageBlock,
};
use envision::prelude::*;
use envision::{TextArea, TextAreaMessage, TextAreaState};

use openatrading_core::llm::{
    create_agent, pi_ai_types::ContentBlock, AgentEvent, AgentMessage, DynTool, LlmClient,
};

// ---------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum Msg {
    Input(TextAreaMessage),
    Conv(ConversationViewMessage),
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
    conv: ConversationViewState,
    input: TextAreaState,
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
    type Args = ();

    fn init(_: ()) -> (Self::State, Command<Msg>) {
        let mut conv = ConversationViewState::new()
            .with_markdown(true)
            .with_show_role_labels(true)
            .with_title(" AI Chat ");
        conv.push_system("AI Chat — Type your message and press Enter. Press Esc to quit.");

        let input = TextAreaState::new().with_placeholder("Type a message...");
        let llm = LlmClient::from_config(None, None);
        let tools: Vec<Arc<DynTool>> = vec![
            openatrading_core::llm::tools::get_market_overview(),
            openatrading_core::llm::tools::get_limit_up(),
            openatrading_core::llm::tools::get_limit_down(),
            openatrading_core::llm::tools::get_super_stock(),
            openatrading_core::llm::tools::get_yesterday_limit_up(),
            openatrading_core::llm::tools::get_dapan_abnormal(),
            openatrading_core::llm::tools::get_realtime_quote(),
            openatrading_core::llm::tools::get_telegraph(),
        ];
        let system_prompt =
            "你是一个A股短线交易AI助手。可以调用工具获取实时市场数据来回答用户问题。".to_string();

        let state = AppState {
            conv,
            input,
            msg_tx: None,
            llm,
            tools,
            system_prompt,
        };

        (state, Command::none())
    }

    fn update(state: &mut Self::State, msg: Msg) -> Command<Msg> {
        match msg {
            Msg::Input(m) => {
                TextArea::update(&mut state.input, m);
            },
            Msg::Conv(m) => {
                ConversationView::update(&mut state.conv, m);
            },
            Msg::SubmitInput => {
                let text = state.input.value().to_string();
                if text.trim().is_empty() {
                    return Command::none();
                }
                state.input.set_value("");
                state.conv.push_user(&text);
                // Queue SpawnLLM on the next tick so the view updates first
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

                tokio::spawn(async move {
                    run_llm(tx, llm, tools, system_prompt, llm_messages).await;
                });
            },
            Msg::LLMChunk(text) => {
                state.conv.update_last_message(|msg| {
                    msg.set_blocks(vec![MessageBlock::text(&text)]);
                    msg.set_streaming(text.len() < 2000);
                });
            },
            Msg::LLMDone => {
                state.conv.update_last_message(|msg| {
                    msg.set_streaming(false);
                });
            },
            Msg::LLMError(err) => {
                state.conv.update_last_message(|msg| {
                    msg.set_blocks(vec![MessageBlock::error(&err)]);
                    msg.set_streaming(false);
                });
            },
            Msg::Quit => return Command::quit(),
        }
        Command::none()
    }

    fn view(state: &Self::State, frame: &mut Frame) {
        let theme = Theme::default();
        let area = frame.area();

        let chunks = Layout::vertical([Constraint::Min(1), Constraint::Length(3)]).split(area);

        ConversationView::view(
            &state.conv,
            &mut RenderContext::new(frame, chunks[0], &theme).focused(true),
        );

        TextArea::view(
            &state.input,
            &mut RenderContext::new(frame, chunks[1], &theme).focused(true),
        );
    }

    fn handle_event_with_state(state: &Self::State, event: &Event) -> Option<Msg> {
        let key = event.as_key()?;
        let ctrl = key.modifiers.ctrl();

        match key.code {
            Key::Char('q') if ctrl => return Some(Msg::Quit),
            Key::Char('c') if ctrl => return Some(Msg::Quit),
            Key::Esc => return Some(Msg::Quit),
            Key::Enter if ctrl => return Some(Msg::SubmitInput),
            _ => {},
        }

        TextArea::handle_event(&state.input, event, &EventContext::default())
            .map(Msg::Input)
            .or_else(|| {
                ConversationView::handle_event(&state.conv, event, &EventContext::default())
                    .map(Msg::Conv)
            })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
    llm: LlmClient,
    tools: Vec<Arc<DynTool>>,
    system_prompt: String,
    llm_messages: Vec<serde_json::Value>,
) {
    use std::pin::Pin;

    let agent = Arc::new(create_agent(&llm, tools, &system_prompt));

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

pub async fn run_chat() -> anyhow::Result<()> {
    let mut runtime = Runtime::<ChatApp, _>::terminal_builder()?.build()?;

    // Inject the message sender so spawned LLM tasks can dispatch messages.
    runtime.state_mut().msg_tx = Some(runtime.message_sender());

    let _final_state = runtime.run_terminal().await?;
    Ok(())
}
